//! `ai-memory restore-agents` — restore AI agent configurations,
//! skills, plugins, and instructions from an archive.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::PathBuf;

use ai_memory_core::agent_backup::{
    AGENT_BACKUP_SCHEMA_VERSION, AgentAssetScope, AgentBackupManifest,
};
use anyhow::{Context, Result, bail};
use flate2::read::GzDecoder;
use tar::Archive;
use tracing::info;

use crate::cli::RestoreAgentsArgs;
use crate::commands::apply_shared::{ApplyOutcome, apply_atomic};
use crate::commands::path_util::home_dir;
use crate::config::Config;

/// Run the `restore-agents` subcommand.
///
/// # Errors
/// Returns an error if the archive cannot be read, contains path traversal,
/// has an unsupported schema version, or writes fail.
pub fn run(_config: &Config, args: RestoreAgentsArgs) -> Result<()> {
    let home = home_dir().context("locating user home directory")?;
    let cwd = std::env::current_dir().context("locating current working directory")?;

    let src = &args.from;
    if !src.is_file() {
        bail!("backup archive not found at {}", src.display());
    }

    let file = File::open(src).with_context(|| format!("opening archive at {}", src.display()))?;
    let tar = GzDecoder::new(file);
    let mut archive = Archive::new(tar);

    // Read all archive entries into memory keyed by archive_path
    let mut files_by_path: HashMap<String, Vec<u8>> = HashMap::new();
    let mut manifest_bytes: Option<Vec<u8>> = None;

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.to_string_lossy().to_string();

        validate_archive_path(&path)?;

        let mut buf = Vec::new();
        entry.read_to_end(&mut buf)?;

        if path == "manifest.json" {
            manifest_bytes = Some(buf);
        } else {
            files_by_path.insert(path, buf);
        }
    }

    let Some(manifest_raw) = manifest_bytes else {
        bail!("archive at {} is missing manifest.json", src.display());
    };

    let manifest: AgentBackupManifest =
        serde_json::from_slice(&manifest_raw).context("parsing agent backup manifest")?;

    if manifest.version > AGENT_BACKUP_SCHEMA_VERSION {
        bail!(
            "archive manifest schema version {} is newer than supported version {}",
            manifest.version,
            AGENT_BACKUP_SCHEMA_VERSION
        );
    }

    let filter_agent = |agent_name: &str| -> bool {
        let Some(filter) = &args.agents else {
            return true;
        };
        let agent_str = agent_name.to_lowercase();
        filter.iter().any(|f| {
            let fl = f.trim().to_lowercase();
            agent_str.contains(&fl) || fl.contains(&agent_str)
        })
    };

    let mut planned = Vec::new();
    let mut identical_count = 0;

    for entry in &manifest.entries {
        let agent_name = format!("{:?}", entry.agent);
        if !filter_agent(&agent_name) {
            continue;
        }

        validate_archive_path(&entry.target_relative)?;

        let target_path = match entry.scope {
            AgentAssetScope::Global => home.join(&entry.target_relative),
            AgentAssetScope::Project => cwd.join(&entry.target_relative),
        };

        let Some(content) = files_by_path.get(&entry.archive_path) else {
            warn_missing_asset(&entry.archive_path);
            continue;
        };

        let exists = target_path.is_file();
        let is_identical = if exists {
            let existing_bytes = fs::read(&target_path)
                .with_context(|| format!("reading existing {}", target_path.display()))?;
            existing_bytes == *content
        } else {
            false
        };

        if is_identical {
            identical_count += 1;
        } else {
            planned.push((entry, target_path, content.clone(), exists));
        }
    }

    if planned.is_empty() && identical_count > 0 {
        println!(
            "✓ All {} assets are already identical on host. Nothing to restore.",
            identical_count
        );
        return Ok(());
    }

    if !args.apply {
        print_dry_run(&planned, identical_count);
        return Ok(());
    }

    let mut created = 0;
    let mut updated = 0;

    for (_entry, target_path, content, exists) in planned {
        if exists && !args.force {
            println!(
                "  [SKIP] {} (already exists, pass --force to overwrite)",
                target_path.display()
            );
            continue;
        }

        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating parent dir {}", parent.display()))?;
        }

        let content_str = String::from_utf8_lossy(&content);
        let outcome = apply_atomic(&target_path, |_old| Ok(content_str.to_string()))?;

        match outcome {
            ApplyOutcome::Created => {
                println!("  ✓ created {}", target_path.display());
                created += 1;
            }
            ApplyOutcome::Updated => {
                println!("  ✓ updated {}", target_path.display());
                updated += 1;
            }
            ApplyOutcome::NoOp => {
                identical_count += 1;
            }
        }
    }

    info!(created, updated, identical_count, "restored agent assets");
    println!(
        "\n✓ Restoration complete: {} created, {} updated, {} unchanged.",
        created, updated, identical_count
    );

    Ok(())
}

fn print_dry_run(
    planned: &[(
        &ai_memory_core::agent_backup::AgentBackupEntry,
        PathBuf,
        Vec<u8>,
        bool,
    )],
    identical_count: usize,
) {
    println!("Restore preview (dry-run, pass --apply to execute):\n");
    for (entry, target_path, content, exists) in planned {
        let status = if *exists { "[OVERWRITE]" } else { "[CREATE]" };
        println!(
            "  {status:<12} [{:?}] {} ({} bytes) -> {}",
            entry.agent,
            entry.asset_kind.label(),
            content.len(),
            target_path.display()
        );
    }
    if identical_count > 0 {
        println!("\n  (skipped {identical_count} identical files)");
    }
    println!("\nRerun with `--apply` to perform restoration.");
}

fn validate_archive_path(path: &str) -> Result<()> {
    if path.contains("..")
        || path.starts_with('/')
        || path.starts_with('\\')
        || path.contains('\0')
        || (path.len() >= 2 && path.as_bytes()[1] == b':')
    {
        bail!("malformed or dangerous archive path: {:?}", path);
    }
    Ok(())
}

fn warn_missing_asset(path: &str) {
    tracing::warn!(path, "manifest referenced asset missing from archive body");
}

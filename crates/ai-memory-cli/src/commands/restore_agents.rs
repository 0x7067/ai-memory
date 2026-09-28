//! `ai-memory restore-agents` — restore AI agent configurations,
//! skills, plugins, and instructions from an archive.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use ai_memory_core::agent_backup::{
    AGENT_BACKUP_SCHEMA_VERSION, AgentAssetScope, AgentBackupManifest,
};
use ai_memory_core::ids::AgentKind;
use anyhow::{Context, Result, bail};
use flate2::read::GzDecoder;
use sha2::{Digest, Sha256};
use tar::Archive;
use tracing::info;

use crate::cli::{AgentBackupScope, RestoreAgentsArgs};
use crate::commands::backup_agents::matches_agent_filter;
use crate::commands::path_util::{claude_config_dir, home_dir};
use crate::config::Config;

/// Run the `restore-agents` subcommand.
///
/// # Errors
/// Returns an error if the archive cannot be read, contains path traversal,
/// targets unauthorized destination paths, has an unsupported schema version,
/// or fails integrity checks.
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

    if manifest.version == 0 || manifest.version > AGENT_BACKUP_SCHEMA_VERSION {
        bail!(
            "archive manifest schema version {} is not supported (supported: 1..={})",
            manifest.version,
            AGENT_BACKUP_SCHEMA_VERSION
        );
    }

    let claude_override = claude_config_dir(std::env::var_os("CLAUDE_CONFIG_DIR"));

    let mut planned = Vec::new();
    let mut identical_count = 0;

    for entry in &manifest.entries {
        if args
            .agents
            .as_ref()
            .is_some_and(|filter| !matches_agent_filter(entry.agent, filter))
        {
            continue;
        }

        let matches_scope = match args.scope {
            AgentBackupScope::Both => true,
            AgentBackupScope::Global => entry.scope == AgentAssetScope::Global,
            AgentBackupScope::Project => entry.scope == AgentAssetScope::Project,
        };
        if !matches_scope {
            continue;
        }

        validate_target_relative(entry.scope, &entry.target_relative)?;

        let target_path = match entry.scope {
            AgentAssetScope::Global => {
                if entry.agent == AgentKind::ClaudeCode
                    && let Some(ref custom_dir) = claude_override
                    && let Some(rel) = entry.target_relative.strip_prefix(".claude/")
                {
                    custom_dir.join(rel)
                } else {
                    home.join(&entry.target_relative)
                }
            }
            AgentAssetScope::Project => cwd.join(&entry.target_relative),
        };

        let Some(content) = files_by_path.get(&entry.archive_path) else {
            warn_missing_asset(&entry.archive_path);
            continue;
        };

        // Verify SHA-256 integrity when hash is provided in manifest
        if let Some(ref expected_sha) = entry.sha256 {
            let mut hasher = Sha256::new();
            hasher.update(content);
            let actual_sha = format!("{:x}", hasher.finalize());
            if !actual_sha.eq_ignore_ascii_case(expected_sha) {
                bail!(
                    "SHA-256 integrity mismatch for asset {}: expected {}, got {}",
                    entry.archive_path,
                    expected_sha,
                    actual_sha
                );
            }
        }

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

    let any_sanitized = manifest.sanitized || manifest.entries.iter().any(|e| e.sanitized);

    if !args.apply {
        print_dry_run(&planned, identical_count, any_sanitized);
        return Ok(());
    }

    let mut created = 0;
    let mut updated = 0;
    let mut skipped_existing = 0;

    for (_entry, target_path, content, exists) in planned {
        if exists && !args.force {
            println!(
                "  [SKIP] {} (already exists, pass --force to overwrite)",
                target_path.display()
            );
            skipped_existing += 1;
            continue;
        }

        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating parent dir {}", parent.display()))?;
        }

        // Keep backup copy of pre-existing file on overwrite
        if exists {
            let stamp = jiff::Timestamp::now().as_second();
            let mut bak = target_path.as_os_str().to_owned();
            bak.push(format!(".bak-{stamp}"));
            let backup_path = PathBuf::from(bak);
            let _ = fs::copy(&target_path, &backup_path);
        }

        // Binary-safe atomic write
        ai_memory_wiki::write_atomic(&target_path, &content)
            .with_context(|| format!("writing {}", target_path.display()))?;

        if exists {
            println!("  ✓ updated {}", target_path.display());
            updated += 1;
        } else {
            println!("  ✓ created {}", target_path.display());
            created += 1;
        }
    }

    info!(
        created,
        updated, identical_count, skipped_existing, "restored agent assets"
    );
    println!(
        "\n✓ Restoration complete: {} created, {} updated, {} identical, {} skipped (existing).",
        created, updated, identical_count, skipped_existing
    );

    if any_sanitized {
        eprintln!(
            "⚠️  Note: This archive contains sanitized configurations. Restored MCP configs may require updating '[REDACTED:...]' tokens with valid credentials."
        );
    }

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
    any_sanitized: bool,
) {
    println!("Restore preview (dry-run, pass --apply to execute):\n");
    for (entry, target_path, content, exists) in planned {
        let status = if *exists { "[OVERWRITE]" } else { "[CREATE]" };
        println!(
            "  {status:<12} [{}] {} ({} bytes) -> {}",
            entry.agent.as_str(),
            entry.asset_kind.label(),
            content.len(),
            target_path.display()
        );
    }
    if identical_count > 0 {
        println!("\n  (skipped {identical_count} identical files)");
    }
    if any_sanitized {
        println!(
            "\n  ⚠️ Note: Archive contains sanitized configurations with redacted credentials."
        );
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

/// Enforce strict destination allowlisting so restored files cannot escape agent directories.
fn validate_target_relative(scope: AgentAssetScope, rel_path: &str) -> Result<()> {
    validate_archive_path(rel_path)?;

    let path = Path::new(rel_path);
    if !path.components().all(|c| matches!(c, Component::Normal(_))) {
        bail!(
            "target relative path contains non-normal components: {:?}",
            rel_path
        );
    }

    match scope {
        AgentAssetScope::Global => {
            const ALLOWED_GLOBAL_PREFIXES: &[&str] = &[
                ".claude/",
                ".claude.json",
                ".codex/",
                ".agents/",
                ".gemini/",
                ".cursor/",
                ".config/opencode/",
                ".devin/",
                ".grok/",
                ".kiro/",
                ".openclaw/",
                ".commandcode/",
                ".kimi/",
            ];
            let is_allowed = ALLOWED_GLOBAL_PREFIXES
                .iter()
                .any(|prefix| rel_path == *prefix || rel_path.starts_with(prefix));
            if !is_allowed {
                bail!(
                    "refusing to restore global asset to unauthorized path: {:?}",
                    rel_path
                );
            }
        }
        AgentAssetScope::Project => {
            const ALLOWED_PROJECT_EXACT: &[&str] = &[
                "CLAUDE.md",
                "AGENTS.md",
                "GEMINI.md",
                ".cursorrules",
                "opencode.json",
                "opencode.jsonc",
                ".vscode/mcp.json",
                ".cursor/mcp.json",
                ".grok/config.toml",
            ];
            const ALLOWED_PROJECT_PREFIXES: &[&str] = &[
                ".claude/skills/",
                ".agents/skills/",
                ".cursor/rules/",
                ".gemini/skills/",
                ".devin/skills/",
                ".grok/skills/",
            ];
            let is_allowed = ALLOWED_PROJECT_EXACT.contains(&rel_path)
                || ALLOWED_PROJECT_PREFIXES
                    .iter()
                    .any(|prefix| rel_path.starts_with(prefix));
            if !is_allowed {
                bail!(
                    "refusing to restore project asset to unauthorized path: {:?}",
                    rel_path
                );
            }
        }
    }
    Ok(())
}

fn warn_missing_asset(path: &str) {
    tracing::warn!(path, "manifest referenced asset missing from archive body");
}

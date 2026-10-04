//! `ai-memory profile` — inspect and curate the cross-project profile
//! (`docs/cross-project-profile.md`).
//!
//! Thin HTTP client: `status` and `list` read `/admin/profile/*`; `show` and
//! `forget` resolve the profile's scope through `status` and then use the
//! ordinary `/admin/read-page` and `/admin/delete-page`, so a profile entry is
//! read and removed exactly like any other page.

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::cli::{ProfileArgs, ProfileCommand, ProfileScopeArgs};
use crate::config::Config;
use crate::http_client::{ServerEndpoint, get_json, post_json};

/// Dispatch a `profile` subcommand.
///
/// # Errors
/// Transport failures, a non-root token on a multi-user server, or an entry
/// that does not exist.
pub async fn run(config: &Config, args: ProfileArgs) -> Result<()> {
    let ep = ServerEndpoint::from_config_resolving_auth(config).await;
    match args.command {
        ProfileCommand::Status(scope) => status(&ep, &scope).await,
        ProfileCommand::List(scope) => list(&ep, &scope).await,
        ProfileCommand::Show { path, scope } => show(&ep, &scope, &path).await,
        ProfileCommand::Forget { path, scope } => forget(&ep, &scope, &path).await,
    }
}

#[derive(Debug, Deserialize)]
struct ScopeNames {
    workspace: String,
    project: String,
}

#[derive(Debug, Deserialize)]
struct StatusResponse {
    enabled: String,
    share: String,
    effective_share: Option<String>,
    distinguishes_operators: bool,
    min_projects: u32,
    inject_on_session_start: bool,
    digest_max_bytes: usize,
    baseline_max_bytes: usize,
    apply_max_lines: usize,
    llm: bool,
    scope: Option<ScopeNames>,
    entries: usize,
    digest_bytes: usize,
    contribute_opt_outs: Vec<ScopeNames>,
}

fn scope_query(scope: &ProfileScopeArgs) -> Vec<(&'static str, &str)> {
    let mut query = vec![("workspace", scope.workspace.as_str())];
    if let Some(user) = scope.user.as_deref() {
        query.push(("user", user));
    }
    query
}

async fn fetch_status(ep: &ServerEndpoint, scope: &ProfileScopeArgs) -> Result<StatusResponse> {
    get_json(ep, "/admin/profile/status", &scope_query(scope))
        .await
        .context("reading the profile status")
}

async fn status(ep: &ServerEndpoint, scope: &ProfileScopeArgs) -> Result<()> {
    let s = fetch_status(ep, scope).await?;
    let deployment = if s.distinguishes_operators {
        "multi-user"
    } else {
        "single-user"
    };
    match s.effective_share.as_deref() {
        Some(share) => println!("Profile: on ({share}; {deployment} server)"),
        None => println!("Profile: off ({deployment} server)"),
    }
    println!("  enabled = {}, share = {}", s.enabled, s.share);
    match &s.scope {
        Some(names) => println!(
            "  scope: {}/{} — {} entr{}, digest {} of {} bytes",
            names.workspace,
            names.project,
            s.entries,
            if s.entries == 1 { "y" } else { "ies" },
            s.digest_bytes,
            s.digest_max_bytes
        ),
        None if s.effective_share.as_deref() == Some("user") && scope.user.is_none() => {
            println!("  scope: one private profile per user; pass --user <name> to inspect one");
        }
        None if s.effective_share.is_some() => {
            println!("  scope: no profile entries yet");
        }
        None => {}
    }
    println!(
        "  digest at session start: {} (baseline for new projects: {} bytes)",
        if s.inject_on_session_start {
            "on"
        } else {
            "off"
        },
        s.baseline_max_bytes
    );
    println!(
        "  min_projects = {}, apply_max_lines = {}, llm = {}",
        s.min_projects, s.apply_max_lines, s.llm
    );
    if s.contribute_opt_outs.is_empty() {
        println!("  every project contributes");
    } else {
        println!("  projects that opted out ([profile] contribute = false):");
        for p in &s.contribute_opt_outs {
            println!("    {}/{}", p.workspace, p.project);
        }
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct ListEntry {
    path: String,
    statement: String,
    applies_to: Vec<String>,
    enforced_by: bool,
}

#[derive(Debug, Deserialize)]
struct ListResponse {
    scope: Option<ScopeNames>,
    entries: Vec<ListEntry>,
}

async fn list(ep: &ServerEndpoint, scope: &ProfileScopeArgs) -> Result<()> {
    let resp: ListResponse = get_json(ep, "/admin/profile/list", &scope_query(scope))
        .await
        .context("listing the profile")?;
    let Some(names) = resp.scope else {
        println!("No profile entries yet.");
        return Ok(());
    };
    println!("{}/{}:", names.workspace, names.project);
    for entry in &resp.entries {
        let applies = if entry.applies_to.is_empty() {
            String::new()
        } else {
            format!(" [{}]", entry.applies_to.join(", "))
        };
        let enforced = if entry.enforced_by {
            " (enforced elsewhere; not in the digest)"
        } else {
            ""
        };
        println!("  {}{applies}{enforced}", entry.path);
        println!("      {}", entry.statement);
    }
    Ok(())
}

/// A profile path as the user may type it: with or without `profile/`.
fn profile_path(raw: &str) -> String {
    let raw = raw.trim().trim_start_matches('/');
    if raw.starts_with(ai_memory_core::profile::PROFILE_PATH_PREFIX) {
        raw.to_owned()
    } else {
        format!("{}{raw}", ai_memory_core::profile::PROFILE_PATH_PREFIX)
    }
}

async fn profile_scope(ep: &ServerEndpoint, scope: &ProfileScopeArgs) -> Result<ScopeNames> {
    let s = fetch_status(ep, scope).await?;
    match s.scope {
        Some(names) => Ok(names),
        None if s.effective_share.is_none() => bail!("the profile is off on this server"),
        None if s.effective_share.as_deref() == Some("user") && scope.user.is_none() => {
            bail!("this server keeps one profile per user; pass --user <name>")
        }
        None => bail!("there are no profile entries yet"),
    }
}

#[derive(Debug, Deserialize)]
struct PageContent {
    path: String,
    body: String,
}

async fn show(ep: &ServerEndpoint, scope: &ProfileScopeArgs, path: &str) -> Result<()> {
    let names = profile_scope(ep, scope).await?;
    let path = profile_path(path);
    let page: PageContent = get_json(
        ep,
        "/admin/read-page",
        &[
            ("workspace", names.workspace.as_str()),
            ("project", names.project.as_str()),
            ("path", path.as_str()),
        ],
    )
    .await
    .with_context(|| format!("reading {path}"))?;
    println!("# {} ({}/{})\n", page.path, names.workspace, names.project);
    println!("{}", page.body);
    Ok(())
}

#[derive(Debug, Deserialize)]
struct DeleteResponse {
    path: String,
    deleted: bool,
}

async fn forget(ep: &ServerEndpoint, scope: &ProfileScopeArgs, path: &str) -> Result<()> {
    let names = profile_scope(ep, scope).await?;
    let path = profile_path(path);
    let resp: DeleteResponse = post_json(
        ep,
        "/admin/delete-page",
        &serde_json::json!({
            "workspace": names.workspace,
            "project": names.project,
            "path": path,
        }),
    )
    .await
    .with_context(|| format!("forgetting {path}"))?;
    if resp.deleted {
        println!("Forgot {} (the git history keeps it).", resp.path);
    } else {
        println!("{} was not in the profile; nothing changed.", resp.path);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::profile_path;

    #[test]
    fn profile_paths_gain_the_prefix_once() {
        assert_eq!(profile_path("tools/pnpm.md"), "profile/tools/pnpm.md");
        assert_eq!(
            profile_path("profile/tools/pnpm.md"),
            "profile/tools/pnpm.md"
        );
        assert_eq!(profile_path(" /tools/pnpm.md "), "profile/tools/pnpm.md");
    }
}

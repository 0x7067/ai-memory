//! Store side of the cross-project profile
//! (`docs/design-cross-project-profile.md`): the per-project `[profile]`
//! flags (V74) and the bounded reads the SessionStart digest needs.

use std::collections::BTreeSet;

use ai_memory_core::profile::{PROFILE_PATH_PREFIX, ProfileEntry, stack_tags_in};
use ai_memory_core::{ProjectId, WorkspaceId};
use rusqlite::{Connection, OptionalExtension, params};

use crate::error::StoreResult;
use crate::reader::{not_expired, now_us};

/// Most profile entries one digest read loads; the byte budget usually cuts
/// far earlier, and the read stays bounded however large a profile grows.
pub const PROFILE_ENTRIES_LIMIT: usize = 200;

/// Recent observations scanned for stack signals. Bounded so the scan costs
/// the same on a project with a long history as on a new one.
const STACK_SIGNAL_OBSERVATIONS: i64 = 2_000;

/// A project's `[profile]` flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectProfileFlags {
    /// The project may be harvested into the profile.
    pub contribute: bool,
    /// The project receives the profile digest and query union.
    pub consume: bool,
}

impl Default for ProjectProfileFlags {
    fn default() -> Self {
        Self {
            contribute: true,
            consume: true,
        }
    }
}

/// Everything the SessionStart digest needs, read in one bounded pass.
#[derive(Debug, Clone, Default)]
pub struct ProfileDigestInputs {
    /// The profile scope's entries.
    pub entries: Vec<ProfileEntry>,
    /// Stack tags of the project the session is in.
    pub project_tags: BTreeSet<String>,
    /// Whether that project already has pages (false: baseline digest).
    pub project_has_pages: bool,
}

/// A project that opted out of contributing to the profile.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ProfileOptOut {
    /// Workspace name.
    pub workspace: String,
    /// Project name.
    pub project: String,
}

/// Persist `flags` on `project_id`, writing only when they differ from the
/// stored values, so the session-start path that calls this on every fetch
/// costs one indexed no-op statement in the common case.
pub(crate) fn set_project_profile_flags(
    conn: &Connection,
    project_id: ProjectId,
    flags: ProjectProfileFlags,
) -> StoreResult<()> {
    conn.execute(
        "UPDATE projects SET profile_contribute = ?2, profile_consume = ?3 \
         WHERE id = ?1 AND (profile_contribute <> ?2 OR profile_consume <> ?3)",
        params![
            project_id.as_bytes(),
            i64::from(flags.contribute),
            i64::from(flags.consume)
        ],
    )?;
    Ok(())
}

/// The stored flags of a project; the defaults for an unknown project.
pub(crate) fn project_profile_flags(
    conn: &Connection,
    workspace_id: WorkspaceId,
    project_id: ProjectId,
) -> StoreResult<ProjectProfileFlags> {
    let row: Option<(i64, i64)> = conn
        .query_row(
            "SELECT profile_contribute, profile_consume FROM projects \
             WHERE id = ?1 AND workspace_id = ?2",
            params![project_id.as_bytes(), workspace_id.as_bytes()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    Ok(
        row.map_or_else(ProjectProfileFlags::default, |(contribute, consume)| {
            ProjectProfileFlags {
                contribute: contribute != 0,
                consume: consume != 0,
            }
        }),
    )
}

/// Every project whose marker keeps it out of the profile, by name.
pub(crate) fn contribute_opt_outs(conn: &Connection) -> StoreResult<Vec<ProfileOptOut>> {
    let mut stmt = conn.prepare(
        "SELECT w.name, p.name FROM projects p JOIN workspaces w ON w.id = p.workspace_id \
         WHERE p.profile_contribute = 0 ORDER BY w.name, p.name",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(ProfileOptOut {
            workspace: row.get(0)?,
            project: row.get(1)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// The current, unexpired `profile/` pages of a scope, as digest entries.
/// Pages with nothing to state are skipped.
pub(crate) fn profile_entries(
    conn: &Connection,
    workspace_id: WorkspaceId,
    project_id: ProjectId,
    limit: usize,
) -> StoreResult<Vec<ProfileEntry>> {
    let limit = i64::try_from(limit.clamp(1, PROFILE_ENTRIES_LIMIT)).unwrap_or(1);
    let sql = format!(
        "SELECT path, title, body, frontmatter_json FROM pages \
         WHERE workspace_id = ?1 AND project_id = ?2 AND is_latest = 1 \
           AND path GLOB ?3{not_expired} \
         ORDER BY path ASC LIMIT ?5",
        not_expired = not_expired("pages", "?4"),
    );
    let mut stmt = conn.prepare_cached(&sql)?;
    let rows = stmt.query_map(
        params![
            workspace_id.as_bytes(),
            project_id.as_bytes(),
            format!("{PROFILE_PATH_PREFIX}*"),
            now_us(),
            limit
        ],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        },
    )?;
    let mut entries = Vec::new();
    for row in rows {
        let (path, title, body, frontmatter) = row?;
        let frontmatter = serde_json::from_str(&frontmatter).unwrap_or(serde_json::Value::Null);
        if let Some(entry) = ProfileEntry::from_page(&path, &title, &body, &frontmatter) {
            entries.push(entry);
        }
    }
    Ok(entries)
}

/// Stack tags the project's recent activity shows (file names and source
/// extensions in observation titles), from one bounded read.
pub(crate) fn project_stack_tags(
    conn: &Connection,
    workspace_id: WorkspaceId,
    project_id: ProjectId,
) -> StoreResult<BTreeSet<String>> {
    let mut stmt = conn.prepare_cached(
        "SELECT title FROM observations WHERE workspace_id = ?1 AND project_id = ?2 \
         ORDER BY created_at DESC LIMIT ?3",
    )?;
    let rows = stmt.query_map(
        params![
            workspace_id.as_bytes(),
            project_id.as_bytes(),
            STACK_SIGNAL_OBSERVATIONS
        ],
        |row| row.get::<_, String>(0),
    )?;
    let mut tags = BTreeSet::new();
    for title in rows {
        tags.extend(stack_tags_in(&title?).into_iter().map(str::to_owned));
    }
    Ok(tags)
}

/// Whether a project has any current page: a project without one is new, and
/// gets the larger baseline digest.
pub(crate) fn project_has_pages(
    conn: &Connection,
    workspace_id: WorkspaceId,
    project_id: ProjectId,
) -> StoreResult<bool> {
    Ok(conn
        .query_row(
            "SELECT 1 FROM pages WHERE workspace_id = ?1 AND project_id = ?2 \
             AND is_latest = 1 LIMIT 1",
            params![workspace_id.as_bytes(), project_id.as_bytes()],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

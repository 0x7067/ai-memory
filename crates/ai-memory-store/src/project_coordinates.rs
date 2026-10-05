//! Private indexed matching for static workspace/project coordinates.

use ai_memory_core::repository_identity::{
    IdentitySource, RepositoryIdentity, legacy_basename_name, path_style_name,
};
use ai_memory_core::{ProjectId, WorkspaceId};
use rusqlite::{Connection, params};

use crate::{StoreError, StoreResult};

pub(crate) const MAX_COORDINATE_ROWS: usize = 4;

const MATCH_SQL: &str = "SELECT id, name, NULLIF(canonical_name, '') \
     FROM projects INDEXED BY sqlite_autoindex_projects_2 \
     WHERE workspace_id = ?1 AND name = ?2 \
     UNION ALL \
     SELECT id, name, NULLIF(canonical_name, '') \
     FROM projects INDEXED BY idx_projects_canonical_name \
     WHERE workspace_id = ?1 AND canonical_name = ?2 AND canonical_name <> '' \
     UNION ALL \
     SELECT id, name, NULLIF(canonical_name, '') \
     FROM projects INDEXED BY idx_projects_legacy_name \
     WHERE workspace_id = ?1 AND legacy_name = ?2 AND legacy_name <> '' \
     LIMIT ?3";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProjectCoordinateMatch {
    pub(crate) id: ProjectId,
    pub(crate) current_name: String,
    pub(crate) canonical_name: Option<String>,
}

pub(crate) fn resolve(
    conn: &Connection,
    workspace_id: WorkspaceId,
    requested: &str,
) -> StoreResult<Option<ProjectCoordinateMatch>> {
    let limit = i64::try_from(MAX_COORDINATE_ROWS).unwrap_or(i64::MAX);
    let mut statement = conn.prepare(MATCH_SQL)?;
    let rows = statement
        .query_map(params![workspace_id.as_bytes(), requested, limit], |row| {
            Ok((
                row.get::<_, Vec<u8>>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut matches = Vec::with_capacity(rows.len());
    for (raw_id, current_name, canonical_name) in rows {
        let candidate = ProjectCoordinateMatch {
            id: ProjectId::from_slice(&raw_id)?,
            current_name,
            canonical_name,
        };
        if !matches
            .iter()
            .any(|existing: &ProjectCoordinateMatch| existing.id == candidate.id)
        {
            matches.push(candidate);
        }
    }
    match matches.len() {
        0 => Ok(None),
        1 => Ok(matches.pop()),
        _ => Err(StoreError::ProjectNameAmbiguous(requested.to_owned())),
    }
}

pub(crate) fn backfill(conn: &mut Connection) -> StoreResult<u64> {
    let mut updated = 0_u64;
    loop {
        let tx = conn.transaction()?;
        let rows = {
            let mut statement = tx.prepare(
                "SELECT id, identity FROM projects INDEXED BY idx_projects_coordinate_backfill \
                 WHERE identity_source = 'git_remote' AND identity <> '' \
                   AND (canonical_name = '' OR legacy_name = '') LIMIT 256",
            )?;
            statement
                .query_map([], |row| {
                    Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?
        };
        if rows.is_empty() {
            return Ok(updated);
        }
        for (raw_id, identity) in rows {
            let remote = RepositoryIdentity {
                identity,
                source: IdentitySource::GitRemote,
            };
            let canonical_name = path_style_name(&remote).ok_or_else(|| {
                StoreError::MalformedRecord("remote has no canonical name".into())
            })?;
            let legacy_name = legacy_basename_name(&remote)
                .ok_or_else(|| StoreError::MalformedRecord("remote has no basename".into()))?;
            updated += u64::try_from(tx.execute(
                "UPDATE projects SET canonical_name = ?1, legacy_name = ?2 WHERE id = ?3",
                params![canonical_name, legacy_name, raw_id],
            )?)
            .unwrap_or(0);
        }
        tx.commit()?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidate_query_is_bounded_and_uses_only_coordinate_indexes() {
        let mut conn = Connection::open_in_memory().unwrap();
        crate::migrations::run(&mut conn).unwrap();
        let workspace = crate::ops::get_or_create_workspace(&mut conn, "default").unwrap();
        for index in 0..1_000_u32 {
            crate::ops::get_or_create_project(
                &mut conn,
                &workspace,
                &format!("unrelated-{index}"),
                None,
            )
            .unwrap();
        }
        let plan: Vec<String> = conn
            .prepare(&format!("EXPLAIN QUERY PLAN {MATCH_SQL}"))
            .unwrap()
            .query_map(
                params![
                    workspace.as_bytes(),
                    "acme-api",
                    i64::try_from(MAX_COORDINATE_ROWS).unwrap()
                ],
                |row| row.get(3),
            )
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert!(MATCH_SQL.contains("LIMIT ?3"));
        let mut statement = conn.prepare(MATCH_SQL).unwrap();
        let mut rows = statement
            .query(params![
                workspace.as_bytes(),
                "absent",
                MAX_COORDINATE_ROWS as i64
            ])
            .unwrap();
        assert!(rows.next().unwrap().is_none());
        drop(rows);
        assert_eq!(
            statement.get_status(rusqlite::StatementStatus::FullscanStep),
            0
        );
        assert!(statement.get_status(rusqlite::StatementStatus::VmStep) < 100);
        drop(statement);
        for index in 0..20_u32 {
            conn.execute(
                "INSERT INTO projects (id, workspace_id, name, created_at, canonical_name, legacy_name) \
                 VALUES (?1, ?2, ?3, 1, 'collision', 'collision')",
                params![ProjectId::new().as_bytes(), workspace.as_bytes(), format!("collision-{index}")],
            ).unwrap();
        }
        let mut statement = conn.prepare(MATCH_SQL).unwrap();
        let count = statement
            .query_map(
                params![
                    workspace.as_bytes(),
                    "collision",
                    MAX_COORDINATE_ROWS as i64
                ],
                |_| Ok(()),
            )
            .unwrap()
            .count();
        assert_eq!(count, MAX_COORDINATE_ROWS);
        assert_eq!(
            statement.get_status(rusqlite::StatementStatus::FullscanStep),
            0
        );
        assert!(statement.get_status(rusqlite::StatementStatus::VmStep) < 200);
        assert!(matches!(
            resolve(&conn, workspace, "collision"),
            Err(StoreError::ProjectNameAmbiguous(_))
        ));
        assert!(
            plan.iter()
                .any(|line| line.contains("sqlite_autoindex_projects_2"))
        );
        assert!(
            plan.iter()
                .any(|line| line.contains("idx_projects_canonical_name"))
        );
        assert!(
            plan.iter()
                .any(|line| line.contains("idx_projects_legacy_name"))
        );
        assert!(
            plan.iter().all(|line| !line.contains("SCAN projects")),
            "{plan:?}"
        );
    }
}

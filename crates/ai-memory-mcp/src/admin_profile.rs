//! `/admin/profile/*`: operator view of the cross-project profile
//! (`docs/cross-project-profile.md`). Read-only; `ai-memory profile show`
//! and `forget` reuse `/admin/read-page` and `/admin/delete-page` against the
//! scope `status` reports. Root-only on a multi-user server, like every
//! `/admin/*` route.

use std::collections::BTreeSet;
use std::sync::Arc;

use ai_memory_core::profile::{EffectiveProfileShare, ProfileSettings, render_digest};
use ai_memory_core::{AuthLevel, Capability};
use ai_memory_store::{ReaderPool, ResolvedScope, ScopeResolutionError};
use axum::body::Body;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

/// State for the profile admin routes.
#[derive(Clone)]
pub struct ProfileAdminState {
    /// Reader pool.
    pub reader: ReaderPool,
    /// `[profile]` settings.
    pub profile: ProfileSettings,
    /// `[auth].actor_proxy_bearer_token` is configured; see
    /// [`crate::admin::AdminState::trusted_proxy_identity`].
    pub trusted_proxy_identity: bool,
}

/// Build the `/admin/profile/*` router.
pub fn profile_admin_router(state: ProfileAdminState) -> Router {
    let state = Arc::new(state);
    Router::new()
        .route("/admin/profile/status", get(handle_status))
        .route("/admin/profile/list", get(handle_list))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            require_root_for_multiuser_admin,
        ))
        .with_state(state)
}

/// Same gate as the rest of `/admin/*`: open on a single-operator server,
/// root-only once the deployment distinguishes operators.
async fn require_root_for_multiuser_admin(
    State(state): State<Arc<ProfileAdminState>>,
    req: axum::http::Request<Body>,
    next: Next,
) -> Response {
    let level = req
        .extensions()
        .get::<AuthLevel>()
        .copied()
        .unwrap_or(AuthLevel::Anonymous);
    let distinguishes = match state
        .reader
        .distinguishes_operators(state.trusted_proxy_identity)
        .await
    {
        Ok(distinguishes) => distinguishes,
        Err(error) => {
            tracing::error!(%error, "profile admin authorization could not read the operator topology");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "admin authorization unavailable" })),
            )
                .into_response();
        }
    };
    match level.authorize(Capability::Admin, distinguishes) {
        Ok(()) => next.run(req).await,
        Err(e) => {
            let status = if e.is_authentication_required() {
                StatusCode::UNAUTHORIZED
            } else {
                StatusCode::FORBIDDEN
            };
            (status, Json(serde_json::json!({ "error": e.message() }))).into_response()
        }
    }
}

/// Query of both routes.
#[derive(Debug, Deserialize)]
struct ProfileQuery {
    /// Workspace whose profile to inspect when `share = "workspace"`.
    #[serde(default = "default_workspace")]
    workspace: String,
    /// Operator whose private profile to inspect when `share = "user"`.
    #[serde(default)]
    user: Option<String>,
}

fn default_workspace() -> String {
    ai_memory_core::DEFAULT_WORKSPACE_NAME.to_owned()
}

/// Names of the scope holding the inspected profile.
#[derive(Debug, Serialize)]
struct ScopeNames {
    workspace: String,
    project: String,
}

/// The project holding the profile for `query`, without creating anything.
/// `Ok(None)` when it does not exist yet or a private profile was asked for
/// without naming its operator.
async fn resolve_scope(
    reader: &ReaderPool,
    share: EffectiveProfileShare,
    query: &ProfileQuery,
) -> Result<Option<(ResolvedScope, ScopeNames)>, ScopeResolutionError> {
    let (workspace, project) = match share {
        EffectiveProfileShare::Global => (
            ai_memory_core::DEFAULT_WORKSPACE_NAME.to_owned(),
            ai_memory_core::GLOBAL_SCOPE_PROJECT.to_owned(),
        ),
        EffectiveProfileShare::Workspace => (
            query.workspace.clone(),
            ai_memory_core::profile::WORKSPACE_PROFILE_PROJECT.to_owned(),
        ),
        EffectiveProfileShare::User => {
            let Some(username) = query
                .user
                .as_deref()
                .map(str::trim)
                .filter(|u| !u.is_empty())
            else {
                return Ok(None);
            };
            let Some(user) = reader.find_user_by_username(username.to_owned()).await? else {
                return Ok(None);
            };
            (
                ai_memory_core::DEFAULT_WORKSPACE_NAME.to_owned(),
                ai_memory_core::profile::user_profile_project(user.id),
            )
        }
    };
    match ai_memory_store::lookup_existing_scope(reader, &workspace, &project).await {
        Ok(scope) => Ok(Some((scope, ScopeNames { workspace, project }))),
        Err(
            ScopeResolutionError::WorkspaceNotFound { .. }
            | ScopeResolutionError::ProjectNotFoundInWorkspace { .. },
        ) => Ok(None),
        Err(e) => Err(e),
    }
}

fn internal(error: impl std::fmt::Display) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({ "error": error.to_string() })),
    )
        .into_response()
}

/// `GET /admin/profile/status`: effective mode, settings, the inspected
/// scope, its entry count and digest size, and the projects that opted out.
async fn handle_status(
    State(state): State<Arc<ProfileAdminState>>,
    Query(query): Query<ProfileQuery>,
) -> Response {
    let distinguishes = match state
        .reader
        .distinguishes_operators(state.trusted_proxy_identity)
        .await
    {
        Ok(distinguishes) => distinguishes,
        Err(e) => return internal(e),
    };
    let settings = &state.profile;
    let share = settings.effective_share(distinguishes);
    let (scope, entries, digest_bytes) = match share {
        None => (None, 0, 0),
        Some(share) => match resolve_scope(&state.reader, share, &query).await {
            Ok(None) => (None, 0, 0),
            Ok(Some((scope, names))) => {
                let entries = match state
                    .reader
                    .profile_entries(
                        scope.workspace_id,
                        scope.project_id,
                        ai_memory_store::PROFILE_ENTRIES_LIMIT,
                    )
                    .await
                {
                    Ok(entries) => entries,
                    Err(e) => return internal(e),
                };
                let digest =
                    render_digest(&entries, &BTreeSet::new(), settings.digest_budget(), false);
                (Some(names), entries.len(), digest.map_or(0, |d| d.len()))
            }
            Err(e) => return internal(e),
        },
    };
    let opt_outs = match state.reader.profile_contribute_opt_outs().await {
        Ok(rows) => rows,
        Err(e) => return internal(e),
    };
    Json(serde_json::json!({
        "enabled": settings.enabled.as_str(),
        "share": settings.share,
        "effective_share": share.map(EffectiveProfileShare::as_str),
        "distinguishes_operators": distinguishes,
        "min_projects": settings.min_projects,
        "inject_on_session_start": settings.inject_on_session_start,
        "digest_max_bytes": settings.digest_budget(),
        "baseline_max_bytes": settings.baseline_budget(),
        "apply_max_lines": settings.apply_max_lines,
        "llm": settings.llm,
        "scope": scope,
        "entries": entries,
        "digest_bytes": digest_bytes,
        "contribute_opt_outs": opt_outs,
    }))
    .into_response()
}

/// `GET /admin/profile/list`: the inspected profile's entries, by path.
async fn handle_list(
    State(state): State<Arc<ProfileAdminState>>,
    Query(query): Query<ProfileQuery>,
) -> Response {
    let distinguishes = match state
        .reader
        .distinguishes_operators(state.trusted_proxy_identity)
        .await
    {
        Ok(distinguishes) => distinguishes,
        Err(e) => return internal(e),
    };
    let Some(share) = state.profile.effective_share(distinguishes) else {
        return Json(serde_json::json!({ "scope": null, "entries": [] })).into_response();
    };
    let (scope, names) = match resolve_scope(&state.reader, share, &query).await {
        Ok(Some(found)) => found,
        Ok(None) => {
            return Json(serde_json::json!({ "scope": null, "entries": [] })).into_response();
        }
        Err(e) => return internal(e),
    };
    let entries = match state
        .reader
        .profile_entries(
            scope.workspace_id,
            scope.project_id,
            ai_memory_store::PROFILE_ENTRIES_LIMIT,
        )
        .await
    {
        Ok(entries) => entries,
        Err(e) => return internal(e),
    };
    let entries: Vec<serde_json::Value> = entries
        .iter()
        .map(|entry| {
            serde_json::json!({
                "path": entry.path,
                "category": entry.category(),
                "statement": entry.statement,
                "applies_to": entry.applies_to,
                "enforced_by": entry.enforced_by,
            })
        })
        .collect();
    Json(serde_json::json!({ "scope": names, "entries": entries })).into_response()
}

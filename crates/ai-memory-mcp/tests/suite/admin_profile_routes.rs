//! `/admin/profile/review` and `/admin/profile/rebuild`
//! (`docs/cross-project-profile.md`): the rebuild converges the profile from
//! scratch, the review reports entries and the habits still short of the bar,
//! and on a multi-user server both are root-only like every `/admin/*` route.

use ai_memory_core::profile::{ProfileEnabled, ProfileSettings, ProfileShare};
use ai_memory_core::{
    AgentKind, AuthLevel, NewObservation, NewSession, NewUser, ObservationKind, ProjectId,
    Sanitized, Sanitizer, SessionId, UserRole, WorkspaceId,
};
use ai_memory_mcp::admin_profile::{ProfileAdminState, profile_admin_router};
use ai_memory_store::Store;
use ai_memory_wiki::Wiki;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use tempfile::TempDir;
use tower::ServiceExt;

struct Fixture {
    _tmp: TempDir,
    store: Store,
    wiki: Wiki,
    ws: WorkspaceId,
}

async fn fixture() -> Fixture {
    let tmp = TempDir::new().unwrap();
    let store = Store::open(tmp.path()).unwrap();
    let ws = store
        .writer
        .get_or_create_workspace("default")
        .await
        .unwrap();
    let wiki = Wiki::new(tmp.path(), store.writer.clone())
        .unwrap()
        .with_store_reader(store.reader.clone());
    Fixture {
        _tmp: tmp,
        store,
        wiki,
        ws,
    }
}

async fn prompt(fx: &Fixture, project: &str, body: &str) -> ProjectId {
    let proj = fx
        .store
        .writer
        .get_or_create_project(fx.ws, project, None)
        .await
        .unwrap();
    let id = SessionId::new();
    fx.store
        .writer
        .begin_session(NewSession {
            occurred_at: None,
            id,
            workspace_id: fx.ws,
            project_id: proj,
            agent_kind: AgentKind::ClaudeCode,
            cwd: None,
            actor_user: None,
        })
        .await
        .unwrap();
    fx.store
        .writer
        .insert_observation(Sanitized::new(
            NewObservation {
                occurred_at: None,
                session_id: id,
                workspace_id: fx.ws,
                project_id: proj,
                kind: ObservationKind::UserPrompt,
                extension: None,
                source_event: None,
                title: "obs".into(),
                body: body.into(),
                importance: 5,
            },
            &Sanitizer::builtin(),
        ))
        .await
        .unwrap();
    proj
}

fn router(fx: &Fixture, profile: ProfileSettings) -> axum::Router {
    profile_admin_router(ProfileAdminState {
        reader: fx.store.reader.clone(),
        writer: fx.store.writer.clone(),
        wiki: fx.wiki.clone(),
        llm: None,
        profile,
        trusted_proxy_identity: false,
    })
}

async fn call(
    router: axum::Router,
    method: &str,
    uri: &str,
    level: AuthLevel,
) -> (StatusCode, serde_json::Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from("{}"))
        .unwrap();
    request.extensions_mut().insert(level);
    let response = router.oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
    )
}

/// Single-user: rebuild converges the profile, review shows the entry and the
/// habit that is still one project short.
#[tokio::test]
async fn rebuild_converges_and_review_reports_entries_and_waiting_habits() {
    let fx = fixture().await;
    prompt(&fx, "alpha", "Always use pnpm.").await;
    prompt(&fx, "alpha", "I prefer tabs over spaces.").await;

    let (status, report) = call(
        router(&fx, ProfileSettings::default()),
        "POST",
        "/admin/profile/rebuild",
        AuthLevel::Anonymous,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{report}");
    assert_eq!(report["share"], "global");
    assert_eq!(report["entries_written"], 1, "{report}");

    let (status, review) = call(
        router(&fx, ProfileSettings::default()),
        "GET",
        "/admin/profile/review",
        AuthLevel::Anonymous,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{review}");
    let entries = review["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 1, "{review}");
    assert!(entries[0]["statement"].as_str().unwrap().contains("pnpm"));
    assert_eq!(entries[0]["managed"], true);
    let waiting = review["waiting"].as_array().unwrap();
    assert!(
        waiting
            .iter()
            .any(|w| w["statement"].as_str().unwrap().contains("tabs") && w["needs"] == 1),
        "{review}"
    );

    // Repeating the rebuild is safe: nothing new, nothing rewritten.
    let (_, again) = call(
        router(&fx, ProfileSettings::default()),
        "POST",
        "/admin/profile/rebuild",
        AuthLevel::Anonymous,
    )
    .await;
    assert_eq!(again["candidates_added"], 0, "{again}");
    assert_eq!(again["entries_written"], 0, "{again}");
}

/// Multi-user: a database user is refused both routes; root is admitted
/// (control).
#[tokio::test]
async fn review_and_rebuild_are_root_only_on_a_multi_user_server() {
    let fx = fixture().await;
    fx.store
        .writer
        .create_human_user(
            NewUser {
                username: "alice".into(),
                name: None,
                email: None,
            },
            UserRole::User,
            None,
            false,
        )
        .await
        .unwrap();
    let settings = ProfileSettings {
        enabled: ProfileEnabled::On,
        share: ProfileShare::Global,
        ..ProfileSettings::default()
    };
    for (method, uri) in [
        ("POST", "/admin/profile/rebuild"),
        ("GET", "/admin/profile/review"),
    ] {
        let (status, _) = call(router(&fx, settings.clone()), method, uri, AuthLevel::User).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{method} {uri}");
        let (status, _) = call(
            router(&fx, settings.clone()),
            method,
            uri,
            AuthLevel::Anonymous,
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{method} {uri}");
        let (status, body) =
            call(router(&fx, settings.clone()), method, uri, AuthLevel::Root).await;
        assert_eq!(status, StatusCode::OK, "{method} {uri}: {body}");
    }
}

/// A rebuild on a server whose profile is off says so instead of doing work.
#[tokio::test]
async fn rebuild_refuses_when_the_profile_is_off() {
    let fx = fixture().await;
    let (status, body) = call(
        router(
            &fx,
            ProfileSettings {
                enabled: ProfileEnabled::Off,
                ..ProfileSettings::default()
            },
        ),
        "POST",
        "/admin/profile/rebuild",
        AuthLevel::Anonymous,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
}

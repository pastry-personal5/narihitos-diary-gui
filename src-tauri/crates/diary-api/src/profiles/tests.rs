use super::*;
use crate::{
    ServerState, router,
    tests::{request, state},
};
use axum::body::{Body, to_bytes};
use diary_application::agent::{AgentKind, ProfileDirectory};
use std::sync::Mutex;
use tower::ServiceExt;

const SESSION: &str = "07249746-e280-4d51-a86a-cbcb198ee101";

/// Alias "main" is Codex; reload swaps the set; checks are counted so listing is proven spawn-free.
struct Fake(Mutex<(Vec<String>, u32, Option<ProfileStatus>)>);
impl Fake {
    fn view(alias: &str, kind: AgentKind, status: Option<ProfileStatus>) -> ProfileView {
        let codex = kind == AgentKind::Codex;
        ProfileView {
            alias: alias.into(),
            kind,
            status: if codex {
                status
            } else {
                Some(ProfileStatus::Unavailable)
            },
            chat: if codex {
                Capability::Ready
            } else {
                Capability::Unavailable(CapabilityReason::AdapterUnimplemented)
            },
            generate: if codex {
                Capability::Ready
            } else {
                Capability::Unavailable(CapabilityReason::AssetsMissing)
            },
        }
    }
}
impl ProfileDirectory for Fake {
    fn list(&self) -> Vec<ProfileView> {
        let g = self.0.lock().unwrap();
        g.0.iter()
            .map(|a| {
                let kind = if a == "claude" {
                    AgentKind::Claude
                } else {
                    AgentKind::Codex
                };
                Fake::view(a, kind, g.2)
            })
            .collect()
    }
    fn check(&self, alias: &str) -> Option<ProfileView> {
        let mut g = self.0.lock().unwrap();
        let kind = if alias == "claude" {
            AgentKind::Claude
        } else {
            AgentKind::Codex
        };
        g.0.iter().any(|a| a == alias).then(|| {
            g.1 += 1;
            g.2 = Some(ProfileStatus::Ready);
            Fake::view(alias, kind, g.2)
        })
    }
}
fn setup() -> (Arc<ServerState>, Arc<Fake>) {
    let mut s = state();
    let fake = Arc::new(Fake(Mutex::new((
        vec!["main".into(), "claude".into()],
        0,
        None,
    ))));
    ServerState::set_profiles(&mut s, fake.clone());
    s.coordinator
        .lock()
        .unwrap()
        .open(SESSION, s.now())
        .unwrap();
    (s, fake)
}
fn req(method: &str, path: &str, authorized: bool) -> axum::http::Request<Body> {
    let mut r = request(method, path);
    if authorized {
        r.headers_mut()
            .insert("x-diary-session", SESSION.parse().unwrap());
    }
    r
}
async fn call(s: Arc<ServerState>, r: axum::http::Request<Body>) -> (StatusCode, Value) {
    let response = router(s).oneshot(r).await.unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), 1 << 20).await.unwrap();
    (status, serde_json::from_slice(&body).unwrap_or(Value::Null))
}

#[tokio::test]
async fn list_reports_status_and_capabilities_without_checking() {
    let (s, fake) = setup();
    let (st, v) = call(s, req("GET", "/api/v1/profiles", true)).await;
    assert_eq!(st, StatusCode::OK);
    let p = &v["profiles"];
    assert_eq!(
        (
            p[0]["alias"].as_str(),
            p[0]["type"].as_str(),
            p[0]["status"].as_str()
        ),
        (Some("main"), Some("codex"), Some("unchecked"))
    );
    assert_eq!(
        p[0]["capabilities"]["chat"],
        json!({"available":true,"reason":null})
    );
    assert_eq!(p[1]["status"], "unavailable");
    assert_eq!(
        p[1]["capabilities"]["chat"]["reason"],
        "adapter_unimplemented"
    );
    assert_eq!(p[1]["capabilities"]["generate"]["reason"], "assets_missing");
    assert_eq!(fake.0.lock().unwrap().1, 0, "listing never runs a check");
}

#[tokio::test]
async fn check_runs_one_profile_and_list_then_shows_it() {
    let (s, fake) = setup();
    let (st, v) = call(s.clone(), req("POST", "/api/v1/profiles/main/check", true)).await;
    assert_eq!((st, v["status"].as_str()), (StatusCode::OK, Some("ready")));
    assert_eq!(fake.0.lock().unwrap().1, 1);
    let (_, v) = call(s, req("GET", "/api/v1/profiles", true)).await;
    assert_eq!(v["profiles"][0]["status"], "ready");
}

#[tokio::test]
async fn unknown_alias_is_not_found() {
    let (s, _) = setup();
    let (st, v) = call(s, req("POST", "/api/v1/profiles/nope/check", true)).await;
    assert_eq!(
        (st, v["error"]["code"].as_str()),
        (StatusCode::NOT_FOUND, Some("not_found"))
    );
}

#[tokio::test]
async fn unauthenticated_requests_are_rejected() {
    let (s, fake) = setup();
    let (st, _) = call(s.clone(), req("GET", "/api/v1/profiles", false)).await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);
    let (st, _) = call(s.clone(), req("POST", "/api/v1/profiles/main/check", false)).await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);
    let mut r = req("GET", "/api/v1/profiles", true);
    r.headers_mut()
        .insert("authorization", "Bearer wrong".parse().unwrap());
    assert!(!call(s, r).await.0.is_success());
    assert_eq!(fake.0.lock().unwrap().1, 0);
}

#[tokio::test]
async fn list_follows_the_directory_after_reload() {
    let (s, fake) = setup();
    fake.0.lock().unwrap().0 = vec!["renamed".into()];
    let (_, v) = call(s.clone(), req("GET", "/api/v1/profiles", true)).await;
    assert_eq!(v["profiles"].as_array().unwrap().len(), 1);
    assert_eq!(v["profiles"][0]["alias"], "renamed");
    let (st, _) = call(s, req("POST", "/api/v1/profiles/main/check", true)).await;
    assert_eq!(st, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn nothing_starts_a_run_and_agents_stay_unavailable() {
    let (s, _) = setup();
    for (m, p) in [
        ("POST", "/api/v1/profiles/main/run"),
        ("POST", "/api/v1/profiles"),
        ("POST", "/api/v1/agents/run"),
    ] {
        let (st, _) = call(s.clone(), req(m, p, true)).await;
        assert!(
            st == StatusCode::NOT_FOUND || st == StatusCode::METHOD_NOT_ALLOWED,
            "{p}"
        );
    }
    let (_, v) = call(s, req("GET", "/api/v1/settings", true)).await;
    assert_eq!(v["agents_available"], false);
}

#[tokio::test]
async fn lease_end_stops_agent_work_by_expiry_close_and_shutdown() {
    let hits = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let count = || hits.load(std::sync::atomic::Ordering::SeqCst);
    let (mut s, _) = setup();
    let h = hits.clone();
    ServerState::set_lease_ended(
        &mut s,
        Arc::new(move || {
            h.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }),
    );
    // Not expired yet: no hook.
    s.expire_leases(s.now());
    assert_eq!(count(), 0);
    // Expiry past the lease duration.
    s.expire_leases(s.now() + diary_application::LEASE_DURATION_MS);
    assert_eq!(count(), 1);
    // Close.
    s.coordinator
        .lock()
        .unwrap()
        .open(SESSION, s.now())
        .unwrap();
    let mut r = req("DELETE", &format!("/api/v1/sessions/{SESSION}"), true);
    r.headers_mut()
        .insert("x-diary-session", SESSION.parse().unwrap());
    let (st, _) = call(s.clone(), r).await;
    assert_eq!(st, StatusCode::NO_CONTENT);
    assert_eq!(count(), 2);
    // Shutdown with a live lease.
    s.coordinator
        .lock()
        .unwrap()
        .open(SESSION, s.now())
        .unwrap();
    s.shutdown();
    assert_eq!(count(), 3);
}

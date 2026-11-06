use super::*;
use crate::tests::{request, state};
use axum::body::{Body, to_bytes};
use diary_application::{
    authentic::*,
    diagnostics::{Diagnostics, Record as LogRecord},
    imports::*,
    operations::{OperationId, Record as OperationRecord},
    source::*,
};
use diary_domain::{
    authentic::{AuthenticHeader, AuthenticRevision},
    source::{
        front_matter::Newline,
        stamp::{Dating, Instant, Precision, StampError},
        window::ImportZone,
    },
};
use std::{
    sync::atomic::{AtomicBool, AtomicUsize},
    time::Duration,
};
use tower::ServiceExt;
const SESSION: &str = "07249746-e280-4d51-a86a-cbcb198ee101";
fn date() -> DiaryDate {
    DiaryDate::parse("2026-11-06").unwrap()
}
struct Sources {
    changed: AtomicBool,
}
fn source() -> SourceListing {
    SourceListing {
        id: "source".into(),
        rel: "PRIVATE_PATH".into(),
        size: 10,
        dating: Dating::Agreed(Instant {
            unix_ms: 0,
            precision: Precision::Milli,
        }),
        token: ObservationToken {
            root_namespace: [0; 32],
            key: "key".into(),
            sha256: [0; 32],
            dev: 0,
            ino: 0,
            size: 10,
            mtime_ns: 0,
            ctime_ns: 0,
            config_generation: 1,
            import_id: Some("import".into()),
        },
        warnings: vec![],
    }
}
impl ImportStore for Sources {
    fn import_date(
        &self,
        _: &str,
        _: DiaryDate,
        _: u64,
        _: &dyn Fn() -> bool,
    ) -> Result<ImportListing, ImportError> {
        unreachable!()
    }
    fn resolve(
        &self,
        _: &str,
        _: &str,
        _: &[u8; 32],
        _: Authority,
        _: u64,
        _: &dyn Fn() -> bool,
    ) -> Result<(), ImportError> {
        unreachable!()
    }
    fn list(&self, id: &str, _: &dyn Fn() -> bool) -> Result<ImportListing, ImportError> {
        if id != "import" {
            return Err(ImportError::NotFound);
        }
        let mut unknown = source();
        unknown.id = "unknown".into();
        unknown.dating = Dating::Undated {
            header: StampError::Format,
            filename: StampError::Format,
        };
        let mut n = Navigation::default();
        n.sources.insert(
            "source".into(),
            SourceDisplay {
                group_id: "g".into(),
                header: None,
                filename: None,
                authority: None,
                included: Some(LocalTime {
                    date: date(),
                    offset_seconds: 32400,
                    datetime: "captured".into(),
                }),
            },
        );
        Ok(ImportListing {
            context: ImportContext {
                id: id.into(),
                date: date(),
                zone: ImportZone {
                    name: "Asia/Seoul".into(),
                    database_version: "2026e".into(),
                },
                config_generation: 1,
                completed_at: Some(0),
            },
            included: vec![IncludedSource {
                source: source(),
                instant: Instant {
                    unix_ms: 0,
                    precision: Precision::Milli,
                },
            }],
            warnings: vec![unknown],
            misplaced: vec![],
            scan: Default::default(),
            navigation: Some(n),
        })
    }
    fn read(
        &self,
        _: &str,
        _: &str,
        _: u64,
        _: &dyn Fn() -> bool,
    ) -> Result<SourceView, ViewError> {
        if self.changed.load(Ordering::SeqCst) {
            return Err(ViewError::Stale);
        }
        Ok(SourceView {
            listing: source(),
            body: "PRIVATE_SOURCE".into(),
            raw_front_matter: "PRIVATE_HEADER".into(),
            fields: vec![],
            newline: Newline::Lf,
            bom: false,
            write: Ok(()),
            trash: Ok(()),
        })
    }
}
struct Entry {
    present: AtomicBool,
}
impl AuthenticStore for Entry {
    fn load(&self, d: &DiaryDate) -> Result<Loaded, StoreError> {
        if !self.present.load(Ordering::SeqCst) {
            return Ok(Loaded::Virtual(*d));
        }
        Ok(Loaded::File {
            revision: AuthenticRevision {
                sequence: 1,
                sha256: "a".repeat(64),
            },
            header: Box::new(AuthenticHeader {
                id: SESSION.into(),
                date: *d,
                timezone: "Asia/Seoul".into(),
                timezone_database: "2026e".into(),
                created_at: "2026-11-06T00:00:00Z".into(),
                updated_at: "2026-11-06T00:00:00Z".into(),
                revision: 1,
                extensions: vec![],
            }),
            body: "PRIVATE_ENTRY".into(),
        })
    }
    fn list(&self, _: Option<DiaryDate>, _: usize) -> Result<Page, StoreError> {
        unreachable!()
    }
    fn save(&self, _: SaveRequest) -> Result<SaveOutcome, StoreError> {
        unreachable!()
    }
    fn create_empty(&self, _: CreateRequest) -> Result<SaveOutcome, StoreError> {
        unreachable!()
    }
    fn refresh(&self) -> Result<RebuildReport, StoreError> {
        unreachable!()
    }
    fn operation(&self, _: &OperationId) -> Result<Option<OperationRecord>, StoreError> {
        unreachable!()
    }
}
struct Profiles {
    kind: AgentKind,
    checks: AtomicUsize,
}
impl ProfileDirectory for Profiles {
    fn list(&self) -> Vec<ProfileView> {
        self.check("main").into_iter().collect()
    }
    fn check(&self, alias: &str) -> Option<ProfileView> {
        self.checks.fetch_add(1, Ordering::SeqCst);
        (alias != "none").then(|| ProfileView {
            alias: alias.into(),
            kind: self.kind,
            status: Some(if alias == "off" {
                ProfileStatus::Unavailable
            } else {
                ProfileStatus::Ready
            }),
            reason: None,
            chat: Capability::Ready,
            generate: Capability::Ready,
        })
    }
}
struct Runner {
    calls: AtomicUsize,
    block: bool,
    ack: AtomicBool,
    partial: bool,
    fail: bool,
}
impl AgentRunner for Runner {
    fn check(&self, _: &ProfileHandle) -> ProfileCheck {
        unreachable!()
    }
    fn run(
        &self,
        request: AgentRequest,
        events: &mut dyn EventSink,
        cancel: &CancelToken,
    ) -> Result<AgentOutput, AgentError> {
        let index = self.calls.fetch_add(1, Ordering::SeqCst);
        let Mode::Candidate { packet, .. } = request.mode() else {
            panic!("generation only")
        };
        assert_eq!(packet.snippets()[0].body, "PRIVATE_SOURCE");
        assert_eq!(packet.snippets()[0].id, "snippet-1");
        events.event(AgentEvent::Phase(Phase::ReceivingOutput));
        events.event(AgentEvent::Delta("PRIVATE_PREVIEW".into()));
        events.event(AgentEvent::Phase(Phase::ReceivingOutput));
        if self.block {
            let until = std::time::Instant::now() + Duration::from_secs(5);
            while (!cancel.is_cancelled() || !self.ack.load(Ordering::SeqCst))
                && std::time::Instant::now() < until
            {
                std::thread::sleep(Duration::from_millis(2));
            }
        }
        if self.fail || (self.partial && index == 1) {
            Err(AgentErrorCode::Timeout.into())
        } else {
            Ok(AgentOutput::Candidate(Candidate {
                draft_markdown: "PRIVATE_DRAFT".into(),
                claim_support: vec![Claim {
                    claim: "PRIVATE_CLAIM".into(),
                    snippet_ids: vec!["snippet-1".into()],
                }],
                warnings: vec![],
            }))
        }
    }
}
#[derive(Default)]
struct Logs(Mutex<Vec<LogRecord>>);
impl Diagnostics for Logs {
    fn emit(&self, r: LogRecord) {
        self.0.lock().unwrap().push(r)
    }
}
struct Fx {
    s: Arc<ServerState>,
    sources: Arc<Sources>,
    entry: Arc<Entry>,
    runner: Arc<Runner>,
    profiles: Arc<Profiles>,
    logs: Arc<Logs>,
}
fn setup(kind: AgentKind, block: bool, partial: bool, fail: bool) -> Fx {
    let mut s = state();
    let sources = Arc::new(Sources {
        changed: AtomicBool::new(false),
    });
    let entry = Arc::new(Entry {
        present: AtomicBool::new(false),
    });
    let profiles = Arc::new(Profiles {
        kind,
        checks: AtomicUsize::new(0),
    });
    let runner = Arc::new(Runner {
        calls: AtomicUsize::new(0),
        block,
        ack: AtomicBool::new(true),
        partial,
        fail,
    });
    let logs = Arc::new(Logs::default());
    ServerState::set_imports(&mut s, sources.clone());
    ServerState::set_authentic(&mut s, entry.clone());
    ServerState::set_profiles(&mut s, profiles.clone());
    ServerState::set_agent_runner(&mut s, runner.clone());
    ServerState::set_diagnostics(&mut s, logs.clone());
    s.coordinator
        .lock()
        .unwrap()
        .open(SESSION, s.now())
        .unwrap();
    Fx {
        s,
        sources,
        entry,
        runner,
        profiles,
        logs,
    }
}
async fn call(
    s: &Arc<ServerState>,
    method: &str,
    path: &str,
    b: Option<Value>,
) -> (StatusCode, Value) {
    let mut r = request(method, path);
    r.headers_mut()
        .insert("x-diary-session", SESSION.parse().unwrap());
    if let Some(b) = b {
        r.headers_mut()
            .insert("content-type", "application/json".parse().unwrap());
        *r.body_mut() = Body::from(b.to_string())
    }
    let response = router(s.clone()).oneshot(r).await.unwrap();
    let st = response.status();
    let bytes = to_bytes(response.into_body(), 10 << 20).await.unwrap();
    (st, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}
fn input(id: &str, count: u8) -> Value {
    json!({"request_id":id,"date":"2026-11-06","profile":"main","locale":"ko-KR","count":count,"selections":[{"import_id":"import","source_id":"source"}]})
}
async fn start(f: &Fx, id: &str, count: u8) -> String {
    let (st, v) = call(&f.s, "POST", "/api/v1/generations", Some(input(id, count))).await;
    assert_eq!(st, StatusCode::ACCEPTED, "{v}");
    v["job"]["id"].as_str().unwrap().into()
}
async fn terminal(f: &Fx, id: &str) -> Value {
    for _ in 0..500 {
        let (_, v) = call(&f.s, "GET", &format!("/api/v1/generations/{id}"), None).await;
        if matches!(
            v["job"]["state"].as_str(),
            Some("succeeded" | "failed" | "cancelled")
        ) {
            return v;
        };
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    panic!("no terminal generation")
}
#[tokio::test]
async fn manifest_has_no_bodies_progress_has_no_bodies_and_partial_candidates_survive() {
    for kind in AgentKind::ALL {
        let f = setup(kind, false, true, false);
        let id = start(&f, "r", 3).await;
        let v = terminal(&f, &id).await;
        assert_eq!(v["job"]["state"], "succeeded");
        assert_eq!(v["progress"]["ready_count"], 2);
        assert_eq!(v["progress"]["failed_count"], 1);
        assert!(!v.to_string().contains("PRIVATE"));
        assert!(v.get("provisional").is_none());
        let cid = v["candidates"][0]["id"].as_str().unwrap();
        let (st, c) = call(&f.s, "GET", &format!("/api/v1/candidates/{cid}"), None).await;
        assert_eq!(st, StatusCode::OK);
        assert_eq!(c["draft_markdown"], "PRIVATE_DRAFT");
        let (_, p) = call(&f.s, "GET", &format!("/api/v1/jobs/{id}"), None).await;
        assert_eq!(p["origin_date"], "2026-11-06");
        assert_eq!(p["locale"], "ko-KR");
        assert_eq!(p["requested_count"], 3);
        assert!(!p.to_string().contains("PRIVATE"));
        let (_, p) = call(&f.s, "GET", "/api/v1/jobs", None).await;
        assert_eq!(p["jobs"][0]["kind"], "generation");
        assert!(!p.to_string().contains("PRIVATE"));
        let events =
            f.s.coordinator
                .lock()
                .unwrap()
                .replay(SESSION, f.s.now(), "instance", 0)
                .unwrap();
        let Replay::Events(events) = events else {
            panic!()
        };
        assert_eq!(
            events
                .iter()
                .filter(|e| e.kind == diary_application::EventKind::CandidateReady)
                .count(),
            2
        );
        let logs = f.logs.0.lock().unwrap();
        assert!(!format!("{logs:?}").contains("PRIVATE"));
        assert!(
            logs.iter()
                .filter_map(|r| r.agent.as_ref())
                .any(
                    |a| a.action == Some(diary_application::diagnostics::ActionTag::Generate)
                        && a.ready == Some(2)
                )
        );
    }
}
#[tokio::test]
async fn retry_after_source_change_returns_same_job_and_retry_after_discard_is_unavailable() {
    let f = setup(AgentKind::Codex, false, false, false);
    let id = start(&f, "r", 1).await;
    terminal(&f, &id).await;
    let checks = f.profiles.checks.load(Ordering::SeqCst);
    f.sources.changed.store(true, Ordering::SeqCst);
    assert_eq!(start(&f, "r", 1).await, id);
    assert_eq!(f.runner.calls.load(Ordering::SeqCst), 1);
    assert_eq!(f.profiles.checks.load(Ordering::SeqCst), checks);
    let mut b = input("r", 1);
    b["locale"] = json!("en-US");
    assert_eq!(
        call(&f.s, "POST", "/api/v1/generations", Some(b)).await.1["error"]["code"],
        "request_mismatch"
    );
    assert_eq!(
        call(&f.s, "DELETE", &format!("/api/v1/jobs/{id}/result"), None)
            .await
            .0,
        StatusCode::OK
    );
    let (st, v) = call(&f.s, "POST", "/api/v1/generations", Some(input("r", 1))).await;
    assert_eq!(st, StatusCode::NOT_FOUND);
    assert_eq!(v["error"]["code"], "result_unavailable");
    assert_eq!(f.runner.calls.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn stale_baseline_reported_and_final_candidates_stay_readable() {
    let f = setup(AgentKind::Codex, false, false, false);
    let id = start(&f, "r", 1).await;
    let v = terminal(&f, &id).await;
    assert_eq!(v["stale_baseline"], false);
    f.entry.present.store(true, Ordering::SeqCst);
    let (_, v) = call(&f.s, "GET", &format!("/api/v1/generations/{id}"), None).await;
    assert_eq!(v["stale_baseline"], true);
    let cid = v["candidates"][0]["id"].as_str().unwrap();
    assert_eq!(
        call(&f.s, "GET", &format!("/api/v1/candidates/{cid}"), None)
            .await
            .0,
        StatusCode::OK
    );
}
#[tokio::test]
async fn refusals_and_failure_reports_a_code_and_no_text() {
    let f = setup(AgentKind::Codex, false, false, true);
    for (key, value, code) in [
        ("selections", json!([]), "empty_selection"),
        (
            "selections",
            json!([{"import_id":"import","source_id":"unknown"}]),
            "empty_selection",
        ),
        (
            "selections",
            json!([{"import_id":"import","source_id":"source"},{"import_id":"import","source_id":"source"}]),
            "invalid_input",
        ),
        ("profile", json!("off"), "profile_not_ready"),
        ("count", json!(0), "invalid_input"),
        ("extra", json!("PRIVATE_ERROR"), "invalid_input"),
    ] {
        let mut b = input("r", 1);
        b[key] = value;
        let (_, v) = call(&f.s, "POST", "/api/v1/generations", Some(b)).await;
        assert_eq!(v["error"]["code"], code, "{v}");
        assert!(!v.to_string().contains("PRIVATE"));
    }
    f.sources.changed.store(true, Ordering::SeqCst);
    assert_eq!(
        call(&f.s, "POST", "/api/v1/generations", Some(input("r", 1)))
            .await
            .1["error"]["code"],
        "reimport_required"
    );
    assert_eq!(f.runner.calls.load(Ordering::SeqCst), 0);
    f.sources.changed.store(false, Ordering::SeqCst);
    let id = start(&f, "r", 1).await;
    let v = terminal(&f, &id).await;
    assert_eq!(v["job"]["state"], "failed");
    assert_eq!(v["progress"]["error_code"], "timeout");
    assert!(!v.to_string().contains("PRIVATE"));
}
#[tokio::test]
async fn cancel_preserves_busy_until_acknowledged_and_drops_late_success() {
    let f = setup(AgentKind::Codex, true, false, false);
    f.runner.ack.store(false, Ordering::SeqCst);
    let id = start(&f, "r", 3).await;
    for _ in 0..300 {
        if f.runner.calls.load(Ordering::SeqCst) > 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    let (_, p) = call(&f.s, "GET", &format!("/api/v1/jobs/{id}/result"), None).await;
    assert_eq!(p["provisional"], "PRIVATE_PREVIEW");
    let (_, v) = call(&f.s, "POST", &format!("/api/v1/jobs/{id}/cancel"), None).await;
    assert_eq!(v["state"], "running");
    assert_eq!(v["cancellation_requested"], true);
    assert_eq!(
        call(&f.s, "POST", "/api/v1/generations", Some(input("other", 1)))
            .await
            .1["error"]["code"],
        "busy"
    );
    assert_eq!(
        call(&f.s, "DELETE", &format!("/api/v1/generations/{id}"), None)
            .await
            .1["error"]["code"],
        "busy"
    );
    f.runner.ack.store(true, Ordering::SeqCst);
    let v = terminal(&f, &id).await;
    assert_eq!(v["job"]["state"], "cancelled");
    assert_eq!(v["progress"]["cancelled_count"], 1);
    assert_eq!(v["progress"]["not_started_count"], 2);
    assert_eq!(v["progress"]["ready_count"], 0);
}
#[tokio::test]
async fn close_expiry_shutdown_clear_transient_generations() {
    for reason in 0..4 {
        let f = setup(AgentKind::Codex, true, false, false);
        let id = start(&f, "r", 1).await;
        let g = f.s.generations.lock().unwrap().get(SESSION, &id).unwrap();
        match reason {
            0 => {
                assert_eq!(
                    call(&f.s, "DELETE", &format!("/api/v1/sessions/{SESSION}"), None)
                        .await
                        .0,
                    StatusCode::NO_CONTENT
                );
            }
            1 => {
                f.s.expire_leases(f.s.now() + diary_application::LEASE_DURATION_MS);
            }
            2 => {
                f.s.shutdown();
            }
            _ => {
                // A job read may discover expiry before the sweep does.
                assert!(
                    f.s.coordinator
                        .lock()
                        .unwrap()
                        .get(
                            SESSION,
                            f.s.now() + diary_application::LEASE_DURATION_MS,
                            &id
                        )
                        .is_err()
                );
                f.s.expire_leases(f.s.now());
            }
        }
        assert!(f.s.generations.lock().unwrap().get(SESSION, &id).is_none());
        for _ in 0..300 {
            if g.lock().unwrap().terminal.is_some() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
        let g = g.lock().unwrap();
        assert_eq!(g.terminal, Some(JobState::Cancelled));
        assert_eq!(g.counts().ready, 0);
    }
}

#[tokio::test]
async fn result_ownership_and_store_limit_require_explicit_discard() {
    let f = setup(AgentKind::Codex, false, false, false);
    let mut first = String::new();
    for i in 0..5 {
        let id = start(&f, &format!("r{i}"), 1).await;
        terminal(&f, &id).await;
        if i == 0 {
            first = id;
        }
    }
    let (st, v) = call(
        &f.s,
        "POST",
        "/api/v1/generations",
        Some(input("overflow", 1)),
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT);
    assert_eq!(v["error"]["code"], "discard_required");
    assert_eq!(f.runner.calls.load(Ordering::SeqCst), 5);
    let mut r = request("GET", &format!("/api/v1/candidates/{first}-1"));
    r.headers_mut().insert(
        "x-diary-session",
        "07249746-e280-4d51-a86a-cbcb198ee102".parse().unwrap(),
    );
    let response = router(f.s.clone()).oneshot(r).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        call(
            &f.s,
            "DELETE",
            &format!("/api/v1/generations/{first}"),
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    let id = start(&f, "overflow", 1).await;
    terminal(&f, &id).await;
    assert_eq!(f.runner.calls.load(Ordering::SeqCst), 6);
}

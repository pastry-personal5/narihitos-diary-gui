use super::*;
use axum::{
    body::{Body, to_bytes},
    http::Request as HttpRequest,
};
use tower::ServiceExt;

pub(crate) fn state() -> Arc<ServerState> {
    ServerState::new(
        "a".repeat(64),
        8781,
        "instance".into(),
        "workspace".into(),
        false,
    )
}
pub(crate) fn request(method: &str, path: &str) -> HttpRequest<Body> {
    let mut req = HttpRequest::builder()
        .method(method)
        .uri(path)
        .header("host", "127.0.0.1:8781")
        .header("authorization", format!("Bearer {}", "a".repeat(64)))
        .body(Body::empty())
        .unwrap();
    req.extensions_mut().insert(ConnectInfo(
        "127.0.0.1:40000".parse::<SocketAddr>().unwrap(),
    ));
    req
}
#[tokio::test]
async fn access_boundary_rejects_host_origin_peer_token_and_query() {
    let app = router(state());
    for (key, value) in [
        ("host", "localhost:8781"),
        ("origin", "null"),
        ("origin", "*"),
        ("origin", "http://localhost:1420"),
        ("authorization", "Bearer wrong"),
        ("forwarded", "host=evil"),
        ("x-forwarded-host", "evil"),
    ] {
        let mut req = request("GET", "/api/v1/status");
        req.headers_mut().insert(
            axum::http::HeaderName::from_bytes(key.as_bytes()).unwrap(),
            value.parse().unwrap(),
        );
        assert!(
            !app.clone()
                .oneshot(req)
                .await
                .unwrap()
                .status()
                .is_success()
        );
    }
    let mut req = request("GET", "/api/v1/status");
    req.extensions_mut().insert(ConnectInfo(
        "192.0.2.1:40000".parse::<SocketAddr>().unwrap(),
    ));
    assert_eq!(
        app.clone().oneshot(req).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        app.clone()
            .oneshot(request("GET", "/api/v1/status?token=secret"))
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    let response = app.oneshot(request("GET", "/api/v1/status")).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 10240).await.unwrap();
    assert!(!String::from_utf8_lossy(&body).contains(&"a".repeat(64)));
    let value: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(value["storage_ready"], false);
}
#[tokio::test]
async fn session_admission_excludes_competitor_and_reconnects_same_lease() {
    let app = router(state());
    let mut req = request("POST", "/api/v1/sessions");
    req.headers_mut()
        .insert("content-type", "application/json".parse().unwrap());
    *req.body_mut() = Body::from("{}");
    let response = app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let value: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 10240).await.unwrap()).unwrap();
    let id = value["session_id"].as_str().unwrap();
    for (payload, expected) in [
        ("{}".to_owned(), StatusCode::CONFLICT),
        (format!("{{\"session_id\":\"{id}\"}}"), StatusCode::OK),
    ] {
        let mut req = request("POST", "/api/v1/sessions");
        req.headers_mut()
            .insert("content-type", "application/json".parse().unwrap());
        *req.body_mut() = Body::from(payload);
        assert_eq!(app.clone().oneshot(req).await.unwrap().status(), expected);
    }
    let mut req = request("GET", "/api/v1/jobs");
    req.headers_mut()
        .insert("x-diary-session", id.parse().unwrap());
    assert_eq!(
        app.clone().oneshot(req).await.unwrap().status(),
        StatusCode::OK
    );
    assert_eq!(
        app.oneshot(request("GET", "/api/v1/jobs"))
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
}
#[tokio::test]
async fn invalid_json_is_redacted_and_fake_admission_route_is_absent() {
    let app = router(state());
    let mut req = request("POST", "/api/v1/sessions");
    req.headers_mut()
        .insert("content-type", "application/json".parse().unwrap());
    *req.body_mut() = Body::from("{\"secret-value\":true}");
    let response = app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(response.into_body(), 10240).await.unwrap();
    assert!(!String::from_utf8_lossy(&body).contains("secret-value"));
    assert_eq!(
        app.oneshot(request("POST", "/api/v1/fake-jobs"))
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
}

const SESSION: &str = "e64575a5-72fe-44af-86b0-21f5bd247aac";
fn seeded() -> Arc<ServerState> {
    let mut s = state();
    let inner = Arc::get_mut(&mut s).unwrap();
    inner.clock = Arc::new(|| 0);
    inner.coordinator = Mutex::new(Coordinator::new(
        "instance".into(),
        Limits {
            replay_events: 2,
            ..Limits::default()
        },
    ));
    let c = inner.coordinator.get_mut().unwrap();
    c.open(SESSION, 0).unwrap();
    c.admit(SESSION, 0, "request", &"a".repeat(64), "job", 100)
        .unwrap();
    s
}
fn owned_request(method: &str, path: &str) -> HttpRequest<Body> {
    let mut req = request(method, path);
    req.headers_mut()
        .insert("x-diary-session", SESSION.parse().unwrap());
    req
}
#[tokio::test]
async fn fake_job_cancellation_is_visible_before_sse_notification() {
    use futures_util::StreamExt;
    let s = seeded();
    let app = router(s.clone());
    let response = app
        .clone()
        .oneshot(owned_request("POST", "/api/v1/jobs/job/cancel"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let response = app
        .clone()
        .oneshot(owned_request("GET", "/api/v1/jobs/job"))
        .await
        .unwrap();
    let value: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 10240).await.unwrap()).unwrap();
    assert_eq!(value["state"], "cancelled");
    let response = app
        .oneshot(owned_request("GET", "/api/v1/events"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let mut stream = response.into_body().into_data_stream();
    let frame = tokio::time::timeout(Duration::from_secs(1), stream.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let text = String::from_utf8(frame.to_vec()).unwrap();
    assert!(text.contains("job_changed") && text.contains("cancelled"));
    assert!(!text.contains(&"a".repeat(64)));
}
#[tokio::test]
async fn watcher_hint_is_one_pathless_event_per_open_stream() {
    use futures_util::StreamExt;
    let s = seeded();
    let response = router(s.clone())
        .oneshot(owned_request("GET", "/api/v1/events"))
        .await
        .unwrap();
    let mut stream = response.into_body().into_data_stream();
    s.authentic_changed();
    s.authentic_changed();
    let frame = tokio::time::timeout(Duration::from_secs(1), stream.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(
        String::from_utf8(frame.to_vec()).unwrap(),
        "event: authentic_changed\ndata: {}\n\n"
    );
    // Two bumps before the next poll still announce a single change.
    assert!(
        tokio::time::timeout(Duration::from_millis(300), stream.next())
            .await
            .is_err()
    );
}
#[tokio::test]
async fn lost_gap_notification_still_forces_resync_on_reconnection() {
    let s = seeded();
    {
        let mut c = s.coordinator.lock().unwrap();
        c.publish(SESSION, 0, "job", 400).unwrap();
        c.transition(SESSION, 0, "job", diary_application::JobState::Running)
            .unwrap();
        c.publish(SESSION, 0, "job", 400).unwrap();
        c.cancel(SESSION, 0, "job").unwrap();
        c.publish(SESSION, 0, "job", 400).unwrap();
    }
    let app = router(s);
    let response = app
        .oneshot(owned_request("GET", "/api/v1/events"))
        .await
        .unwrap();
    let data = tokio::time::timeout(
        Duration::from_secs(1),
        to_bytes(response.into_body(), 10240),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(String::from_utf8_lossy(&data).contains("resync_required"));
}
#[tokio::test]
async fn expiration_and_shutdown_close_streams_without_waiting_for_heartbeat() {
    use std::sync::atomic::{AtomicU64, Ordering};
    let clock = Arc::new(AtomicU64::new(0));
    let mut s = seeded();
    let captured = clock.clone();
    Arc::get_mut(&mut s).unwrap().clock = Arc::new(move || captured.load(Ordering::Acquire));
    let app = router(s.clone());
    clock.store(30000, Ordering::Release);
    assert_eq!(
        app.clone()
            .oneshot(owned_request("GET", "/api/v1/jobs"))
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    s.shutdown();
    assert_eq!(
        app.oneshot(request("GET", "/api/v1/status"))
            .await
            .unwrap()
            .status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
}
#[tokio::test]
async fn pagination_cursor_is_bounded_and_query_credentials_are_rejected() {
    let s = seeded();
    let app = router(s);
    let response = app
        .clone()
        .oneshot(owned_request("GET", "/api/v1/jobs?cursor=job"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let value: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 10240).await.unwrap()).unwrap();
    assert_eq!(value["jobs"], json!([]));
    for path in [
        "/api/v1/jobs?cursor=job&token=secret",
        "/api/v1/jobs?unknown=x",
        "/api/v1/jobs?cursor=%2F",
    ] {
        assert_eq!(
            app.clone()
                .oneshot(owned_request("GET", path))
                .await
                .unwrap()
                .status(),
            StatusCode::FORBIDDEN
        );
    }
}

#[tokio::test]
async fn configured_unavailable_profiles_have_redacted_reasons() {
    let mut s = state();
    ServerState::set_startup(
        &mut s,
        diary_application::StartupSummary {
            sources: vec![
                diary_application::Availability {
                    alias: "source".into(),
                    reason: Some(diary_application::UnavailableReason::SourceRootMissing),
                },
                diary_application::Availability {
                    alias: "mobile".into(),
                    reason: None,
                },
            ],
            profiles: vec![diary_application::Availability {
                alias: "cli".into(),
                reason: Some(diary_application::UnavailableReason::ExecutableMissing),
            }],
        },
    );
    s.coordinator
        .lock()
        .unwrap()
        .open(SESSION, s.now())
        .unwrap();
    let response = router(s)
        .oneshot(owned_request("GET", "/api/v1/settings"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let value: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 10240).await.unwrap()).unwrap();
    assert_eq!(
        value["sources"],
        json!([
            {"alias":"source","available":false,"reason":"source_root_missing"},
            {"alias":"mobile","available":true,"reason":"ready"}
        ])
    );
    assert_eq!(value["sources_available"], json!(true));
    assert_eq!(
        value["profiles"],
        json!([{"alias":"cli","available":false,"reason":"executable_missing"}])
    );
    assert_eq!(value["agents_available"], json!(false));
    assert_eq!(
        value.as_object().unwrap().len(),
        if cfg!(feature = "scaffold-smoke") {
            11
        } else {
            10
        }
    );
}

#[tokio::test]
async fn slow_consumers_have_bounded_slots_and_do_not_block_controls() {
    let s = seeded();
    let app = router(s);
    let mut held = Vec::new();
    for _ in 0..4 {
        let response = app
            .clone()
            .oneshot(owned_request("GET", "/api/v1/events"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        held.push(response);
    }
    assert_eq!(
        app.clone()
            .oneshot(owned_request("GET", "/api/v1/events"))
            .await
            .unwrap()
            .status(),
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(
        app.clone()
            .oneshot(request("GET", "/api/v1/status"))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        app.clone()
            .oneshot(owned_request("POST", "/api/v1/jobs/job/cancel"))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        app.clone()
            .oneshot(owned_request(
                "POST",
                &format!("/api/v1/sessions/{SESSION}/heartbeat")
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );
    held.pop();
    assert_eq!(
        app.oneshot(owned_request("GET", "/api/v1/events"))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
}
#[tokio::test]
async fn repeated_cancellation_preserves_notification_watermark() {
    let s = seeded();
    let app = router(s.clone());
    for _ in 0..3 {
        assert_eq!(
            app.clone()
                .oneshot(owned_request("POST", "/api/v1/jobs/job/cancel"))
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
    }
    assert_eq!(s.coordinator.lock().unwrap().event_cursor(), 1);
}

#[tokio::test]
async fn all_authenticated_routes_enforce_body_bounds_and_redacted_media_errors() {
    let app = router(state());
    for (method, path) in [("GET", "/api/v1/status"), ("POST", "/api/v1/sessions")] {
        let mut req = request(method, path);
        req.headers_mut()
            .insert("content-type", "application/json".parse().unwrap());
        *req.body_mut() = Body::from(vec![b' '; 10 * 1024 * 1024 + 1]);
        let response = app.clone().oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        let body = to_bytes(response.into_body(), 10240).await.unwrap();
        assert!(String::from_utf8_lossy(&body).contains("request_limit"));
    }
    assert_eq!(
        app.oneshot(request("POST", "/api/v1/sessions"))
            .await
            .unwrap()
            .status(),
        StatusCode::UNSUPPORTED_MEDIA_TYPE
    );
}
#[tokio::test(start_paused = true)]
async fn stalled_request_bodies_have_a_deadline_without_blocking_status() {
    let app = router(state());
    let mut stalled = request("POST", "/api/v1/sessions");
    stalled
        .headers_mut()
        .insert("content-type", "application/json".parse().unwrap());
    *stalled.body_mut() = Body::from_stream(futures_util::stream::pending::<
        Result<axum::body::Bytes, std::io::Error>,
    >());
    let pending = tokio::spawn(app.clone().oneshot(stalled));
    assert_eq!(
        app.oneshot(request("GET", "/api/v1/status"))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        pending.await.unwrap().unwrap().status(),
        StatusCode::REQUEST_TIMEOUT
    );
}

#[tokio::test]
async fn missing_or_wrong_token_is_rejected_for_status_and_event_stream() {
    let app = router(seeded());
    for path in ["/api/v1/status", "/api/v1/events"] {
        for replacement in [None, Some("Bearer wrong")] {
            let mut req = owned_request("GET", path);
            req.headers_mut().remove("authorization");
            if let Some(value) = replacement {
                req.headers_mut()
                    .insert("authorization", value.parse().unwrap());
            }
            assert_eq!(
                app.clone().oneshot(req).await.unwrap().status(),
                StatusCode::UNAUTHORIZED,
                "{path} {replacement:?}"
            );
        }
    }
}

mod diagnostics {
    use super::*;
    use diary_application::diagnostics::{
        Diagnostics, ErrorTag, EventCode, Level, OpaqueId, Record, TraceId,
    };

    #[derive(Default)]
    struct Recorder(Mutex<Vec<Record>>);
    impl Diagnostics for Recorder {
        fn emit(&self, record: Record) {
            self.0.lock().unwrap().push(record);
        }
    }
    impl Recorder {
        fn all(&self) -> Vec<Record> {
            self.0.lock().unwrap().clone()
        }
        fn find(&self, code: EventCode) -> Option<Record> {
            self.all().into_iter().find(|r| r.code == code)
        }
    }
    const TRACE: &str = "0123456789abcdef0123456789abcdef";
    const JOB: &str = "123e4567-e89b-42d3-a456-426614174000";

    fn recorded(mut s: Arc<ServerState>) -> (Arc<ServerState>, Arc<Recorder>) {
        let rec = Arc::new(Recorder::default());
        ServerState::set_diagnostics(&mut s, rec.clone());
        (s, rec)
    }
    fn open_request(trace: Option<&str>) -> HttpRequest<Body> {
        let mut req = request("POST", "/api/v1/sessions");
        req.headers_mut()
            .insert("content-type", "application/json".parse().unwrap());
        if let Some(trace) = trace {
            req.headers_mut()
                .insert("x-diary-trace-id", trace.parse().unwrap());
        }
        *req.body_mut() = Body::from("{}");
        req
    }

    #[tokio::test]
    async fn valid_trace_header_is_echoed_and_recorded() {
        let (s, rec) = recorded(state());
        let response = router(s).oneshot(open_request(Some(TRACE))).await.unwrap();
        assert_eq!(response.headers()["x-diary-trace-id"], TRACE);
        let opened = rec.find(EventCode::SessionOpened).unwrap();
        assert_eq!(opened.trace.unwrap().as_str(), TRACE);
        assert!(opened.session.is_some());
    }

    #[tokio::test]
    async fn invalid_or_missing_trace_header_is_replaced() {
        for bad in [
            Some("short"),
            Some(&"a".repeat(33)[..]),
            Some(&"A".repeat(32)[..]),
            Some("0123456789abcdef0123456789abcde!"),
            None,
        ] {
            let (s, rec) = recorded(state());
            let response = router(s).oneshot(open_request(bad)).await.unwrap();
            let got = response.headers()["x-diary-trace-id"].to_str().unwrap();
            assert_ne!(Some(got), bad);
            assert!(TraceId::parse(got).is_some(), "{got}");
            let recorded = rec.find(EventCode::SessionOpened).unwrap().trace.unwrap();
            assert_eq!(recorded.as_str(), got);
        }
    }

    #[tokio::test]
    async fn auth_rejection_emits_one_record_without_token_or_body() {
        let (s, rec) = recorded(state());
        let mut req = request("POST", "/api/v1/sessions");
        req.headers_mut()
            .insert("authorization", "Bearer wrong-token".parse().unwrap());
        req.headers_mut()
            .insert("content-type", "application/json".parse().unwrap());
        *req.body_mut() = Body::from("{\"secret\":\"SECRET-BODY\"}");
        let response = router(s).oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let all = rec.all();
        assert_eq!(all.len(), 1, "{all:?}");
        assert_eq!(all[0].code, EventCode::AuthRejected);
        assert_eq!(all[0].level, Level::Warn);
        assert_eq!(all[0].error, Some(ErrorTag::Denied));
        assert!(all[0].trace.is_some());
        let dump = format!("{all:?}");
        for secret in ["SECRET-BODY", "wrong-token", &"a".repeat(64)] {
            assert!(!dump.contains(secret));
        }
    }

    #[tokio::test]
    async fn session_open_busy_and_close_emit_events() {
        let (s, rec) = recorded(state());
        let app = router(s);
        let response = app.clone().oneshot(open_request(None)).await.unwrap();
        let value: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 10240).await.unwrap()).unwrap();
        let id = value["session_id"].as_str().unwrap().to_owned();
        let busy = app.clone().oneshot(open_request(None)).await.unwrap();
        assert_eq!(busy.status(), StatusCode::CONFLICT);
        let mut close = request("DELETE", &format!("/api/v1/sessions/{id}"));
        close
            .headers_mut()
            .insert("x-diary-session", id.parse().unwrap());
        assert_eq!(
            app.oneshot(close).await.unwrap().status(),
            StatusCode::NO_CONTENT
        );
        let busy = rec.find(EventCode::ClientBusy).unwrap();
        assert_eq!(
            (busy.level, busy.error),
            (Level::Warn, Some(ErrorTag::Busy))
        );
        let closed = rec.find(EventCode::SessionClosed).unwrap();
        assert_eq!(closed.session, OpaqueId::parse(&id));
    }

    #[tokio::test]
    async fn repeated_cancel_emits_job_finished_only_once() {
        let mut s = state();
        let inner = Arc::get_mut(&mut s).unwrap();
        inner.clock = Arc::new(|| 0);
        let c = inner.coordinator.get_mut().unwrap();
        c.open(SESSION, 0).unwrap();
        c.admit(SESSION, 0, "request", &"a".repeat(64), JOB, 100)
            .unwrap();
        let (s, rec) = recorded(s);
        let app = router(s);
        for _ in 0..3 {
            let response = app
                .clone()
                .oneshot(owned_request("POST", &format!("/api/v1/jobs/{JOB}/cancel")))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
        }
        let finished = rec
            .all()
            .into_iter()
            .filter(|r| r.code == EventCode::JobFinished)
            .count();
        assert_eq!(finished, 1);
    }

    #[tokio::test]
    async fn cancel_emits_job_finished_with_the_job_id() {
        let mut s = state();
        let inner = Arc::get_mut(&mut s).unwrap();
        inner.clock = Arc::new(|| 0);
        let c = inner.coordinator.get_mut().unwrap();
        c.open(SESSION, 0).unwrap();
        c.admit(SESSION, 0, "request", &"a".repeat(64), JOB, 100)
            .unwrap();
        let (s, rec) = recorded(s);
        let response = router(s)
            .oneshot(owned_request("POST", &format!("/api/v1/jobs/{JOB}/cancel")))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let finished = rec.find(EventCode::JobFinished).unwrap();
        assert_eq!(finished.job, OpaqueId::parse(JOB));
        assert!(finished.trace.is_some());
    }
}

#[tokio::test]
async fn storage_ready_follows_workspace_readiness_in_status_and_settings() {
    for ready in [false, true] {
        let mut s = state();
        ServerState::set_storage_ready(&mut s, ready);
        s.coordinator
            .lock()
            .unwrap()
            .open(SESSION, s.now())
            .unwrap();
        for path in ["/api/v1/status", "/api/v1/settings"] {
            let response = router(s.clone())
                .oneshot(owned_request("GET", path))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK, "{path}");
            let value: Value =
                serde_json::from_slice(&to_bytes(response.into_body(), 20480).await.unwrap())
                    .unwrap();
            assert_eq!(value["storage_ready"], ready, "{path}");
        }
    }
}

mod authentic_routes {
    use super::*;
    use diary_application::{
        authentic::{
            AuthenticStore, CatalogRow, ConflictKind, CreateRequest, Loaded, Page, RebuildReport,
            SaveOutcome, SaveRequest, StoreError,
        },
        operations::{Identity, OperationId, OperationState, Record as OpRecord},
    };
    use diary_domain::authentic::{AuthenticHeader, AuthenticRevision, DiaryDate, FormatError};

    const OP: &str = "00000000-0000-4000-8000-000000000001";
    const SHA: &str = "0000000000000000000000000000000000000000000000000000000000000000";

    #[derive(Default)]
    struct Fake {
        fail: Mutex<Option<StoreError>>,
        saved: Mutex<Vec<SaveRequest>>,
    }
    fn revision() -> AuthenticRevision {
        AuthenticRevision {
            sequence: 3,
            sha256: SHA.into(),
        }
    }
    impl Fake {
        fn check(&self) -> Result<(), StoreError> {
            self.fail.lock().unwrap().take().map_or(Ok(()), Err)
        }
    }
    impl AuthenticStore for Fake {
        fn load(&self, date: &DiaryDate) -> Result<Loaded, StoreError> {
            self.check()?;
            if date.to_string() == "2026-11-06" {
                return Ok(Loaded::Virtual(*date));
            }
            Ok(Loaded::File {
                revision: revision(),
                header: Box::new(AuthenticHeader {
                    id: OP.into(),
                    date: *date,
                    timezone: "UTC".into(),
                    timezone_database: "2026e".into(),
                    created_at: "2026-11-05T10:00:00Z".into(),
                    updated_at: "2026-11-05T11:00:00Z".into(),
                    revision: 3,
                    extensions: vec![],
                }),
                body: "hello".into(),
            })
        }
        fn list(&self, after: Option<DiaryDate>, limit: usize) -> Result<Page, StoreError> {
            self.check()?;
            assert_eq!(limit, 200);
            Ok(Page {
                rows: vec![CatalogRow {
                    date: DiaryDate::parse("2026-11-05").unwrap(),
                    revision: Some(revision()),
                    problem: None,
                }],
                next: after.map(|_| DiaryDate::parse("2026-11-06").unwrap()),
            })
        }
        fn save(&self, r: SaveRequest) -> Result<SaveOutcome, StoreError> {
            self.check()?;
            let out = SaveOutcome {
                operation_id: r.operation_id.clone(),
                revision: revision(),
                buffer_generation: r.buffer_generation,
            };
            self.saved.lock().unwrap().push(r);
            Ok(out)
        }
        fn create_empty(&self, r: CreateRequest) -> Result<SaveOutcome, StoreError> {
            self.check()?;
            Ok(SaveOutcome {
                operation_id: r.operation_id,
                revision: revision(),
                buffer_generation: 0,
            })
        }
        fn refresh(&self) -> Result<RebuildReport, StoreError> {
            self.check()?;
            Ok(RebuildReport {
                files: 4,
                problems: 1,
            })
        }
        fn operation(&self, id: &OperationId) -> Result<Option<OpRecord>, StoreError> {
            self.check()?;
            Ok((id.as_str() == OP).then(|| OpRecord {
                id: id.clone(),
                kind: "authentic_save".into(),
                hash_version: 1,
                input_hash: SHA.into(),
                state: OperationState::Succeeded,
                checkpoint: 1,
                result: Some(format!("3:{SHA}")),
                identity: Identity {
                    workspace_id: "w".into(),
                    root: "/private/root".into(),
                    native: "1:2".into(),
                    adapter_version: 1,
                },
            }))
        }
    }

    fn with_store() -> (Arc<ServerState>, Arc<Fake>) {
        let fake = Arc::new(Fake::default());
        let mut s = state();
        ServerState::set_authentic(&mut s, fake.clone());
        s.coordinator
            .lock()
            .unwrap()
            .open(SESSION, s.now())
            .unwrap();
        (s, fake)
    }
    async fn call(s: &Arc<ServerState>, req: HttpRequest<Body>) -> (StatusCode, Value) {
        let response = router(s.clone()).oneshot(req).await.unwrap();
        let status = response.status();
        let bytes = to_bytes(response.into_body(), 1 << 20).await.unwrap();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }
    fn json_request(method: &str, path: &str, body: Value) -> HttpRequest<Body> {
        let mut req = owned_request(method, path);
        *req.body_mut() = Body::from(body.to_string());
        req.headers_mut()
            .insert("content-type", "application/json".parse().unwrap());
        req
    }

    #[tokio::test]
    async fn every_route_needs_an_admitted_session() {
        let (s, _) = with_store();
        for (method, path) in [
            ("GET", "/api/v1/authentic"),
            ("GET", "/api/v1/authentic/2026-11-05"),
            ("PUT", "/api/v1/authentic/2026-11-05"),
            ("POST", "/api/v1/authentic/2026-11-05/empty"),
            ("POST", "/api/v1/authentic/refresh"),
            (
                "GET",
                "/api/v1/operations/00000000-0000-4000-8000-000000000001",
            ),
        ] {
            let (status, body) = call(&s, request(method, path)).await;
            assert_eq!(status, StatusCode::UNAUTHORIZED, "{method} {path}");
            assert_eq!(body["error"]["code"], "session_expired");
        }
    }

    #[tokio::test]
    async fn without_a_store_every_route_reports_storage_unavailable() {
        let mut s = state();
        s.coordinator
            .lock()
            .unwrap()
            .open(SESSION, s.now())
            .unwrap();
        let (status, body) = call(&s, owned_request("GET", "/api/v1/authentic/2026-11-05")).await;
        assert_eq!(
            (status, &body["error"]["code"]),
            (
                StatusCode::SERVICE_UNAVAILABLE,
                &json!("storage_unavailable")
            )
        );
        let _ = &mut s;
    }

    #[tokio::test]
    async fn list_load_save_create_refresh_and_operation_round_trip() {
        let (s, fake) = with_store();
        let (status, body) = call(
            &s,
            owned_request("GET", "/api/v1/authentic?cursor=2026-11-01"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["items"][0]["revision"]["sequence"], 3);
        assert_eq!(body["next_cursor"], "2026-11-06");
        let (_, body) = call(&s, owned_request("GET", "/api/v1/authentic/2026-11-05")).await;
        assert_eq!(
            (body["exists"].clone(), body["body"].clone()),
            (json!(true), json!("hello"))
        );
        let (_, body) = call(&s, owned_request("GET", "/api/v1/authentic/2026-11-06")).await;
        assert_eq!(
            (body["exists"].clone(), body["revision"].clone()),
            (json!(false), Value::Null)
        );

        let (status, body) = call(
            &s,
            json_request(
                "PUT",
                "/api/v1/authentic/2026-11-05",
                json!({
                    "operation_id": OP, "expected_revision": {"sequence": 2, "sha256": SHA},
                    "body": "text", "buffer_generation": 9,
                }),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            (
                body["buffer_generation"].clone(),
                body["revision"]["sequence"].clone()
            ),
            (json!(9), json!(3))
        );
        let sent = fake.saved.lock().unwrap().remove(0);
        assert_eq!(
            (
                sent.body.as_str(),
                sent.expected.unwrap().sequence,
                sent.date.to_string().as_str()
            ),
            ("text", 2, "2026-11-05")
        );

        let (status, _) = call(
            &s,
            json_request(
                "POST",
                "/api/v1/authentic/2026-11-05/empty",
                json!({"operation_id": OP}),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let (_, body) = call(&s, owned_request("POST", "/api/v1/authentic/refresh")).await;
        assert_eq!(
            (body["files"].clone(), body["problems"].clone()),
            (json!(4), json!(1))
        );

        let path = format!("/api/v1/operations/{OP}");
        let (status, body) = call(&s, owned_request("GET", &path)).await;
        assert_eq!(
            (status, body["state"].clone(), body["succeeded"].clone()),
            (StatusCode::OK, json!("succeeded"), json!(true))
        );
        assert!(
            !body.to_string().contains("/private/root"),
            "no path in operation DTO"
        );
        let (status, body) = call(
            &s,
            owned_request(
                "GET",
                "/api/v1/operations/00000000-0000-4000-8000-0000000000ff",
            ),
        )
        .await;
        assert_eq!(
            (status, &body["error"]["code"]),
            (StatusCode::NOT_FOUND, &json!("not_found"))
        );
    }

    #[tokio::test]
    async fn malformed_requests_are_rejected_before_the_store() {
        let (s, fake) = with_store();
        let good = json!({"operation_id": OP, "expected_revision": null, "body": "x", "buffer_generation": 1});
        let mut cases: Vec<(&str, Value)> = vec![
            ("/api/v1/authentic/2026-02-30", good.clone()),
            ("/api/v1/authentic/not-a-date", good.clone()),
        ];
        for (field, value) in [
            ("operation_id", json!("nope")),
            ("expected_revision", json!({"sequence": 1, "sha256": "ABC"})),
            ("extra", json!(1)),
            ("buffer_generation", json!(-1)),
        ] {
            let mut bad = good.clone();
            bad[field] = value;
            cases.push(("/api/v1/authentic/2026-11-05", bad));
        }
        for (path, body) in cases {
            let (status, _) = call(&s, json_request("PUT", path, body.clone())).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{path} {body}");
        }
        assert!(fake.saved.lock().unwrap().is_empty());
        let (status, _) = call(
            &s,
            owned_request("GET", "/api/v1/authentic?cursor=2026-11-01&x=1"),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        let (status, _) = call(&s, owned_request("GET", "/api/v1/authentic?cursor=%2F")).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn store_errors_map_to_stable_codes_without_leaking_content() {
        let table = [
            (
                StoreError::Conflict(ConflictKind::Stale),
                409,
                "revision_conflict",
            ),
            (
                StoreError::Conflict(ConflictKind::Exists),
                409,
                "already_exists",
            ),
            (
                StoreError::Conflict(ConflictKind::External),
                409,
                "external_change",
            ),
            (StoreError::Reserved, 409, "resource_reserved"),
            (StoreError::Mismatch, 409, "operation_mismatch"),
            (
                StoreError::Failed("stage_missing".into()),
                409,
                "operation_failed",
            ),
            (StoreError::Uncertain, 503, "outcome_uncertain"),
            (
                StoreError::Blocked("duplicate id".into()),
                409,
                "date_blocked",
            ),
            (
                StoreError::Format(FormatError::BodyTooLarge),
                413,
                "request_limit",
            ),
            (
                StoreError::Format(FormatError::CarriageReturn),
                422,
                "invalid_content",
            ),
            (StoreError::Coordination, 503, "coordination_failed"),
            (StoreError::Unavailable, 503, "storage_unavailable"),
            (StoreError::Storage, 500, "storage_error"),
        ];
        let (s, fake) = with_store();
        for (error, status, code) in table {
            *fake.fail.lock().unwrap() = Some(error);
            let (got, body) = call(&s, json_request("PUT", "/api/v1/authentic/2026-11-05", json!({
                "operation_id": OP, "expected_revision": null, "body": "private diary text", "buffer_generation": 1,
            }))).await;
            assert_eq!(
                (got.as_u16(), body["error"]["code"].as_str()),
                (status, Some(code))
            );
            assert!(!body.to_string().contains("private diary text"));
        }
    }
}

struct FakeReloader(Mutex<Vec<bool>>, fn() -> diary_application::ReloadOutcome);
impl diary_application::ConfigReloader for FakeReloader {
    fn reload(&self, busy: bool) -> diary_application::ReloadOutcome {
        self.0.lock().unwrap().push(busy);
        (self.1)()
    }
}
async fn json_of(response: Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), 10240).await.unwrap()).unwrap()
}

#[tokio::test]
async fn reload_without_a_reloader_is_unavailable() {
    let response = router(seeded())
        .oneshot(owned_request("POST", "/api/v1/settings/reload"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn applied_reload_bumps_generation_swaps_availability_and_reports_busy_jobs() {
    let mut s = seeded();
    let fake = Arc::new(FakeReloader(Mutex::new(Vec::new()), || {
        diary_application::ReloadOutcome::Applied(diary_application::StartupSummary {
            sources: vec![diary_application::Availability {
                alias: "fresh".into(),
                reason: Some(diary_application::UnavailableReason::SourceRootMissing),
            }],
            profiles: Vec::new(),
        })
    }));
    ServerState::set_reloader(&mut s, fake.clone());
    let app = router(s);
    let response = app
        .clone()
        .oneshot(owned_request("POST", "/api/v1/settings/reload"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        json_of(response).await,
        json!({"outcome":"applied","config_generation":2})
    );
    // The seeded job is not terminal, so the reloader was told an operation is in flight.
    assert_eq!(*fake.0.lock().unwrap(), vec![true]);
    let value = json_of(
        app.oneshot(owned_request("GET", "/api/v1/settings"))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(value["config_generation"], 2);
    assert_eq!(value["last_reload"]["outcome"], "applied");
    assert_eq!(value["sources"][0]["alias"], "fresh");
}

#[tokio::test]
async fn rejected_reload_keeps_generation_and_returns_only_codes() {
    let mut s = seeded();
    ServerState::set_reloader(
        &mut s,
        Arc::new(FakeReloader(Mutex::new(Vec::new()), || {
            diary_application::ReloadOutcome::Rejected(vec![diary_application::ReloadIssue {
                path: "config",
                code: "schema",
            }])
        })),
    );
    let app = router(s);
    let value = json_of(
        app.clone()
            .oneshot(owned_request("POST", "/api/v1/settings/reload"))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(
        value,
        json!({"outcome":"rejected","issues":[{"path":"config","code":"schema"}]})
    );
    let value = json_of(
        app.oneshot(owned_request("GET", "/api/v1/settings"))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(value["config_generation"], 1);
    assert_eq!(value["last_reload"]["outcome"], "rejected");
}

#[tokio::test]
async fn reload_requires_an_open_session() {
    let mut s = state();
    ServerState::set_reloader(
        &mut s,
        Arc::new(FakeReloader(Mutex::new(Vec::new()), || {
            diary_application::ReloadOutcome::Busy
        })),
    );
    let response = router(s)
        .oneshot(request("POST", "/api/v1/settings/reload"))
        .await
        .unwrap();
    assert!(response.status().is_client_error());
}

#[tokio::test]
async fn insertion_behind_job_page_cursor_is_discovered_by_first_watermark_replay() {
    use futures_util::StreamExt;
    let s = seeded();
    {
        let mut c = s.coordinator.lock().unwrap();
        for i in 0..200 {
            let id = format!("job-{i:03}");
            c.admit(
                SESSION,
                0,
                &format!("request-{i}"),
                &"a".repeat(64),
                &id,
                100,
            )
            .unwrap();
            c.cancel(SESSION, 0, &id).unwrap();
        }
    }
    let app = router(s.clone());
    let response = app
        .clone()
        .oneshot(owned_request("GET", "/api/v1/jobs"))
        .await
        .unwrap();
    let first: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 1_000_000).await.unwrap()).unwrap();
    assert_eq!(first["jobs"].as_array().unwrap().len(), 200);
    let cursor = first["next_cursor"].as_str().unwrap();
    {
        let mut c = s.coordinator.lock().unwrap();
        c.admit(SESSION, 0, "late-request", &"a".repeat(64), "job-000a", 100)
            .unwrap();
        c.publish(SESSION, 0, "job-000a", 400).unwrap();
    }
    let response = app
        .clone()
        .oneshot(owned_request(
            "GET",
            &format!("/api/v1/jobs?cursor={cursor}"),
        ))
        .await
        .unwrap();
    let second: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 1_000_000).await.unwrap()).unwrap();
    assert_eq!(second["jobs"].as_array().unwrap().len(), 1);
    assert!(second["next_cursor"].is_null());
    let mut request = owned_request("GET", "/api/v1/events");
    request.headers_mut().insert(
        "last-event-id",
        first["event_cursor"]
            .as_u64()
            .unwrap()
            .to_string()
            .parse()
            .unwrap(),
    );
    let response = app.oneshot(request).await.unwrap();
    let mut stream = response.into_body().into_data_stream();
    let frame = tokio::time::timeout(Duration::from_secs(1), stream.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let text = String::from_utf8(frame.to_vec()).unwrap();
    assert!(text.contains("job-000a") && text.contains("job_changed"));
}

#[tokio::test]
async fn negotiated_replay_boundary_follows_events_and_overflow_never_claims_completion() {
    use futures_util::StreamExt;
    for overflow in [false, true] {
        let s = seeded();
        {
            let mut c = s.coordinator.lock().unwrap();
            c.publish(SESSION, 0, "job", 400).unwrap();
            if overflow {
                c.transition(SESSION, 0, "job", diary_application::JobState::Running)
                    .unwrap();
                c.publish(SESSION, 0, "job", 400).unwrap();
                c.cancel(SESSION, 0, "job").unwrap();
                c.publish(SESSION, 0, "job", 400).unwrap();
            }
        }
        let mut request = owned_request("GET", "/api/v1/events");
        request
            .headers_mut()
            .insert("x-diary-reconcile", "1".parse().unwrap());
        let response = router(s).oneshot(request).await.unwrap();
        assert_eq!(response.headers()["x-diary-reconcile"], "1");
        let mut stream = response.into_body().into_data_stream();
        let frame = tokio::time::timeout(Duration::from_secs(1), stream.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let text = String::from_utf8(frame.to_vec()).unwrap();
        if overflow {
            assert!(text.contains("resync_required"));
            assert!(!text.contains("replay_complete"));
            assert!(stream.next().await.is_none());
        } else {
            assert!(text.contains("job_changed"));
            let frame = tokio::time::timeout(Duration::from_secs(1), stream.next())
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            assert_eq!(
                String::from_utf8(frame.to_vec()).unwrap(),
                "event: replay_complete\ndata: {}\n\n"
            );
        }
    }
}

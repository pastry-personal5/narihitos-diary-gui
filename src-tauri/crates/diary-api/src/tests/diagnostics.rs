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

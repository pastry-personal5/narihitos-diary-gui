use super::*;
use crate::{
    router,
    tests::{request, state},
};
use axum::body::{Body, to_bytes};
use diary_application::ReloadOutcome;
use std::sync::Mutex;
use tower::ServiceExt;

const SESSION: &str = "07249746-e280-4d51-a86a-cbcb198ee101";
const OP: &str = "00000000-0000-4000-8000-000000000001";
const STEP: &str = "00000000-0000-4000-8000-000000000002";

struct Stub {
    /// Checkpoint and state reported by `operation` and `start`.
    now: Mutex<(u64, ExportState)>,
    failure: Option<ExportError>,
    unfinished: bool,
    confirms: Mutex<u32>,
}
fn result(checkpoint: u64, state: ExportState) -> ExportResult {
    ExportResult {
        progress: None,
        content_kind: ContentKind::Authentic,
        illustrated_version: None,
        operation_id: OperationId::parse(OP).unwrap(),
        input_hash: "a".repeat(64),
        target: Target::Icloud,
        date: "2026-11-05".into(),
        revision_sequence: 1,
        revision_sha: "b".repeat(64),
        state,
        checkpoint,
        reason: None,
    }
}
impl Exports for Stub {
    fn start_illustrated(
        &self,
        _: &str,
        _: u64,
        request: IllustratedStartRequest,
    ) -> Result<Started, ExportError> {
        let mut r = result(1, ExportState::Running);
        r.content_kind = ContentKind::Illustrated;
        r.target = Target::Notion;
        r.illustrated_version = Some(request.expected_version);
        r.revision_sequence = 0;
        Ok(Started::Result(r))
    }
    fn available(&self, _: Target) -> bool {
        true
    }
    fn start(&self, _: &str, _: u64, _: StartRequest) -> Result<Started, ExportError> {
        if let Some(e) = self.failure {
            return Err(e);
        }
        let (c, s) = *self.now.lock().unwrap();
        Ok(Started::Result(result(c, s)))
    }
    fn confirm(&self, _: &str, _: u64, r: ConfirmRequest) -> Result<Admitted, ExportError> {
        if let Some(e) = self.failure {
            return Err(e);
        }
        *self.confirms.lock().unwrap() += 1;
        *self.now.lock().unwrap() = (2, ExportState::Running);
        Ok(Admitted {
            operation_id: r.operation_id,
            step_id: r.step_id,
            admitted_checkpoint: 2,
        })
    }
    fn cancel(&self, _: &OperationId) -> Result<ExportResult, ExportError> {
        *self.now.lock().unwrap() = (3, ExportState::Cancelled);
        Ok(result(3, ExportState::Cancelled))
    }
    fn abandon(&self, _: &str, _: u64, _: &OperationId) -> Result<Started, ExportError> {
        Ok(Started::Result(result(3, ExportState::Succeeded)))
    }
    fn abandon_confirm(
        &self,
        _: &str,
        _: u64,
        _: ConfirmRequest,
    ) -> Result<ExportResult, ExportError> {
        Ok(result(3, ExportState::Cancelled))
    }
    fn operation(&self, id: &OperationId) -> Result<Option<ExportResult>, ExportError> {
        let (c, s) = *self.now.lock().unwrap();
        Ok((id.as_str() == OP).then(|| result(c, s)))
    }
    fn unfinished(&self, _: Option<&str>, _: usize) -> Result<ExportPage, ExportError> {
        Ok(ExportPage {
            operations: vec![result(1, ExportState::NeedsConfirmation)],
            next_cursor: None,
        })
    }
    fn has_unfinished(&self) -> Result<bool, ExportError> {
        Ok(self.unfinished)
    }
    fn set_notifier(&self, _: Notifier) {}
}
fn setup(failure: Option<ExportError>, unfinished: bool) -> (Arc<ServerState>, Arc<Stub>) {
    let mut s = state();
    let stub = Arc::new(Stub {
        now: Mutex::new((1, ExportState::Running)),
        failure,
        unfinished,
        confirms: Mutex::new(0),
    });
    ServerState::set_exports(&mut s, stub.clone());
    s.coordinator
        .lock()
        .unwrap()
        .open(SESSION, s.now())
        .unwrap();
    (s, stub)
}
fn req(
    method: &str,
    path: &str,
    body: Option<Value>,
    authorized: bool,
) -> axum::http::Request<Body> {
    let mut req = request(method, path);
    if authorized {
        req.headers_mut()
            .insert("x-diary-session", SESSION.parse().unwrap());
    }
    if let Some(body) = body {
        req.headers_mut()
            .insert("content-type", "application/json".parse().unwrap());
        *req.body_mut() = Body::from(body.to_string());
    }
    req
}
async fn call(s: &Arc<ServerState>, r: axum::http::Request<Body>) -> (StatusCode, Value) {
    let response = router(s.clone()).oneshot(r).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1 << 20).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}
fn start_body() -> Value {
    json!({"operation_id":OP,"date":"2026-11-05","target":"icloud","expected_revision":{"sequence":1,"sha256":"b".repeat(64)}})
}
fn confirm_body() -> Value {
    json!({"step_id":STEP,"expected_checkpoint":1,"token":"private-token"})
}
fn confirm_path() -> String {
    format!("/api/v1/exports/{OP}/confirmation")
}

#[tokio::test]
async fn illustrated_start_requires_a_version_and_notion_without_authentic_revision() {
    let (s, _) = setup(None, false);
    let valid = json!({"operation_id":OP,"date":"2026-11-05","target":"notion","content_kind":"illustrated","expected_version":STEP});
    let (status, body) = call(
        &s,
        req("POST", "/api/v1/exports", Some(valid.clone()), true),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["content_kind"], "illustrated");
    assert_eq!(body["illustrated_version"], STEP);
    for invalid in [
        {
            let mut v = valid.clone();
            v["target"] = json!("icloud");
            v
        },
        {
            let mut v = valid.clone();
            v["expected_revision"] = json!({"sequence":1,"sha256":"b".repeat(64)});
            v
        },
        {
            let mut v = valid.clone();
            v["expected_version"] = json!("not-a-version");
            v
        },
    ] {
        assert_eq!(
            call(&s, req("POST", "/api/v1/exports", Some(invalid), true))
                .await
                .0,
            StatusCode::BAD_REQUEST
        );
    }
}

#[tokio::test]
async fn routes_require_a_session_and_validate_input() {
    let (s, stub) = setup(None, false);
    for (path, body) in [
        ("/api/v1/exports".to_string(), start_body()),
        (confirm_path(), confirm_body()),
    ] {
        assert_eq!(
            call(&s, req("POST", &path, Some(body), false)).await.0,
            StatusCode::UNAUTHORIZED
        );
    }
    let mut bad = start_body();
    bad["expected_revision"]["sha256"] = json!("short");
    let mut extra = start_body();
    extra["path"] = json!("/tmp");
    let mut target = start_body();
    target["target"] = json!("dropbox");
    for body in [bad, extra, target] {
        assert_eq!(
            call(&s, req("POST", "/api/v1/exports", Some(body), true))
                .await
                .0,
            StatusCode::BAD_REQUEST
        );
    }
    let mut zero = confirm_body();
    zero["expected_checkpoint"] = json!(0);
    assert_eq!(
        call(&s, req("POST", &confirm_path(), Some(zero), true))
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        call(
            &s,
            req(
                "POST",
                "/api/v1/exports/not-a-uuid/confirmation",
                Some(confirm_body()),
                true
            )
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(*stub.confirms.lock().unwrap(), 0);
}

#[tokio::test]
async fn disabled_targets_and_typed_errors() {
    let mut s = state();
    s.coordinator
        .lock()
        .unwrap()
        .open(SESSION, s.now())
        .unwrap();
    let (status, body) = call(&s, req("POST", "/api/v1/exports", Some(start_body()), true)).await;
    assert_eq!(
        (status, body["error"]["code"].as_str()),
        (StatusCode::UNPROCESSABLE_ENTITY, Some("export_disabled"))
    );
    let _ = &mut s;
    for (e, code) in [
        (ExportError::StaleRevision, "stale_revision"),
        (ExportError::Uncertain, "outcome_uncertain"),
        (ExportError::Busy, "resource_reserved"),
        (ExportError::NothingToExport, "nothing_to_export"),
    ] {
        let (s, _) = setup(Some(e), false);
        let (_, body) = call(&s, req("POST", "/api/v1/exports", Some(start_body()), true)).await;
        assert_eq!(body["error"]["code"], code);
    }
}

// O8: one event per durable change; a duplicate or rejected confirmation adds none.
#[tokio::test]
async fn state_changes_publish_one_job_change_each_and_exports_cannot_be_cancelled() {
    let (s, stub) = setup(None, false);
    let (status, started) =
        call(&s, req("POST", "/api/v1/exports", Some(start_body()), true)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(started["outcome"], "running");
    let job = |s: &Arc<ServerState>| {
        s.coordinator
            .lock()
            .unwrap()
            .get(SESSION, s.now(), OP)
            .unwrap()
    };
    let first = job(&s).sequence;
    // The same state again publishes nothing.
    call(&s, req("POST", "/api/v1/exports", Some(start_body()), true)).await;
    assert_eq!(job(&s).sequence, first);
    let (status, ack) = call(&s, req("POST", &confirm_path(), Some(confirm_body()), true)).await;
    assert_eq!(
        (status, ack["admitted_checkpoint"].as_u64()),
        (StatusCode::OK, Some(2))
    );
    let second = job(&s).sequence;
    assert!(second > first);
    call(&s, req("POST", &confirm_path(), Some(confirm_body()), true)).await;
    assert_eq!(
        job(&s).sequence,
        second,
        "a duplicate confirmation publishes nothing"
    );
    *stub.now.lock().unwrap() = (3, ExportState::Succeeded);
    let (e, _) = (
        stub.operation(&OperationId::parse(OP).unwrap())
            .unwrap()
            .unwrap(),
        0,
    );
    sync(&s, SESSION, &e);
    assert_eq!(job(&s).state, JobState::Succeeded);
    let (status, body) = call(
        &s,
        req("POST", &format!("/api/v1/jobs/{OP}/cancel"), None, true),
    )
    .await;
    // A terminal export cannot be cancelled again; the call reports the recorded job.
    assert_eq!(status, StatusCode::OK, "{body}");
}

#[tokio::test]
async fn typed_lookup_and_unfinished_listing() {
    let (s, _) = setup(None, false);
    let (status, body) = call(
        &s,
        req("GET", &format!("/api/v1/operations/{OP}"), None, true),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        (body["kind"].as_str(), body["export"]["kind"].as_str()),
        (Some("export"), Some("export"))
    );
    assert!(!body.to_string().contains("private"));
    let (status, body) = call(
        &s,
        req(
            "GET",
            "/api/v1/operations?kind=export&state=unfinished",
            None,
            true,
        ),
    )
    .await;
    assert_eq!(
        (status, body["operations"][0]["state"].as_str()),
        (StatusCode::OK, Some("needs_confirmation"))
    );
}

struct Reloader(Mutex<Vec<bool>>);
impl diary_application::ConfigReloader for Reloader {
    fn reload(&self, busy: bool) -> ReloadOutcome {
        self.0.lock().unwrap().push(busy);
        ReloadOutcome::Busy
    }
}
// T09: an unfinished export that survived a restart blocks guarded reloads.
#[tokio::test]
async fn reload_is_busy_while_a_ledger_export_is_unfinished() {
    for unfinished in [false, true] {
        let (mut s, _) = setup(None, unfinished);
        let reloader = Arc::new(Reloader(Mutex::default()));
        // `setup` already shares the Arc, so build the state again before attaching the reloader.
        drop(s.clone());
        let mut fresh = state();
        ServerState::set_exports(
            &mut fresh,
            Arc::new(Stub {
                now: Mutex::new((1, ExportState::Running)),
                failure: None,
                unfinished,
                confirms: Mutex::new(0),
            }),
        );
        ServerState::set_reloader(&mut fresh, reloader.clone());
        fresh
            .coordinator
            .lock()
            .unwrap()
            .open(SESSION, fresh.now())
            .unwrap();
        s = fresh;
        call(&s, req("POST", "/api/v1/settings/reload", None, true)).await;
        assert_eq!(*reloader.0.lock().unwrap(), vec![unfinished]);
    }
}

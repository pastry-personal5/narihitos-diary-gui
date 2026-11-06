use super::*;
use crate::{
    router,
    tests::{request, state},
};
use axum::body::{Body, to_bytes};
use std::sync::Mutex;
use tower::ServiceExt;

const SESSION: &str = "07249746-e280-4d51-a86a-cbcb198ee101";
const OP: &str = "00000000-0000-4000-8000-000000000001";
const STEP: &str = "00000000-0000-4000-8000-000000000002";
const SOURCE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
struct Fake {
    previews: Mutex<Vec<DeletePreviewRequest>>,
    confirmations: Mutex<Vec<DeleteConfirmRequest>>,
    failure: Option<DeleteError>,
}
fn result_fixture() -> DeleteResult {
    DeleteResult {
        operation_id: OperationId::parse(OP).unwrap(),
        source_id: SOURCE.into(),
        plan_hash: "a".repeat(64),
        checkpoint: 1,
        state: DeleteState::NeedsConfirmation,
        members: vec![
            DeleteMember {
                ordinal: 0,
                relative_label: "source.md".into(),
                state: DeleteMemberState::Moved,
                reason: None,
                retry_eligible: false,
            },
            DeleteMember {
                ordinal: 1,
                relative_label: "source.assets/audio.m4a".into(),
                state: DeleteMemberState::Failed,
                reason: Some("unavailable"),
                retry_eligible: true,
            },
        ],
    }
}
impl SourceTrash for Fake {
    fn preview(
        &self,
        _: &str,
        _: u64,
        r: DeletePreviewRequest,
        _: &dyn Fn() -> bool,
    ) -> Result<DeletePreview, DeleteError> {
        let checkpoint = r.expected_checkpoint;
        self.previews.lock().unwrap().push(r);
        if let Some(e) = self.failure {
            return Err(e);
        }
        Ok(DeletePreview {
            token: "private-token".into(),
            plan_hash: "a".repeat(64),
            expected_checkpoint: checkpoint,
            expires_in_ms: 60_000,
            ordinals: vec![0, 1],
            members: result_fixture().members,
        })
    }
    fn confirm(
        &self,
        _: &str,
        _: u64,
        r: DeleteConfirmRequest,
        _: &dyn Fn() -> bool,
    ) -> Result<DeleteAcknowledged, DeleteError> {
        let ack = DeleteAcknowledged {
            operation_id: r.operation_id.clone(),
            step_id: r.step_id.clone(),
            admitted_checkpoint: r.expected_checkpoint + 1,
        };
        self.confirmations.lock().unwrap().push(r);
        if let Some(e) = self.failure {
            return Err(e);
        }
        Ok(ack)
    }
    fn operation(&self, id: &OperationId) -> Result<Option<DeleteResult>, DeleteError> {
        if let Some(e) = self.failure {
            return Err(e);
        }
        Ok((id.as_str() == OP).then(result_fixture))
    }
    fn unfinished(&self, _: Option<&str>, _: usize) -> Result<DeletePage, DeleteError> {
        if let Some(e) = self.failure {
            return Err(e);
        }
        Ok(DeletePage {
            operations: vec![result_fixture()],
            next_cursor: Some("opaque-cursor".into()),
        })
    }
}
fn setup(failure: Option<DeleteError>) -> (Arc<ServerState>, Arc<Fake>) {
    let mut s = state();
    let fake = Arc::new(Fake {
        previews: Mutex::default(),
        confirmations: Mutex::default(),
        failure,
    });
    ServerState::set_source_trash(&mut s, fake.clone());
    s.coordinator
        .lock()
        .unwrap()
        .open(SESSION, s.now())
        .unwrap();
    (s, fake)
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
async fn call(s: Arc<ServerState>, r: axum::http::Request<Body>) -> (StatusCode, Value) {
    let response = router(s).oneshot(r).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1 << 20).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}
fn confirm_body() -> Value {
    json!({"kind":"initial","operation_id":OP,"step_id":STEP,"expected_checkpoint":0,"plan_hash":"a".repeat(64),"ordinals":[0,1],"token":"private-token"})
}

#[tokio::test]
async fn every_trash_route_requires_an_admitted_session() {
    let (s, fake) = setup(None);
    for (method, path, body) in [
        (
            "POST",
            format!("/api/v1/sources/{SOURCE}/trash-preview"),
            Some(json!({"kind":"initial","expected_checkpoint":0})),
        ),
        (
            "POST",
            format!("/api/v1/sources/{SOURCE}/trash"),
            Some(confirm_body()),
        ),
        (
            "GET",
            "/api/v1/operations?kind=source_trash&state=unfinished".into(),
            None,
        ),
        ("GET", format!("/api/v1/operations/{OP}"), None),
    ] {
        assert_eq!(
            call(s.clone(), req(method, &path, body, false)).await.0,
            StatusCode::UNAUTHORIZED
        );
    }
    assert!(fake.previews.lock().unwrap().is_empty());
    assert!(fake.confirmations.lock().unwrap().is_empty());
}

#[tokio::test]
async fn preview_and_confirmation_preserve_tagged_business_identity() {
    let (s, fake) = setup(None);
    for (kind, checkpoint, operation) in [("initial", 0, None), ("continuation", 4, Some(OP))] {
        let body = json!({"kind":kind,"operation_id":operation,"expected_checkpoint":checkpoint});
        let (status, p) = call(
            s.clone(),
            req(
                "POST",
                &format!("/api/v1/sources/{SOURCE}/trash-preview"),
                Some(body),
                true,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(p["expires_in_ms"], 60_000);
        assert_eq!(p["members"][1]["relative_label"], "source.assets/audio.m4a");
        let mut body = confirm_body();
        body["kind"] = json!(kind);
        body["expected_checkpoint"] = json!(checkpoint);
        let (status, a) = call(
            s.clone(),
            req(
                "POST",
                &format!("/api/v1/sources/{SOURCE}/trash"),
                Some(body),
                true,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(a["admitted_checkpoint"], checkpoint + 1);
    }
    assert_eq!(
        fake.previews.lock().unwrap()[1].action,
        DeleteAction::Continuation
    );
    assert_eq!(fake.confirmations.lock().unwrap()[1].step_id.as_str(), STEP);
}

#[tokio::test]
async fn discovery_and_existing_lookup_materialize_safe_members_without_authentic_store() {
    let (s, _) = setup(None);
    let (status, p) = call(
        s.clone(),
        req(
            "GET",
            "/api/v1/operations?kind=source_trash&state=unfinished&limit=50",
            None,
            true,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(p["operations"][0]["state"], "needs_confirmation");
    assert_eq!(p["next_cursor"], "opaque-cursor");
    let (status, o) = call(
        s,
        req("GET", &format!("/api/v1/operations/{OP}"), None, true),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(o["state"], "paused");
    assert_eq!(o["trash"]["state"], "needs_confirmation");
    assert_eq!(o["trash"]["members"][0]["state"], "moved");
    let text = o.to_string();
    for private in ["private-token", "receipt", "native", "root"] {
        assert!(!text.contains(private));
    }
}

#[tokio::test]
async fn malformed_oversized_and_arbitrary_path_inputs_are_rejected() {
    let (s, fake) = setup(None);
    for body in [
        json!({"kind":"unknown","expected_checkpoint":0}),
        json!({"kind":"initial","expected_checkpoint":0,"path":"/private"}),
        json!({"kind":"initial","expected_checkpoint":0,"operation_id":"invalid"}),
    ] {
        assert_eq!(
            call(
                s.clone(),
                req(
                    "POST",
                    &format!("/api/v1/sources/{SOURCE}/trash-preview"),
                    Some(body),
                    true
                )
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
    }
    let mut oversized = confirm_body();
    oversized["token"] = json!("x".repeat(5000));
    assert_eq!(
        call(
            s.clone(),
            req(
                "POST",
                &format!("/api/v1/sources/{SOURCE}/trash"),
                Some(oversized),
                true
            )
        )
        .await
        .0,
        StatusCode::PAYLOAD_TOO_LARGE
    );
    assert_eq!(
        call(
            s.clone(),
            req(
                "POST",
                "/api/v1/sources/arbitrary-path/trash",
                Some(confirm_body()),
                true
            )
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    for query in [
        "kind=all&state=unfinished",
        "kind=source_trash&state=all",
        "kind=source_trash&state=unfinished&limit=101",
        "kind=source_trash&state=unfinished&path=private",
    ] {
        assert_eq!(
            call(
                s.clone(),
                req("GET", &format!("/api/v1/operations?{query}"), None, true)
            )
            .await
            .0,
            if query.contains("path=") {
                StatusCode::FORBIDDEN
            } else {
                StatusCode::BAD_REQUEST
            }
        );
    }
    assert!(fake.previews.lock().unwrap().is_empty());
    assert!(fake.confirmations.lock().unwrap().is_empty());
}

#[tokio::test]
async fn errors_are_allowlisted_and_never_echo_tokens_or_native_details() {
    for (failure, code) in [
        (DeleteError::Expired, "preview_expired"),
        (DeleteError::Changed, "member_changed"),
        (DeleteError::OwnershipAmbiguous, "ownership_ambiguous"),
        (DeleteError::Uncertain, "outcome_uncertain"),
        (DeleteError::CheckpointMismatch, "checkpoint_mismatch"),
        (DeleteError::Busy, "resource_reserved"),
    ] {
        let (s, _) = setup(Some(failure));
        let (_, body) = call(
            s,
            req(
                "POST",
                &format!("/api/v1/sources/{SOURCE}/trash"),
                Some(confirm_body()),
                true,
            ),
        )
        .await;
        assert_eq!(body["error"]["code"], code);
        assert!(!body.to_string().contains("private-token"));
    }
}

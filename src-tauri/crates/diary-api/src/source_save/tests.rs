use super::*;
use crate::{
    router,
    tests::{request, state},
};
use axum::body::{Body, to_bytes};
use diary_application::source::{SourceSaved, SourceWriter};
use serde_json::{Value, json};
use std::sync::Mutex;
use tower::ServiceExt;

const SESSION: &str = "07249746-e280-4d51-a86a-cbcb198ee101";
const OP: &str = "00000000-0000-4000-8000-000000000001";
struct Fake(Mutex<Vec<SourceSaveRequest>>, Result<(), SaveError>);
impl SourceWriter for Fake {
    fn save_body(&self, r: SourceSaveRequest) -> Result<SourceSaved, SaveError> {
        self.0.lock().unwrap().push(r.clone());
        self.1.map(|()| SourceSaved {
            operation_id: r.operation_id,
            digest: "b".repeat(64),
            size: 9,
            unchanged: false,
        })
    }
}
fn setup(result: Result<(), SaveError>, open: bool) -> (Arc<ServerState>, Arc<Fake>) {
    let mut s = state();
    let fake = Arc::new(Fake(Mutex::default(), result));
    ServerState::set_source_writer(&mut s, fake.clone());
    if open {
        s.coordinator
            .lock()
            .unwrap()
            .open(SESSION, s.now())
            .unwrap();
    }
    (s, fake)
}
fn put(id: &str, body: Value, owner: bool) -> axum::http::Request<Body> {
    let mut r = request("PUT", &format!("/api/v1/sources/{id}/body"));
    if owner {
        r.headers_mut()
            .insert("x-diary-session", SESSION.parse().unwrap());
    }
    r.headers_mut()
        .insert("content-type", "application/json".parse().unwrap());
    *r.body_mut() = Body::from(body.to_string());
    r
}
fn ok_body() -> Value {
    json!({"operation_id":OP,"base_digest":"a".repeat(64),"body":"text"})
}
async fn call(s: Arc<ServerState>, r: axum::http::Request<Body>) -> (StatusCode, Value) {
    let res = router(s).oneshot(r).await.unwrap();
    let status = res.status();
    (
        status,
        serde_json::from_slice(&to_bytes(res.into_body(), 1 << 20).await.unwrap()).unwrap(),
    )
}

#[tokio::test]
async fn requires_session() {
    let (s, fake) = setup(Ok(()), false);
    assert_eq!(
        call(s, put("abc", ok_body(), false)).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert!(fake.0.lock().unwrap().is_empty());
}

#[tokio::test]
async fn saves_and_answers_digest() {
    let (s, fake) = setup(Ok(()), true);
    let (status, v) = call(s, put("abc", ok_body(), true)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        v,
        json!({"operation_id":OP,"digest":"b".repeat(64),"size":9,"unchanged":false})
    );
    let seen = fake.0.lock().unwrap();
    assert_eq!(
        (seen[0].source_id.as_str(), seen[0].body.as_str()),
        ("abc", "text")
    );
}

#[tokio::test]
async fn rejects_unknown_fields_and_bad_input() {
    let (s, fake) = setup(Ok(()), true);
    let bad = [
        json!({"operation_id":OP,"base_digest":"a".repeat(64),"body":"x","path":"/p"}),
        json!({"operation_id":"nope","base_digest":"a".repeat(64),"body":"x"}),
        json!({"operation_id":OP,"base_digest":"A".repeat(64),"body":"x"}),
        json!({"operation_id":OP,"base_digest":"a","body":"x"}),
    ];
    for b in bad {
        assert_eq!(
            call(s.clone(), put("abc", b, true)).await.0,
            StatusCode::BAD_REQUEST
        );
    }
    assert_eq!(
        call(s, put("a%20b", ok_body(), true)).await.0.is_success(),
        false
    );
    assert!(fake.0.lock().unwrap().is_empty());
}

#[tokio::test]
async fn maps_every_error_code() {
    use SaveError::*;
    let cases = [
        (ExternalChange, 409, "external_change"),
        (UnsupportedFormat, 422, "unsupported_format"),
        (InvalidBody, 422, "invalid_body"),
        (CloudUnavailable, 503, "cloud_unavailable"),
        (SourceChanging, 409, "source_changing"),
        (SourceStale, 409, "source_stale"),
        (Reserved, 409, "resource_reserved"),
        (Mismatch, 409, "operation_mismatch"),
        (Uncertain, 503, "outcome_uncertain"),
        (NotFound, 404, "not_found"),
        (Storage, 500, "storage_error"),
    ];
    for (e, status, code) in cases {
        let (s, _) = setup(Err(e), true);
        let (got, v) = call(s, put("abc", ok_body(), true)).await;
        assert_eq!(
            (got.as_u16(), v["error"]["code"].as_str()),
            (status, Some(code))
        );
    }
}

#[tokio::test]
async fn unavailable_without_a_writer() {
    let s = state();
    s.coordinator
        .lock()
        .unwrap()
        .open(SESSION, s.now())
        .unwrap();
    let (status, v) = call(s, put("abc", ok_body(), true)).await;
    assert_eq!(
        (status, v["error"]["code"].as_str()),
        (StatusCode::SERVICE_UNAVAILABLE, Some("storage_unavailable"))
    );
}

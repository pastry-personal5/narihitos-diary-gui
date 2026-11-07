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
    let good =
        json!({"operation_id": OP, "expected_revision": null, "body": "x", "buffer_generation": 1});
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

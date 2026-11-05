use super::*;
use crate::tests::{request, state};
use axum::body::{Body, to_bytes};
use diary_application::imports::ImportContext;
use diary_domain::source::window::ImportZone;
use std::sync::atomic::{AtomicBool, AtomicUsize};
use tower::ServiceExt;

struct Fake {
    calls: AtomicUsize,
    wait: AtomicBool,
    fail: AtomicBool,
}
fn listing(id: &str) -> ImportListing {
    ImportListing {
        context: ImportContext {
            id: id.into(),
            date: DiaryDate::parse("2024-02-29").unwrap(),
            zone: ImportZone {
                name: "UTC".into(),
                database_version: "2026e".into(),
            },
            config_generation: 1,
            completed_at: Some(1),
        },
        included: vec![],
        warnings: vec![],
        misplaced: vec![],
        scan: Default::default(),
    }
}
impl ImportStore for Fake {
    fn import_date(
        &self,
        id: &str,
        _: DiaryDate,
        _: u64,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<ImportListing, ImportError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let until = std::time::Instant::now() + Duration::from_secs(3);
        while self.wait.load(Ordering::SeqCst) && std::time::Instant::now() < until {
            if cancelled() {
                return Err(ImportError::Cancelled);
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        if self.fail.load(Ordering::SeqCst) {
            Err(ImportError::TimezoneDatabase)
        } else {
            Ok(listing(id))
        }
    }
    fn list(&self, id: &str, _: &dyn Fn() -> bool) -> Result<ImportListing, ImportError> {
        Ok(listing(id))
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
        Err(ImportError::Stale)
    }
}
fn setup() -> (Arc<ServerState>, Arc<Fake>) {
    let mut s = state();
    let fake = Arc::new(Fake {
        calls: AtomicUsize::new(0),
        wait: AtomicBool::new(false),
        fail: AtomicBool::new(false),
    });
    ServerState::set_imports(&mut s, fake.clone());
    (s, fake)
}
fn req(method: &str, uri: &str, body: Option<Value>, owner: bool) -> axum::http::Request<Body> {
    let mut r = request(method, uri);
    if owner {
        r.headers_mut().insert(
            "x-diary-session",
            "07249746-e280-4d51-a86a-cbcb198ee101".parse().unwrap(),
        );
    }
    if let Some(b) = body {
        r.headers_mut()
            .insert("content-type", "application/json".parse().unwrap());
        *r.body_mut() = Body::from(b.to_string());
    }
    r
}
async fn value(r: Response) -> Value {
    serde_json::from_slice(&to_bytes(r.into_body(), 1024 * 1024).await.unwrap()).unwrap()
}
async fn terminal(s: &ServerState, id: &str) -> JobState {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let state = s
                .coordinator
                .lock()
                .unwrap()
                .get("07249746-e280-4d51-a86a-cbcb198ee101", s.now(), id)
                .unwrap()
                .state;
            if state.is_terminal() {
                return state;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap()
}
#[tokio::test]
async fn owning_session_strict_bodies_destinations_retry_and_redacted_errors() {
    let (s, fake) = setup();
    let app = router(s.clone());
    let b = json!({"date":"2024-02-29","request_id":"request-a"});
    assert_eq!(
        app.clone()
            .oneshot(req("POST", "/api/v1/imports", Some(b.clone()), false))
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    s.coordinator
        .lock()
        .unwrap()
        .open("07249746-e280-4d51-a86a-cbcb198ee101", s.now())
        .unwrap();
    for b in [
        json!({"date":"/PRIVATE_SECRET", "request_id":"r"}),
        json!({"date":"2024-02-29","request_id":"r","path":"/PRIVATE_SECRET"}),
    ] {
        let response = app
            .clone()
            .oneshot(req("POST", "/api/v1/imports", Some(b), true))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(!value(response).await.to_string().contains("PRIVATE_SECRET"));
    }
    assert_eq!(fake.calls.load(Ordering::SeqCst), 0);
    let response = app
        .clone()
        .oneshot(req("POST", "/api/v1/imports", Some(b.clone()), true))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let ack = value(response).await;
    let id = ack["import_id"].as_str().unwrap();
    assert_eq!(terminal(&s, id).await, JobState::Succeeded);
    let retry = value(
        app.clone()
            .oneshot(req("POST", "/api/v1/imports", Some(b), true))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(retry["import_id"], id);
    assert_eq!(fake.calls.load(Ordering::SeqCst), 1);
    let mismatch = app
        .clone()
        .oneshot(req(
            "POST",
            "/api/v1/imports",
            Some(json!({"date":"2024-03-01","request_id":"request-a"})),
            true,
        ))
        .await
        .unwrap();
    assert_eq!(mismatch.status(), StatusCode::CONFLICT);
    let response = app
        .clone()
        .oneshot(req(
            "GET",
            &format!("/api/v1/sources?import_id={id}"),
            None,
            true,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(value(response).await["zone"], "UTC");
    let response = app
        .clone()
        .oneshot(req(
            "POST",
            "/api/v1/sources/source-a/time-resolution",
            Some(json!({"import_id":id,"digest":"a".repeat(64),"authority":"header"})),
            true,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(value(response).await["error"]["code"], "source_stale");
    for query in [
        "import_id=/PRIVATE_SECRET",
        "import_id=x&path=secret",
        "import_id=x&import_id=y",
        "import_id=x&cursor=-1",
    ] {
        assert_eq!(
            app.clone()
                .oneshot(req("GET", &format!("/api/v1/sources?{query}"), None, true))
                .await
                .unwrap()
                .status(),
            StatusCode::FORBIDDEN
        );
    }
    assert_eq!(
        app.oneshot(req(
            "GET",
            &format!("/api/v1/sources?import_id={id}"),
            None,
            false
        ))
        .await
        .unwrap()
        .status(),
        StatusCode::UNAUTHORIZED
    );
}
#[tokio::test]
async fn import_jobs_report_zone_failure_and_honour_cancellation() {
    let (s, fake) = setup();
    s.coordinator
        .lock()
        .unwrap()
        .open("07249746-e280-4d51-a86a-cbcb198ee101", s.now())
        .unwrap();
    let app = router(s.clone());
    fake.fail.store(true, Ordering::SeqCst);
    let ack = value(
        app.clone()
            .oneshot(req(
                "POST",
                "/api/v1/imports",
                Some(json!({"date":"2024-02-29","request_id":"failed"})),
                true,
            ))
            .await
            .unwrap(),
    )
    .await;
    let id = ack["import_id"].as_str().unwrap();
    assert_eq!(terminal(&s, id).await, JobState::Failed);
    let job = value(
        app.clone()
            .oneshot(req("GET", &format!("/api/v1/jobs/{id}"), None, true))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(job["error_code"], "timezone_database_unavailable");
    fake.wait.store(true, Ordering::SeqCst);
    let ack = value(
        app.clone()
            .oneshot(req(
                "POST",
                "/api/v1/imports",
                Some(json!({"date":"2024-02-29","request_id":"cancel"})),
                true,
            ))
            .await
            .unwrap(),
    )
    .await;
    let id = ack["import_id"].as_str().unwrap();
    let r = app
        .oneshot(req(
            "POST",
            &format!("/api/v1/jobs/{id}/cancel"),
            None,
            true,
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    assert_eq!(terminal(&s, id).await, JobState::Cancelled);
    fake.wait.store(false, Ordering::SeqCst);
}

#[test]
fn source_pages_are_bounded_and_do_not_expose_diagnostics_or_content() {
    let mut r = listing("import");
    for i in 0..201 {
        r.scan.failures.push((
            format!("2024/02/29/{i:03}.md"),
            diary_application::source::SourceOutcome::Unavailable(
                diary_application::source::Unavailable::Missing,
            ),
        ));
    }
    let first = page(r, 0).unwrap();
    assert_eq!(first["items"].as_array().unwrap().len(), 100);
    assert_eq!(first["next_cursor"], 100);
    assert_eq!(first["items"][0]["code"], "missing");
    assert_eq!(first["items"][0]["relative_key"], "2024/02/29/000.md");
}

#[tokio::test]
async fn rejects_oversized_import_bodies_before_store_calls() {
    let (s, fake) = setup();
    s.coordinator
        .lock()
        .unwrap()
        .open("07249746-e280-4d51-a86a-cbcb198ee101", s.now())
        .unwrap();
    let response = router(s)
        .oneshot(req(
            "POST",
            "/api/v1/imports",
            Some(json!({"date":"2024-02-29","request_id":"x".repeat(5000)})),
            true,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(fake.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn capacity_failure_remains_visible_on_retry_and_job_lookup() {
    let (s, fake) = setup();
    let session = "07249746-e280-4d51-a86a-cbcb198ee101";
    s.coordinator
        .lock()
        .unwrap()
        .open(session, s.now())
        .unwrap();
    let _slots = s.source_slots.clone().acquire_many_owned(2).await.unwrap();
    let app = router(s.clone());
    let input = json!({"date":"2024-02-29","request_id":"capacity"});
    let r = app
        .clone()
        .oneshot(req("POST", "/api/v1/imports", Some(input.clone()), true))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::TOO_MANY_REQUESTS);
    let retry = value(
        app.clone()
            .oneshot(req("POST", "/api/v1/imports", Some(input), true))
            .await
            .unwrap(),
    )
    .await;
    let id = retry["import_id"].as_str().unwrap();
    assert_eq!(retry["job"]["state"], "failed");
    let job = value(
        app.oneshot(req("GET", &format!("/api/v1/jobs/{id}"), None, true))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(job["error_code"], "limit");
    assert_eq!(fake.calls.load(Ordering::SeqCst), 0);
}

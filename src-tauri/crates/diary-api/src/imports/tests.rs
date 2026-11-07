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
        navigation: None,
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
    fn today(&self) -> Result<DiaryDate, ImportError> {
        Ok(DiaryDate::parse("2024-02-29").unwrap())
    }
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
    let first = page(r, 0, true).unwrap();
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

#[test]
fn cancellation_polling_is_throttled_and_latched() {
    let polls = std::cell::Cell::new(0);
    let stop = std::cell::Cell::new(false);
    let check = throttled(|| {
        polls.set(polls.get() + 1);
        stop.get()
    });
    assert!(!check());
    assert_eq!(polls.get(), 1);
    stop.set(true);
    for _ in 0..63 {
        assert!(!check());
    }
    assert_eq!(polls.get(), 1);
    assert!(check());
    assert_eq!(polls.get(), 2);
    stop.set(false);
    for _ in 0..200 {
        assert!(check());
    }
    assert_eq!(polls.get(), 2);
    assert!(throttled(|| true)());
}

#[test]
fn page_crosses_category_boundaries_and_handles_end_cursors() {
    let make = || {
        let mut r = listing("paging");
        r.scan.failures = (0..105)
            .map(|i| {
                (
                    format!("{i}.md"),
                    diary_application::source::SourceOutcome::Unavailable(
                        diary_application::source::Unavailable::Missing,
                    ),
                )
            })
            .collect();
        r.scan.collisions = vec![vec!["a.md".into(), "A.md".into()]];
        r
    };
    let first = page(make(), 0, true).unwrap();
    assert_eq!(first["items"].as_array().unwrap().len(), 100);
    assert_eq!(first["next_cursor"], 100);
    let last = page(make(), 100, true).unwrap();
    assert_eq!(last["items"].as_array().unwrap().len(), 6);
    assert_eq!(last["items"][0]["relative_key"], "100.md");
    assert_eq!(last["items"][5]["kind"], "path_collision");
    assert!(last["next_cursor"].is_null());
    assert!(
        page(make(), 106, true).unwrap()["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(page(make(), 107, true), Err(ImportError::NotFound));
}

#[tokio::test]
async fn server_calendar_requires_owner_and_clamps_civil_navigation() {
    let (s, _) = setup();
    let app = router(s.clone());
    assert_eq!(
        app.clone()
            .oneshot(req("GET", "/api/v1/calendar", None, false))
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
    let response = app
        .clone()
        .oneshot(req(
            "GET",
            "/api/v1/calendar?anchor=2024-01-31&action=next_month",
            None,
            true,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let dto = value(response).await;
    assert_eq!(dto["anchor"], "2024-02-29");
    assert_eq!(dto["today"], "2024-02-29");
    assert_eq!(dto["days"].as_array().unwrap().len(), 29);
    let response = app
        .clone()
        .oneshot(req(
            "GET",
            "/api/v1/calendar?anchor=0001-01-01&action=previous_day",
            None,
            true,
        ))
        .await
        .unwrap();
    assert_eq!(value(response).await["error"]["code"], "invalid_diary_date");
    for uri in [
        "/api/v1/calendar?anchor=2024-02-30",
        "/api/v1/calendar?timezone=private",
        "/api/v1/calendar?action=month&action=today",
        "/api/v1/sources/id?snapshot_id=x&path=private",
    ] {
        assert_eq!(
            app.clone()
                .oneshot(req("GET", uri, None, true))
                .await
                .unwrap()
                .status(),
            StatusCode::FORBIDDEN
        );
    }
}

struct ReadFake {
    wait: AtomicBool,
    started: AtomicBool,
}
fn readable_listing(id: &str) -> ImportListing {
    use diary_application::{imports::*, source::*};
    use diary_domain::source::stamp::Precision;
    let mut r = listing(id);
    let source = SourceListing {
        id: "source-id".into(),
        rel: "2024/02/29/test.md".into(),
        size: 100,
        dating: Dating::Agreed(Instant {
            unix_ms: 1,
            precision: Precision::Milli,
        }),
        warnings: vec![],
        token: ObservationToken {
            root_namespace: [9; 32],
            key: "2024/02/29/test.md".into(),
            sha256: [1; 32],
            dev: 99,
            ino: 98,
            size: 100,
            mtime_ns: 1,
            ctime_ns: 1,
            config_generation: 1,
            import_id: Some(id.into()),
        },
    };
    r.scan.listings.push(source);
    r.navigation = Some(Navigation {
        groups: vec![SourceGroup {
            id: "group".into(),
            alias: "Diary".into(),
            kind: "ayumi".into(),
        }],
        sources: std::collections::BTreeMap::from([(
            "source-id".into(),
            SourceDisplay {
                group_id: "group".into(),
                header: None,
                filename: None,
                included: None,
                authority: None,
            },
        )]),
        ..Default::default()
    });
    r
}
impl ImportStore for ReadFake {
    fn import_date(
        &self,
        _: &str,
        _: DiaryDate,
        _: u64,
        _: &dyn Fn() -> bool,
    ) -> Result<ImportListing, ImportError> {
        unreachable!()
    }
    fn list(&self, id: &str, _: &dyn Fn() -> bool) -> Result<ImportListing, ImportError> {
        Ok(readable_listing(id))
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
    fn read(
        &self,
        import_id: &str,
        _: &str,
        _: u64,
        _: &dyn Fn() -> bool,
    ) -> Result<diary_application::source::SourceView, diary_application::source::ViewError> {
        use diary_application::source::*;
        self.started.store(true, Ordering::SeqCst);
        let end = std::time::Instant::now() + Duration::from_secs(3);
        while self.wait.load(Ordering::SeqCst) && std::time::Instant::now() < end {
            std::thread::sleep(Duration::from_millis(1));
        }
        Ok(SourceView {
            listing: readable_listing(import_id).scan.listings.remove(0),
            body: "visible body".into(),
            raw_front_matter: "title: visible header".into(),
            fields: vec![],
            newline: diary_domain::source::front_matter::Newline::Lf,
            bom: false,
            write: Err(diary_application::source::WriteReason::WriterUnverified),
            trash: Err(diary_application::source::TrashReason::TrashUnverified),
        })
    }
}
#[tokio::test]
async fn selected_read_redacts_metadata_and_refuses_reload_during_work() {
    let mut s = state();
    let fake = Arc::new(ReadFake {
        wait: AtomicBool::new(false),
        started: AtomicBool::new(false),
    });
    ServerState::set_imports(&mut s, fake.clone());
    s.coordinator
        .lock()
        .unwrap()
        .open("07249746-e280-4d51-a86a-cbcb198ee101", s.now())
        .unwrap();
    let app = router(s.clone());
    let uri = "/api/v1/sources/source-id?snapshot_id=import";
    let response = app
        .clone()
        .oneshot(req("GET", uri, None, true))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let dto = value(response).await;
    assert_eq!(dto["body"], "visible body");
    assert_eq!(dto["front_matter"], "title: visible header");
    assert_eq!(dto["capabilities"]["save"], false);
    assert_eq!(dto["capabilities"]["reason"], "writer_unverified");
    assert_eq!(dto["capabilities"]["trash"], false);
    assert_eq!(dto["capabilities"]["trash_reason"], "trash_unverified");
    for private in [
        "root_namespace",
        "mtime_ns",
        "raw_front_matter",
        "\"ino\"",
        "\"dev\"",
    ] {
        assert!(!dto.to_string().contains(private));
    }
    fake.started.store(false, Ordering::SeqCst);
    fake.wait.store(true, Ordering::SeqCst);
    let task = tokio::spawn(app.oneshot(req("GET", uri, None, true)));
    tokio::time::timeout(Duration::from_secs(3), async {
        while !fake.started.load(Ordering::SeqCst) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    s.config_generation.store(2, Ordering::SeqCst);
    fake.wait.store(false, Ordering::SeqCst);
    assert_eq!(
        value(task.await.unwrap().unwrap()).await["error"]["code"],
        "source_stale"
    );
}

#[test]
fn listing_trash_readiness_respects_producer_and_independent_evidence_without_claiming_ownership() {
    let mut listing = readable_listing("import");
    let source = listing.scan.listings[0].clone();
    assert_eq!(
        enhanced_source(&source, &listing, false, true)["capabilities"]["trash_reason"],
        "producer_unverified"
    );
    listing.navigation.as_mut().unwrap().groups[0].kind = "narihito".into();
    for (available, reason) in [(false, "trash_unverified"), (true, "checked_on_preview")] {
        let value = enhanced_source(&source, &listing, false, available);
        assert_eq!(value["capabilities"]["trash"], false);
        assert_eq!(value["capabilities"]["trash_reason"], reason);
        assert!(value.get("members").is_none());
    }
    listing.navigation = None;
    assert_eq!(
        enhanced_source(&source, &listing, false, true)["capabilities"]["trash_reason"],
        "reimport_required"
    );
}

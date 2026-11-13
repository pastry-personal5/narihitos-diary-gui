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
            12
        } else {
            11
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

mod authentic_routes;
mod diagnostics;
mod reload;
mod replay;

#[tokio::test]
async fn configuration_folder_requires_an_active_session_and_configured_location() {
    for configured in [false, true] {
        let mut s = state();
        if configured {
            ServerState::set_configuration_folder(&mut s, "/private/server config".into());
        }
        s.coordinator
            .lock()
            .unwrap()
            .open(SESSION, s.now())
            .unwrap();
        let app = router(s);
        let path = "/api/v1/settings/configuration-folder";
        assert!(
            !app.clone()
                .oneshot(request("GET", path))
                .await
                .unwrap()
                .status()
                .is_success()
        );
        let mut wrong = owned_request("GET", path);
        wrong
            .headers_mut()
            .insert("authorization", "Bearer wrong".parse().unwrap());
        assert_eq!(
            app.clone().oneshot(wrong).await.unwrap().status(),
            StatusCode::UNAUTHORIZED
        );
        let response = app
            .clone()
            .oneshot(owned_request("GET", path))
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            if configured {
                StatusCode::OK
            } else {
                StatusCode::SERVICE_UNAVAILABLE
            }
        );
        if configured {
            let value: Value =
                serde_json::from_slice(&to_bytes(response.into_body(), 10240).await.unwrap())
                    .unwrap();
            assert_eq!(value, json!({"folder":"/private/server config"}));
            let response = app
                .oneshot(owned_request("GET", "/api/v1/settings"))
                .await
                .unwrap();
            let body = to_bytes(response.into_body(), 10240).await.unwrap();
            assert!(!String::from_utf8_lossy(&body).contains("/private/server config"));
        }
    }
}

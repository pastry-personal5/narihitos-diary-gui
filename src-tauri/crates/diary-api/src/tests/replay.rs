use super::*;

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

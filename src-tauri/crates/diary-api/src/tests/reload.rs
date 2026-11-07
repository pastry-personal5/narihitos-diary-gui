use super::*;

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

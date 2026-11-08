//! Read-only settings and the configuration reload route.
use super::*;

pub(crate) fn availability_dto(items: &[diary_application::Availability]) -> Vec<Value> {
    use diary_application::UnavailableReason as Reason;
    items.iter().map(|item|json!({"alias":item.alias,"available":item.reason.is_none(),"reason":match item.reason {
        None=>"ready",Some(Reason::SourceRootMissing)=>"source_root_missing",Some(Reason::SourceRootDenied)=>"source_root_denied",Some(Reason::ProfileHomeMissing)=>"profile_home_missing",Some(Reason::ProfileHomeDenied)=>"profile_home_denied",Some(Reason::ExecutableMissing)=>"executable_missing",Some(Reason::AgentAdapterUnverified)=>"agent_adapter_unverified",Some(Reason::NativeAdapterUnverified)=>"native_adapter_unverified",
    }})).collect()
}
pub(crate) async fn settings(State(s): State<Arc<ServerState>>, headers: HeaderMap) -> Response {
    let id = match session(&headers) {
        Ok(id) => id,
        Err(e) => return app_error(e),
    };
    match s
        .coordinator
        .lock()
        .expect("coordinator poisoned")
        .list(id, s.now(), None, 1)
    {
        Ok(_) => {
            let startup = s.startup.lock().expect("startup state poisoned").clone();
            let value = json!({"config_schema_version":1,"config_generation":s.config_generation.load(std::sync::atomic::Ordering::SeqCst),"last_reload":s.last_reload.lock().expect("reload state poisoned").clone(),"storage_ready":s.storage_ready,"sources_available":startup.sources.iter().any(|s|s.reason.is_none()),"agents_available":s.agent_runner.is_some() && s.profiles.as_ref().is_some_and(|p|p.list().iter().any(|v|v.chat==diary_application::agent::Capability::Ready)),"exports_available":s.exports.as_ref().is_some_and(|e|e.available(diary_application::export::Target::Icloud)),"notion_exports_available":s.exports.as_ref().is_some_and(|e|e.available(diary_application::export::Target::Notion)),"sources":availability_dto(&startup.sources),"profiles":availability_dto(&startup.profiles)});
            #[cfg(feature = "scaffold-smoke")]
            let value = {
                let mut value = value;
                value["folder_checks"] =
                    serde_json::to_value(&s.folder_checks).expect("fixture DTO");
                value
            };
            Json(value).into_response()
        }
        Err(e) => app_error(e),
    }
}
/// Validates a complete new config first; any failure leaves the last one in force.
pub(crate) async fn reload_settings(
    State(s): State<Arc<ServerState>>,
    headers: HeaderMap,
) -> Response {
    let id = match session(&headers) {
        Ok(id) => id,
        Err(e) => return app_error(e),
    };
    let busy =
        match s
            .coordinator
            .lock()
            .expect("coordinator poisoned")
            .list(id, s.now(), None, 200)
        {
            Ok(jobs) => jobs.iter().any(|job| !job.state.is_terminal()),
            Err(e) => return app_error(e),
        };
    // Paused or uncertain exports survive restarts, so the ledger decides, not session jobs.
    let busy = busy
        || s.exports
            .as_ref()
            .is_some_and(|e| e.has_unfinished().unwrap_or(true));
    let Some(reloader) = s.reloader.clone() else {
        return failure(StatusCode::SERVICE_UNAVAILABLE, "unavailable");
    };
    let issues = |list: &[diary_application::ReloadIssue]| -> Vec<Value> {
        list.iter()
            .map(|i| json!({"path":i.path,"code":i.code}))
            .collect()
    };
    let result = match tokio::task::spawn_blocking(move || reloader.reload(busy)).await {
        Ok(diary_application::ReloadOutcome::Applied(summary)) => {
            *s.startup.lock().expect("startup state poisoned") = summary;
            let generation = s
                .config_generation
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                + 1;
            json!({"outcome":"applied","config_generation":generation})
        }
        Ok(diary_application::ReloadOutcome::RestartRequired(list)) => {
            json!({"outcome":"restart_required","issues":issues(&list)})
        }
        Ok(diary_application::ReloadOutcome::Rejected(list)) => {
            json!({"outcome":"rejected","issues":issues(&list)})
        }
        Ok(diary_application::ReloadOutcome::Busy) => json!({"outcome":"busy"}),
        Err(_) => return failure(StatusCode::SERVICE_UNAVAILABLE, "unavailable"),
    };
    *s.last_reload.lock().expect("reload state poisoned") = result.clone();
    Json(result).into_response()
}

/// Native-only consumer: this path is never returned to the webview or diagnostic logs.
pub(crate) async fn configuration_folder(
    State(s): State<Arc<ServerState>>,
    headers: HeaderMap,
) -> Response {
    let id = match session(&headers) {
        Ok(id) => id,
        Err(e) => return app_error(e),
    };
    if let Err(e) = s
        .coordinator
        .lock()
        .expect("coordinator poisoned")
        .list(id, s.now(), None, 1)
    {
        return app_error(e);
    }
    match s.configuration_folder.as_ref().and_then(|p| p.to_str()) {
        Some(folder) => Json(json!({"folder":folder})).into_response(),
        None => failure(StatusCode::SERVICE_UNAVAILABLE, "unavailable"),
    }
}

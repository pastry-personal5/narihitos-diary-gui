//! Connection, session lease, settings, user settings and the job list.
use crate::*;

#[tauri::command]
pub async fn connect_server(
    window: WebviewWindow,
    state: State<'_, Bridge>,
) -> Result<Status, &'static str> {
    main_window(&window)?;
    let result = state.connect().await;
    #[cfg(feature = "scaffold-smoke")]
    if result.is_ok() {
        eprintln!("SMOKE main_handshake_allowed");
    }
    result
}
#[tauri::command]
pub async fn get_status(
    window: WebviewWindow,
    state: State<'_, Bridge>,
) -> Result<Status, &'static str> {
    main_window(&window)?;
    state.status().await
}
#[tauri::command]
pub async fn open_session(
    window: WebviewWindow,
    state: State<'_, Bridge>,
) -> Result<Lease, &'static str> {
    main_window(&window)?;
    let result = state.open().await;
    #[cfg(feature = "scaffold-smoke")]
    if result.as_ref().err() == Some(&"client_busy") {
        eprintln!("SMOKE client_busy_rejected");
    }
    result
}
#[tauri::command]
pub async fn heartbeat_session(
    window: WebviewWindow,
    state: State<'_, Bridge>,
) -> Result<(), &'static str> {
    main_window(&window)?;
    state.heartbeat().await
}
#[tauri::command]
pub async fn close_session(
    window: WebviewWindow,
    state: State<'_, Bridge>,
) -> Result<(), &'static str> {
    main_window(&window)?;
    let result = state.close().await;
    #[cfg(feature = "scaffold-smoke")]
    if result.is_ok() {
        eprintln!("SMOKE main_session_released");
    }
    result
}
#[tauri::command]
pub async fn get_settings(
    window: WebviewWindow,
    state: State<'_, Bridge>,
) -> Result<Settings, &'static str> {
    main_window(&window)?;
    state.settings().await
}
#[tauri::command]
pub async fn reload_settings(
    window: WebviewWindow,
    state: State<'_, Bridge>,
) -> Result<ReloadResult, &'static str> {
    main_window(&window)?;
    state.reload_settings().await
}
#[tauri::command]
pub async fn get_user_settings(
    window: WebviewWindow,
    store: State<'_, user_settings::Store>,
    log: State<'_, Arc<ClientLog>>,
) -> Result<user_settings::Snapshot, &'static str> {
    main_window(&window)?;
    let snapshot = store.snapshot();
    log.log(
        Record::new(EventCode::SettingsLoaded, Level::Info)
            .count(u32::try_from(snapshot.warnings.len()).unwrap_or(u32::MAX)),
    );
    Ok(snapshot)
}
#[derive(serde::Serialize)]
pub struct UserSettingsError {
    code: &'static str,
}
/// `section` is `writing`, `writing_init` (value: preferred tags) or `keybindings`; `recover`
/// ignores both and replaces an invalid or newer file after a private backup.
#[tauri::command]
pub async fn update_user_settings(
    window: WebviewWindow,
    store: State<'_, user_settings::Store>,
    log: State<'_, Arc<ClientLog>>,
    section: String,
    value: serde_json::Value,
    expected_revision: String,
    recover: bool,
) -> Result<user_settings::Snapshot, UserSettingsError> {
    use user_settings::{Change, Error};
    let fail = |e: Error| {
        let tag = match e {
            Error::Conflict => ErrorTag::StaleRevision,
            Error::Uncertain | Error::Unavailable => ErrorTag::Unavailable,
            _ => ErrorTag::Invalid,
        };
        log.log(Record::new(EventCode::SettingsFailed, Level::Warn).error(tag));
        UserSettingsError { code: e.code() }
    };
    if main_window(&window).is_err() {
        return Err(UserSettingsError {
            code: "permission_denied",
        });
    }
    let change = match (recover, section.as_str(), value) {
        _ if expected_revision.len() > 64 => Err(Error::Invalid),
        (true, ..) => Ok(Change::Recover),
        (_, "writing", serde_json::Value::String(tag)) => Ok(Change::Writing(tag)),
        (_, "writing_init", serde_json::Value::Array(items)) if items.len() <= 32 => items
            .into_iter()
            .map(|v| v.as_str().map(str::to_owned).ok_or(Error::Invalid))
            .collect::<Result<_, _>>()
            .map(Change::InitWriting),
        (_, "agent_progress", v) => user_settings::agent_progress(&v)
            .map(Change::AgentProgress)
            .ok_or(Error::Invalid),
        (_, "status_line", v) => user_settings::status_line(&v)
            .map(Change::StatusLine)
            .ok_or(Error::Invalid),
        (_, "generation", v) => user_settings::generation_count(&v)
            .map(Change::Generation)
            .ok_or(Error::Invalid),
        (_, "keybindings", v) => Ok(Change::Keybindings(v)),
        _ => Err(Error::Invalid),
    }
    .map_err(fail)?;
    let snapshot = store.update(&expected_revision, change).map_err(fail)?;
    log.log(Record::new(EventCode::SettingsSaved, Level::Info));
    Ok(snapshot)
}
#[tauri::command]
pub async fn list_jobs(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    cursor: Option<String>,
) -> Result<JobList, &'static str> {
    main_window(&window)?;
    state.jobs(cursor).await
}

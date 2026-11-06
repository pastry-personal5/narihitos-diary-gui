mod bridge;
mod delivery;
mod diagnostics;
mod discovery;
mod events;
#[cfg(feature = "scaffold-smoke")]
mod native_smoke;
mod symbols;
mod user_settings;
use bridge::{Bridge, Job, JobList, Lease, Metadata, ReloadResult, Settings, Status};
use diagnostics::{ClientEvent, ClientLog, Reject};
use diary_application::diagnostics::{ErrorTag, EventCode, Level, Record};
use std::sync::{Arc, Mutex};
use tauri::{Manager, State, WebviewWindow, ipc::Channel};

/// The WebView registers here so a close request first flushes pending Authentic saves. With
/// no registered guard (or a dead channel) the window closes immediately, as before.
#[derive(Default)]
struct CloseGate(Mutex<Option<Channel<()>>>);

async fn release_and_close(
    app: tauri::AppHandle,
    window: tauri::Window,
) -> Result<(), &'static str> {
    app.state::<Bridge>().close().await?;
    app.state::<Arc<ClientLog>>()
        .shutdown(std::time::Duration::from_millis(500));
    #[cfg(feature = "scaffold-smoke")]
    eprintln!("SMOKE main_window_close_released");
    window.destroy().map_err(|_| "window_close_failed")
}
#[tauri::command]
fn watch_close(
    window: WebviewWindow,
    gate: State<'_, CloseGate>,
    on_close: Channel<()>,
) -> Result<(), &'static str> {
    main_window(&window)?;
    *gate.0.lock().map_err(|_| "unavailable")? = Some(on_close);
    Ok(())
}
#[tauri::command]
async fn finish_close(window: WebviewWindow, app: tauri::AppHandle) -> Result<(), &'static str> {
    main_window(&window)?;
    release_and_close(app, window.as_ref().window()).await
}
fn main_window(window: &WebviewWindow) -> Result<(), &'static str> {
    if window.label() == "main" {
        Ok(())
    } else {
        Err("permission_denied")
    }
}
#[tauri::command]
async fn connect_server(
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
async fn get_status(
    window: WebviewWindow,
    state: State<'_, Bridge>,
) -> Result<Status, &'static str> {
    main_window(&window)?;
    state.status().await
}
#[tauri::command]
async fn open_session(
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
async fn heartbeat_session(
    window: WebviewWindow,
    state: State<'_, Bridge>,
) -> Result<(), &'static str> {
    main_window(&window)?;
    state.heartbeat().await
}
#[tauri::command]
async fn close_session(
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
async fn get_settings(
    window: WebviewWindow,
    state: State<'_, Bridge>,
) -> Result<Settings, &'static str> {
    main_window(&window)?;
    state.settings().await
}
#[tauri::command]
async fn reload_settings(
    window: WebviewWindow,
    state: State<'_, Bridge>,
) -> Result<ReloadResult, &'static str> {
    main_window(&window)?;
    state.reload_settings().await
}
#[tauri::command]
async fn get_user_settings(
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
struct UserSettingsError {
    code: &'static str,
}
/// `section` is `writing`, `writing_init` (value: preferred tags) or `keybindings`; `recover`
/// ignores both and replaces an invalid or newer file after a private backup.
#[tauri::command]
async fn update_user_settings(
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
async fn list_jobs(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    cursor: Option<String>,
) -> Result<JobList, &'static str> {
    main_window(&window)?;
    state.jobs(cursor).await
}
#[tauri::command]
async fn get_calendar(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    anchor: Option<String>,
    action: bridge::CalendarAction,
) -> Result<bridge::Calendar, bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.get_calendar(anchor, action).await
}
#[tauri::command]
async fn list_agents(
    window: WebviewWindow,
    state: State<'_, Bridge>,
) -> Result<bridge::ProfileList, bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.list_profiles().await
}
#[tauri::command]
async fn check_agent(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    alias: String,
) -> Result<bridge::ProfileStatus, bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.check_profile(alias).await
}
#[tauri::command]
async fn open_chat(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    profile: String,
    locale: String,
) -> Result<bridge::ChatOpened, bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.open_chat(profile, locale).await
}
#[tauri::command]
async fn send_chat(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    chat_id: String,
    request_id: String,
    message: String,
    document: Option<bridge::ChatDocument>,
) -> Result<bridge::TurnAccepted, bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state
        .send_chat(chat_id, request_id, message, document)
        .await
}
#[tauri::command]
async fn get_chat(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    chat_id: String,
) -> Result<bridge::ChatView, bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.get_chat(chat_id).await
}
#[tauri::command]
async fn close_chat(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    chat_id: String,
) -> Result<(), bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.close_chat(chat_id).await
}
#[tauri::command]
async fn start_generation(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    input: bridge::GenerationInput,
) -> Result<bridge::TurnAccepted, bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.start_generation(input).await
}
#[tauri::command]
async fn get_generation(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    job_id: String,
) -> Result<bridge::GenerationView, bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.get_generation(job_id).await
}
#[tauri::command]
async fn read_candidate(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    candidate_id: String,
) -> Result<bridge::CandidateView, bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.read_candidate(candidate_id).await
}
#[tauri::command]
async fn discard_generation(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    job_id: String,
) -> Result<(), bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.discard_generation(job_id).await
}
#[tauri::command]
async fn read_source(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    source_id: String,
    snapshot_id: String,
) -> Result<bridge::SourceBody, bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.read_source(source_id, snapshot_id).await
}
#[tauri::command]
async fn save_source(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    source_id: String,
    operation_id: String,
    base_digest: String,
    body: String,
) -> Result<bridge::SourceSaved, bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state
        .save_source(source_id, operation_id, base_digest, body)
        .await
}
#[tauri::command]
async fn preview_source_delete(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    source_id: String,
    request: bridge::TrashPreviewRequest,
) -> Result<bridge::TrashPreview, bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.preview_source_delete(source_id, request).await
}
#[tauri::command]
async fn confirm_source_delete(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    source_id: String,
    request: bridge::TrashConfirmRequest,
) -> Result<bridge::TrashAcknowledged, bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.confirm_source_delete(source_id, request).await
}
#[tauri::command]
async fn list_source_deletions(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    cursor: Option<String>,
    limit: Option<u16>,
) -> Result<bridge::TrashPage, bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.list_source_deletions(cursor, limit).await
}
#[tauri::command]
async fn import_date(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    date: String,
    request_id: String,
) -> Result<bridge::ImportAck, bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.import_date(date, request_id).await
}
#[tauri::command]
async fn list_sources(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    import_id: String,
    cursor: Option<usize>,
) -> Result<bridge::ImportPage, bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.list_sources(import_id, cursor).await
}
#[tauri::command]
async fn resolve_source_time(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    import_id: String,
    source_id: String,
    digest: String,
    authority: bridge::TimeAuthority,
) -> Result<bridge::ResolutionAck, bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state
        .resolve_source_time(import_id, source_id, digest, authority)
        .await
}
#[tauri::command]
async fn list_authentic(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    cursor: Option<String>,
) -> Result<bridge::AuthenticList, bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.list_authentic(cursor).await
}
#[tauri::command]
async fn load_authentic(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    date: String,
) -> Result<bridge::AuthenticFile, bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.load_authentic(date).await
}
#[tauri::command]
async fn save_authentic(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    date: String,
    operation_id: String,
    expected_revision: Option<bridge::Revision>,
    body: String,
    buffer_generation: u64,
) -> Result<bridge::SaveAck, bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state
        .save_authentic(
            date,
            operation_id,
            expected_revision,
            body,
            buffer_generation,
        )
        .await
}
#[tauri::command]
async fn create_empty_authentic(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    date: String,
    operation_id: String,
) -> Result<bridge::SaveAck, bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.create_empty_authentic(date, operation_id).await
}
#[tauri::command]
async fn refresh_authentic(
    window: WebviewWindow,
    state: State<'_, Bridge>,
) -> Result<bridge::RefreshReport, bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.refresh_authentic().await
}
#[tauri::command]
async fn get_operation(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    operation_id: String,
) -> Result<bridge::OperationState, bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.get_operation(operation_id).await
}
#[tauri::command]
async fn get_job(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    id: String,
) -> Result<Job, &'static str> {
    main_window(&window)?;
    let result = state.job(id, false).await;
    #[cfg(feature = "scaffold-smoke")]
    if result
        .as_ref()
        .is_ok_and(|job| matches!(job.state, bridge::JobState::Cancelled))
    {
        eprintln!("SMOKE main_cancelled_job_read");
    }
    #[cfg(feature = "scaffold-smoke")]
    if result
        .as_ref()
        .is_ok_and(|job| matches!(job.state, bridge::JobState::Running))
    {
        eprintln!("SMOKE main_running_job_read");
    }
    result
}
#[tauri::command]
async fn cancel_job(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    id: String,
) -> Result<Job, &'static str> {
    main_window(&window)?;
    state.job(id, true).await
}
#[tauri::command]
async fn subscribe_events(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    request: delivery::SubscriptionRequest,
) -> Result<(), &'static str> {
    main_window(&window)?;
    match request {
        delivery::SubscriptionRequest::Start { channel } => {
            state
                .subscribe(channel.channel_on(window.as_ref().clone()))
                .await
        }
        delivery::SubscriptionRequest::Acknowledge {
            subscription_id,
            delivery_id,
        } => state.acknowledge(&subscription_id, delivery_id),
    }
}
#[tauri::command]
fn get_app_metadata(
    window: WebviewWindow,
    app: tauri::AppHandle,
) -> Result<Metadata, &'static str> {
    main_window(&window)?;
    #[cfg(feature = "scaffold-smoke")]
    eprintln!("SMOKE main_metadata_allowed");
    Ok(Metadata {
        version: app.package_info().version.to_string(),
    })
}
#[tauri::command]
async fn get_symbol(
    window: WebviewWindow,
    app: tauri::AppHandle,
    name: String,
    size: u16,
    scale: u8,
) -> Result<symbols::SymbolResult, &'static str> {
    main_window(&window)?;
    symbols::validate(&name, size, scale).map_err(|_| "invalid_symbol")?;
    // AppKit drawing must run on the main thread; an async command keeps that thread free to do it.
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.run_on_main_thread(move || {
        let _ = tx.send(symbols::render(&name, size, scale));
    })
    .map_err(|_| "symbol_unavailable")?;
    match tokio::time::timeout(std::time::Duration::from_secs(2), rx).await {
        Ok(Ok(outcome)) => Ok(outcome.into()),
        _ => Err("symbol_unavailable"),
    }
}
#[tauri::command]
fn log_client_event(
    window: WebviewWindow,
    log: State<'_, Arc<ClientLog>>,
    event: ClientEvent,
) -> Result<(), &'static str> {
    main_window(&window)?;
    log.client_event(event).map_err(|reject| match reject {
        Reject::Rate => "rate_limited",
        Reject::Invalid => "invalid_event",
    })
}
fn main() {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() != Some(std::ffi::OsStr::new("--connect")) {
        eprintln!("diary-client: expected --connect PATH");
        std::process::exit(1);
    }
    let Some(path) = args.next() else {
        eprintln!("diary-client: expected discovery path");
        std::process::exit(1);
    };
    if args.next().is_some() {
        eprintln!("diary-client: invalid arguments");
        std::process::exit(1);
    }
    let bridge = Bridge::new(path.into()).expect("HTTP bridge setup");
    let log = bridge.log();
    tauri::Builder::default()
        .manage(bridge)
        .manage(log)
        .manage(CloseGate::default())
        .setup(|app| {
            app.manage(user_settings::Store::new(app.path().app_config_dir()?));
            #[cfg(feature = "scaffold-smoke")]
            native_smoke::setup(app)?;
            Ok(())
        })
        .on_page_load(|_webview, _payload| {
            #[cfg(feature = "scaffold-smoke")]
            if _webview.label() == "main"
                && matches!(_payload.event(), tauri::webview::PageLoadEvent::Finished)
            {
                let _ = native_smoke::main_loaded(_webview);
            }
        })
        .invoke_handler(tauri::generate_handler![
            connect_server,
            get_status,
            open_session,
            heartbeat_session,
            close_session,
            get_settings,
            reload_settings,
            get_user_settings,
            update_user_settings,
            get_job,
            list_jobs,
            cancel_job,
            subscribe_events,
            get_app_metadata,
            log_client_event,
            get_symbol,
            get_calendar,
            list_agents,
            check_agent,
            open_chat,
            send_chat,
            get_chat,
            close_chat,
            start_generation,
            get_generation,
            read_candidate,
            discard_generation,
            read_source,
            save_source,
            preview_source_delete,
            confirm_source_delete,
            list_source_deletions,
            import_date,
            list_sources,
            resolve_source_time,
            list_authentic,
            load_authentic,
            save_authentic,
            create_empty_authentic,
            refresh_authentic,
            get_operation,
            watch_close,
            finish_close
        ])
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }
            #[cfg(feature = "scaffold-smoke")]
            if matches!(event, tauri::WindowEvent::Focused(true)) {
                if let Some(webview) = window.app_handle().get_webview_window("main") {
                    let _ = tauri::Webview::set_focus(webview.as_ref());
                    let _ = webview.eval("document.querySelector('.cm-content')?.focus()");
                }
            }
            #[cfg(feature = "scaffold-smoke")]
            if matches!(event, tauri::WindowEvent::Destroyed) {
                eprintln!("SMOKE main_window_destroyed");
                window.app_handle().exit(0);
            }
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let app = window.app_handle().clone();
                let window = window.clone();
                let guarded = app
                    .state::<CloseGate>()
                    .0
                    .lock()
                    .ok()
                    .and_then(|gate| gate.as_ref().map(|c| c.send(()).is_ok()))
                    .unwrap_or(false);
                if !guarded {
                    tauri::async_runtime::spawn(release_and_close(app, window));
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("Tauri runtime");
}

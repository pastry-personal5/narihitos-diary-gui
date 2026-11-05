mod bridge;
mod delivery;
mod diagnostics;
mod discovery;
mod events;
#[cfg(feature = "scaffold-smoke")]
mod native_smoke;
use bridge::{Bridge, Job, JobList, Lease, Metadata, Settings, Status};
use diagnostics::{ClientEvent, ClientLog, Reject};
use std::sync::{Arc, Mutex};
use tauri::{Manager, State, WebviewWindow, ipc::Channel};

/// The WebView registers here so a close request first flushes pending Authentic saves. With
/// no registered guard (or a dead channel) the window closes immediately, as before.
#[derive(Default)]
struct CloseGate(Mutex<Option<Channel<()>>>);

async fn release_and_close(app: tauri::AppHandle, window: tauri::Window) {
    let _result = app.state::<Bridge>().close().await;
    app.state::<Arc<ClientLog>>()
        .shutdown(std::time::Duration::from_millis(500));
    #[cfg(feature = "scaffold-smoke")]
    if _result.is_ok() {
        eprintln!("SMOKE main_window_close_released");
    }
    let _ = window.destroy();
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
    release_and_close(app, window.as_ref().window()).await;
    Ok(())
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
async fn list_jobs(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    cursor: Option<String>,
) -> Result<JobList, &'static str> {
    main_window(&window)?;
    state.jobs(cursor).await
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
        .setup(|_app| {
            #[cfg(feature = "scaffold-smoke")]
            native_smoke::setup(_app)?;
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
            get_job,
            list_jobs,
            cancel_job,
            subscribe_events,
            get_app_metadata,
            log_client_event,
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

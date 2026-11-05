mod bridge;
mod delivery;
mod diagnostics;
mod discovery;
mod events;
#[cfg(feature = "scaffold-smoke")]
mod native_smoke;
use bridge::{Bridge, Job, JobList, Lease, Metadata, Settings, Status};
use diagnostics::{ClientEvent, ClientLog, Reject};
use std::sync::Arc;
use tauri::{Manager, State, WebviewWindow};
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
            log_client_event
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
                tauri::async_runtime::spawn(async move {
                    // The scaffold has only a disposable scratch buffer. Future
                    // durable/source editing supplies its own guarded flush here.
                    let _result = app.state::<Bridge>().close().await;
                    app.state::<Arc<ClientLog>>()
                        .shutdown(std::time::Duration::from_millis(500));
                    #[cfg(feature = "scaffold-smoke")]
                    if _result.is_ok() {
                        eprintln!("SMOKE main_window_close_released");
                    }
                    let _ = window.destroy();
                });
            }
        })
        .run(tauri::generate_context!())
        .expect("Tauri runtime");
}

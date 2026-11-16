mod bridge;
mod commands;
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
            let settings_dir = app.path().app_config_dir()?;
            #[cfg(feature = "scaffold-smoke")]
            let settings_dir = if native_smoke::illustrated_enabled() {
                std::path::PathBuf::from(
                    std::env::var("DIARY_ILLUSTRATED_SMOKE_SETTINGS")
                        .map_err(|_| "missing isolated smoke settings")?,
                )
            } else {
                settings_dir
            };
            app.manage(user_settings::Store::new(settings_dir));
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
            commands::session::connect_server,
            commands::session::get_status,
            commands::session::open_session,
            commands::session::heartbeat_session,
            commands::session::close_session,
            commands::session::get_settings,
            commands::session::reload_settings,
            commands::events::reveal_configuration_folder,
            commands::session::get_user_settings,
            commands::session::update_user_settings,
            commands::events::get_job,
            commands::session::list_jobs,
            commands::events::cancel_job,
            commands::events::subscribe_events,
            commands::events::get_app_metadata,
            commands::events::log_client_event,
            commands::events::get_symbol,
            commands::ai::get_calendar,
            commands::ai::list_agents,
            commands::ai::check_agent,
            commands::ai::open_chat,
            commands::ai::send_chat,
            commands::ai::get_chat,
            commands::ai::close_chat,
            commands::ai::start_generation,
            commands::ai::get_generation,
            commands::ai::read_candidate,
            commands::ai::discard_generation,
            commands::ai::start_review,
            commands::ai::decide_review,
            commands::ai::end_review,
            commands::sources::read_source,
            commands::sources::save_source,
            commands::sources::preview_source_delete,
            commands::sources::confirm_source_delete,
            commands::sources::list_source_deletions,
            commands::sources::start_export,
            commands::sources::confirm_export,
            commands::sources::abandon_export,
            commands::sources::list_exports,
            commands::sources::confirm_abandon_export,
            commands::sources::import_date,
            commands::sources::list_sources,
            commands::sources::resolve_source_time,
            commands::illustrated::list_image_engines,
            commands::illustrated::check_image_engine,
            commands::illustrated::list_illustrated,
            commands::illustrated::read_illustrated,
            commands::illustrated::create_illustrated,
            commands::illustrated::delete_illustrated,
            commands::illustrated::read_illustrated_photo,
            commands::authentic::list_authentic,
            commands::authentic::load_authentic,
            commands::authentic::save_authentic,
            commands::authentic::create_empty_authentic,
            commands::authentic::refresh_authentic,
            commands::authentic::get_operation,
            watch_close,
            finish_close
        ])
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }
            #[cfg(feature = "scaffold-smoke")]
            if matches!(event, tauri::WindowEvent::Focused(true))
                && let Some(webview) = window.app_handle().get_webview_window("main")
            {
                let _ = tauri::Webview::set_focus(webview.as_ref());
                let _ = webview.eval("document.querySelector('.cm-content')?.focus()");
            }
            if matches!(event, tauri::WindowEvent::Destroyed) {
                #[cfg(feature = "scaffold-smoke")]
                eprintln!("SMOKE main_window_destroyed");
                // Main-window destruction follows the acknowledged close above. Exit even
                // if auxiliary windows remain so the dev CLI can stop its frontend tree.
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

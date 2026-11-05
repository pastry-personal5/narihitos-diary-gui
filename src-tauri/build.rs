const COMMANDS: &[&str] = &[
    "connect_server",
    "get_status",
    "open_session",
    "heartbeat_session",
    "close_session",
    "get_settings",
    "get_job",
    "list_jobs",
    "cancel_job",
    "subscribe_events",
    "get_app_metadata",
    "log_client_event",
    "list_authentic",
    "load_authentic",
    "save_authentic",
    "create_empty_authentic",
    "refresh_authentic",
    "get_operation",
    "watch_close",
    "finish_close",
];
fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS)),
    )
    .expect("Tauri build configuration");
}

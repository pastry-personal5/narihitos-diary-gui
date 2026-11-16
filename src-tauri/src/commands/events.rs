//! Jobs, the event stream, app metadata, symbols, client log and the configuration folder.
use crate::*;

#[tauri::command]
pub async fn get_job(
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
pub async fn cancel_job(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    id: String,
) -> Result<Job, &'static str> {
    main_window(&window)?;
    state.job(id, true).await
}
#[tauri::command]
pub async fn subscribe_events(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    request: delivery::SubscriptionRequest,
) -> Result<(), &'static str> {
    main_window(&window)?;
    match request {
        delivery::SubscriptionRequest::Stop => {
            state.detach_events().await;
            Ok(())
        }
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
pub fn get_app_metadata(
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
pub async fn get_symbol(
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
pub fn log_client_event(
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
#[tauri::command]
pub async fn reveal_configuration_folder(
    window: WebviewWindow,
    state: State<'_, Bridge>,
) -> Result<(), &'static str> {
    main_window(&window)?;
    state.reveal_configuration_folder().await
}

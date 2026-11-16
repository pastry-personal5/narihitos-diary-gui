//! Authentic entries and durable operations.
use crate::*;

#[tauri::command]
pub async fn list_authentic(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    cursor: Option<String>,
) -> Result<bridge::AuthenticList, bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.list_authentic(cursor).await
}
#[tauri::command]
pub async fn load_authentic(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    date: String,
) -> Result<bridge::AuthenticFile, bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.load_authentic(date).await
}
#[tauri::command]
pub async fn save_authentic(
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
pub async fn create_empty_authentic(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    date: String,
    operation_id: String,
) -> Result<bridge::SaveAck, bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.create_empty_authentic(date, operation_id).await
}
#[tauri::command]
pub async fn refresh_authentic(
    window: WebviewWindow,
    state: State<'_, Bridge>,
) -> Result<bridge::RefreshReport, bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.refresh_authentic().await
}
#[tauri::command]
pub async fn get_operation(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    operation_id: String,
) -> Result<bridge::OperationState, bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.get_operation(operation_id).await
}

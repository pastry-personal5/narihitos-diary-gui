//! Source bodies, Move to Trash, exports, imports and time resolution.
use crate::*;

#[tauri::command]
pub async fn read_source(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    source_id: String,
    snapshot_id: String,
) -> Result<bridge::SourceBody, bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.read_source(source_id, snapshot_id).await
}
#[tauri::command]
pub async fn save_source(
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
pub async fn preview_source_delete(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    source_id: String,
    request: bridge::TrashPreviewRequest,
) -> Result<bridge::TrashPreview, bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.preview_source_delete(source_id, request).await
}
#[tauri::command]
pub async fn confirm_source_delete(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    source_id: String,
    request: bridge::TrashConfirmRequest,
) -> Result<bridge::TrashAcknowledged, bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.confirm_source_delete(source_id, request).await
}
#[tauri::command]
pub async fn list_source_deletions(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    cursor: Option<String>,
    limit: Option<u16>,
) -> Result<bridge::TrashPage, bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.list_source_deletions(cursor, limit).await
}
#[tauri::command]
pub async fn start_export(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    request: bridge::StartExportRequest,
) -> Result<bridge::StartExportResult, bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.start_export(request).await
}
#[tauri::command]
pub async fn confirm_export(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    operation_id: String,
    request: bridge::ConfirmExportRequest,
) -> Result<bridge::ExportAcknowledged, bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.confirm_export(operation_id, request).await
}
#[tauri::command]
pub async fn abandon_export(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    operation_id: String,
) -> Result<bridge::StartExportResult, bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.abandon_export(operation_id).await
}
#[tauri::command]
pub async fn confirm_abandon_export(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    operation_id: String,
    request: bridge::ConfirmExportRequest,
) -> Result<bridge::ExportResult, bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.confirm_abandon_export(operation_id, request).await
}
#[tauri::command]
pub async fn list_exports(
    window: WebviewWindow,
    state: State<'_, Bridge>,
) -> Result<bridge::ExportPage, bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.list_exports().await
}
#[tauri::command]
pub async fn import_date(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    date: String,
    request_id: String,
) -> Result<bridge::ImportAck, bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.import_date(date, request_id).await
}
#[tauri::command]
pub async fn list_sources(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    import_id: String,
    cursor: Option<usize>,
) -> Result<bridge::ImportPage, bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.list_sources(import_id, cursor).await
}
#[tauri::command]
pub async fn resolve_source_time(
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

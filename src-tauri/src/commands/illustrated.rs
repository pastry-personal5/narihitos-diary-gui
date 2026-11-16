//! Image engines and the Illustrated diary.
use crate::*;

#[tauri::command]
pub async fn list_image_engines(
    window: WebviewWindow,
    state: State<'_, Bridge>,
) -> Result<bridge::ImageEngines, bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.list_image_engines().await
}
#[tauri::command]
pub async fn check_image_engine(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    id: String,
) -> Result<bridge::ImageEngine, bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.check_image_engine(id).await
}
#[tauri::command]
pub async fn list_illustrated(
    window: WebviewWindow,
    state: State<'_, Bridge>,
) -> Result<bridge::IllustratedList, bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.list_illustrated().await
}
#[tauri::command]
pub async fn read_illustrated(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    date: String,
) -> Result<bridge::IllustratedEntry, bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.read_illustrated(date).await
}
#[tauri::command]
pub async fn create_illustrated(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    date: String,
    input: bridge::IllustratedInput,
) -> Result<bridge::TurnAccepted, bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.create_illustrated(date, input).await
}
#[tauri::command]
pub async fn delete_illustrated(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    date: String,
    version: String,
    confirmed: bool,
) -> Result<(), bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.delete_illustrated(date, version, confirmed).await
}
#[tauri::command]
pub async fn read_illustrated_photo(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    date: String,
    version: String,
    n: u8,
) -> Result<Vec<u8>, bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.read_illustrated_photo(date, version, n).await
}

//! Calendar, AI agents, chat, generation and candidate review.
use crate::*;

#[tauri::command]
pub async fn get_calendar(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    anchor: Option<String>,
    action: bridge::CalendarAction,
) -> Result<bridge::Calendar, bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.get_calendar(anchor, action).await
}
#[tauri::command]
pub async fn list_agents(
    window: WebviewWindow,
    state: State<'_, Bridge>,
) -> Result<bridge::ProfileList, bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.list_profiles().await
}
#[tauri::command]
pub async fn check_agent(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    alias: String,
) -> Result<bridge::ProfileStatus, bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.check_profile(alias).await
}
#[tauri::command]
pub async fn open_chat(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    profile: String,
    locale: String,
    purpose: Option<String>,
) -> Result<bridge::ChatOpened, bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    if let Some(purpose) = purpose {
        state.open_chat_kind(profile, locale, &purpose).await
    } else {
        state.open_chat(profile, locale).await
    }
}
#[tauri::command]
pub async fn send_chat(
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
pub async fn get_chat(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    chat_id: String,
) -> Result<bridge::ChatView, bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.get_chat(chat_id).await
}
#[tauri::command]
pub async fn close_chat(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    chat_id: String,
) -> Result<(), bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.close_chat(chat_id).await
}
#[tauri::command]
pub async fn start_generation(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    input: bridge::GenerationInput,
) -> Result<bridge::TurnAccepted, bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.start_generation(input).await
}
#[tauri::command]
pub async fn get_generation(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    job_id: String,
) -> Result<bridge::GenerationView, bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.get_generation(job_id).await
}
#[tauri::command]
pub async fn read_candidate(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    candidate_id: String,
) -> Result<bridge::CandidateView, bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.read_candidate(candidate_id).await
}
#[tauri::command]
pub async fn discard_generation(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    job_id: String,
) -> Result<(), bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.discard_generation(job_id).await
}
#[tauri::command]
pub async fn start_review(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    input: bridge::StartReviewInput,
) -> Result<bridge::ReviewView, bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.start_review(input).await
}
#[tauri::command]
pub async fn decide_review(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    input: bridge::DecideReviewInput,
) -> Result<bridge::ReviewDecision, bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.decide_review(input).await
}
#[tauri::command]
pub async fn end_review(
    window: WebviewWindow,
    state: State<'_, Bridge>,
    review_id: String,
) -> Result<(), bridge::AuthenticError> {
    main_window(&window).map_err(|_| bridge::AuthenticError::denied())?;
    state.end_review(review_id).await
}

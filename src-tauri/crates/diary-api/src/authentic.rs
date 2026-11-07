//! Authentic routes: saved-date catalog, load, guarded save, Create Empty, refresh and durable
//! operation lookup. Every route needs an admitted session; store calls run off the async
//! executor; responses and errors never carry body text or paths.
// Helpers return the finished HTTP `Response` as their error so handlers can return it directly.
use super::{ServerState, app_error, failure, session, traced};
use axum::{
    Extension, Json,
    extract::{Path, Query, State, rejection::JsonRejection},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use diary_application::{
    authentic::{
        AuthenticStore, ConflictKind, CreateRequest, Loaded, SaveOutcome, SaveRequest, StoreError,
    },
    diagnostics::{
        EventCode, Level, OpaqueId, Record, ResourceDetail, ResourceKind, StateTag, TraceId,
    },
    operations::{OperationId, OperationState},
};
use diary_domain::authentic::{AuthenticRevision, DiaryDate, FormatError};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

fn authorize(s: &ServerState, headers: &HeaderMap) -> Result<(), Response> {
    let id = session(headers).map_err(app_error)?;
    s.coordinator
        .lock()
        .expect("coordinator poisoned")
        .list(id, s.now(), None, 1)
        .map(|_| ())
        .map_err(app_error)
}

fn store(s: &ServerState) -> Result<Arc<dyn AuthenticStore>, Response> {
    s.authentic
        .clone()
        .ok_or_else(|| failure(StatusCode::SERVICE_UNAVAILABLE, "storage_unavailable"))
}

async fn blocking<T: Send + 'static>(
    store: Arc<dyn AuthenticStore>,
    f: impl FnOnce(&dyn AuthenticStore) -> Result<T, StoreError> + Send + 'static,
) -> Result<T, Response> {
    tokio::task::spawn_blocking(move || f(&*store))
        .await
        .map_err(|_| failure(StatusCode::INTERNAL_SERVER_ERROR, "internal"))?
        .map_err(store_error)
}

pub(super) fn store_error(error: StoreError) -> Response {
    let (status, code) = match &error {
        StoreError::Conflict(ConflictKind::Stale) => (StatusCode::CONFLICT, "revision_conflict"),
        StoreError::Conflict(ConflictKind::Exists) => (StatusCode::CONFLICT, "already_exists"),
        StoreError::Conflict(ConflictKind::External) => (StatusCode::CONFLICT, "external_change"),
        StoreError::Reserved => (StatusCode::CONFLICT, "resource_reserved"),
        StoreError::Mismatch => (StatusCode::CONFLICT, "operation_mismatch"),
        StoreError::Failed(_) => (StatusCode::CONFLICT, "operation_failed"),
        StoreError::Uncertain => (StatusCode::SERVICE_UNAVAILABLE, "outcome_uncertain"),
        StoreError::Blocked(_) => (StatusCode::CONFLICT, "date_blocked"),
        StoreError::Format(FormatError::BodyTooLarge) => {
            (StatusCode::PAYLOAD_TOO_LARGE, "request_limit")
        }
        StoreError::Format(_) => (StatusCode::UNPROCESSABLE_ENTITY, "invalid_content"),
        StoreError::Coordination => (StatusCode::SERVICE_UNAVAILABLE, "coordination_failed"),
        StoreError::Unavailable => (StatusCode::SERVICE_UNAVAILABLE, "storage_unavailable"),
        StoreError::Storage => (StatusCode::INTERNAL_SERVER_ERROR, "storage_error"),
    };
    // Diagnostics come from fixed format messages (keys and line numbers), never file content.
    let detail = match &error {
        StoreError::Blocked(text) => Some(text.clone()),
        StoreError::Format(e) => Some(e.to_string()),
        StoreError::Failed(tag) => Some(tag.clone()),
        _ => None,
    };
    (
        status,
        Json(json!({"error":{"code":code,"message":code,"retryable":false,"detail":detail}})),
    )
        .into_response()
}

fn date_of(text: &str) -> Result<DiaryDate, Response> {
    DiaryDate::parse(text).ok_or_else(|| failure(StatusCode::BAD_REQUEST, "invalid_input"))
}
fn op_of(text: &str) -> Result<OperationId, Response> {
    OperationId::parse(text).ok_or_else(|| failure(StatusCode::BAD_REQUEST, "invalid_input"))
}
fn bad_body(error: JsonRejection) -> Response {
    failure(
        match error.status() {
            StatusCode::UNSUPPORTED_MEDIA_TYPE => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            StatusCode::PAYLOAD_TOO_LARGE => StatusCode::PAYLOAD_TOO_LARGE,
            _ => StatusCode::BAD_REQUEST,
        },
        "invalid_input",
    )
}
fn revision_json(r: &AuthenticRevision) -> Value {
    json!({"sequence":r.sequence,"sha256":r.sha256})
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ListQuery {
    cursor: Option<String>,
}

pub(super) async fn list(
    State(s): State<Arc<ServerState>>,
    headers: HeaderMap,
    Query(query): Query<ListQuery>,
) -> Response {
    if let Err(r) = authorize(&s, &headers) {
        return r;
    }
    let after = match query.cursor.as_deref().map(date_of).transpose() {
        Ok(after) => after,
        Err(r) => return r,
    };
    let store = match store(&s) {
        Ok(store) => store,
        Err(r) => return r,
    };
    match blocking(store, move |st| {
        st.list(after, diary_application::authentic::PAGE_LIMIT)
    })
    .await
    {
        Ok(page) => Json(json!({
            "items": page.rows.iter().map(|r| json!({
                "date": r.date.to_string(),
                "revision": r.revision.as_ref().map(revision_json),
                "problem": r.problem,
            })).collect::<Vec<_>>(),
            "next_cursor": page.next.map(|d| d.to_string()),
        }))
        .into_response(),
        Err(r) => r,
    }
}

pub(super) async fn load(
    State(s): State<Arc<ServerState>>,
    headers: HeaderMap,
    Path(date): Path<String>,
) -> Response {
    if let Err(r) = authorize(&s, &headers) {
        return r;
    }
    let date = match date_of(&date) {
        Ok(d) => d,
        Err(r) => return r,
    };
    let store = match store(&s) {
        Ok(store) => store,
        Err(r) => return r,
    };
    match blocking(store, move |st| st.load(&date)).await {
        Ok(Loaded::Virtual(date)) => Json(json!({
            "date": date.to_string(), "exists": false, "revision": null, "body": "",
        }))
        .into_response(),
        Ok(Loaded::File {
            revision,
            header,
            body,
        }) => Json(json!({
            "date": header.date.to_string(), "exists": true,
            "revision": revision_json(&revision), "body": body,
            "timezone": header.timezone, "created_at": header.created_at,
            "updated_at": header.updated_at,
        }))
        .into_response(),
        Err(r) => r,
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RevisionIn {
    sequence: u64,
    sha256: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SaveIn {
    operation_id: String,
    expected_revision: Option<RevisionIn>,
    body: String,
    buffer_generation: u64,
}

fn outcome_json(o: &SaveOutcome) -> Value {
    json!({
        "operation_id": o.operation_id.as_str(),
        "revision": revision_json(&o.revision),
        "buffer_generation": o.buffer_generation,
    })
}

fn logged(
    s: &ServerState,
    trace: &TraceId,
    code: EventCode,
    date: &DiaryDate,
    outcome: &Result<SaveOutcome, Response>,
    op: &OperationId,
) {
    let mut record = Record::new(code, Level::Info).trace(trace.clone());
    if let Some(id) = OpaqueId::parse(op.as_str()) {
        record = record.operation(id);
    }
    if let (Ok(o), Some(id)) = (outcome, OpaqueId::parse(&date.to_string())) {
        record = record.resource(ResourceDetail {
            kind: ResourceKind::AuthenticEntry,
            id,
            revision: Some(o.revision.sequence),
            bytes: None,
            lines: None,
            fingerprint: None,
            state: Some(StateTag::Saved),
            path: None,
        });
    }
    s.log(record);
}

fn log_failure(s: &ServerState, trace: &TraceId, op: &OperationId, error: &Response) {
    use diary_application::diagnostics::ErrorTag;
    let (code, tag) = match error.status() {
        StatusCode::CONFLICT => (EventCode::AuthenticConflict, ErrorTag::StaleRevision),
        StatusCode::PAYLOAD_TOO_LARGE => (EventCode::AuthenticBlocked, ErrorTag::Limit),
        StatusCode::UNPROCESSABLE_ENTITY => (EventCode::AuthenticBlocked, ErrorTag::Invalid),
        StatusCode::INTERNAL_SERVER_ERROR => (EventCode::AuthenticBlocked, ErrorTag::Internal),
        _ => (EventCode::AuthenticBlocked, ErrorTag::Unavailable),
    };
    let mut record = Record::new(code, Level::Warn)
        .trace(trace.clone())
        .error(tag);
    if let Some(id) = OpaqueId::parse(op.as_str()) {
        record = record.operation(id);
    }
    s.log(record);
}

pub(super) async fn save(
    State(s): State<Arc<ServerState>>,
    Extension(trace): Extension<TraceId>,
    headers: HeaderMap,
    Path(date): Path<String>,
    payload: Result<Json<SaveIn>, JsonRejection>,
) -> Response {
    if let Err(r) = authorize(&s, &headers) {
        return r;
    }
    let input = match payload {
        Ok(Json(input)) => input,
        Err(e) => return bad_body(e),
    };
    let (date, op) = match (date_of(&date), op_of(&input.operation_id)) {
        (Ok(d), Ok(o)) => (d, o),
        (Err(r), _) | (_, Err(r)) => return r,
    };
    let expected = match input.expected_revision {
        None => None,
        Some(r)
            if r.sha256.len() == 64
                && r.sha256
                    .bytes()
                    .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) =>
        {
            Some(AuthenticRevision {
                sequence: r.sequence,
                sha256: r.sha256,
            })
        }
        Some(_) => return failure(StatusCode::BAD_REQUEST, "invalid_input"),
    };
    let store = match store(&s) {
        Ok(store) => store,
        Err(r) => return r,
    };
    let request = SaveRequest {
        operation_id: op.clone(),
        date,
        expected,
        body: input.body,
        buffer_generation: input.buffer_generation,
    };
    let outcome = blocking(store, move |st| st.save(request)).await;
    match &outcome {
        Ok(_) => logged(&s, &trace, EventCode::AuthenticSaved, &date, &outcome, &op),
        Err(r) => log_failure(&s, &trace, &op, r),
    }
    outcome.map_or_else(|r| r, |o| Json(outcome_json(&o)).into_response())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CreateIn {
    operation_id: String,
}

pub(super) async fn create_empty(
    State(s): State<Arc<ServerState>>,
    Extension(trace): Extension<TraceId>,
    headers: HeaderMap,
    Path(date): Path<String>,
    payload: Result<Json<CreateIn>, JsonRejection>,
) -> Response {
    if let Err(r) = authorize(&s, &headers) {
        return r;
    }
    let input = match payload {
        Ok(Json(input)) => input,
        Err(e) => return bad_body(e),
    };
    let (date, op) = match (date_of(&date), op_of(&input.operation_id)) {
        (Ok(d), Ok(o)) => (d, o),
        (Err(r), _) | (_, Err(r)) => return r,
    };
    let store = match store(&s) {
        Ok(store) => store,
        Err(r) => return r,
    };
    let request = CreateRequest {
        operation_id: op.clone(),
        date,
    };
    let outcome = blocking(store, move |st| st.create_empty(request)).await;
    match &outcome {
        Ok(_) => logged(
            &s,
            &trace,
            EventCode::AuthenticCreated,
            &date,
            &outcome,
            &op,
        ),
        Err(r) => log_failure(&s, &trace, &op, r),
    }
    outcome.map_or_else(|r| r, |o| Json(outcome_json(&o)).into_response())
}

pub(super) async fn refresh(
    State(s): State<Arc<ServerState>>,
    Extension(trace): Extension<TraceId>,
    headers: HeaderMap,
) -> Response {
    if let Err(r) = authorize(&s, &headers) {
        return r;
    }
    let store = match store(&s) {
        Ok(store) => store,
        Err(r) => return r,
    };
    match blocking(store, |st| st.refresh()).await {
        Ok(report) => {
            s.log(
                traced(EventCode::AuthenticRebuilt, Level::Info, &trace)
                    .count(u32::try_from(report.files).unwrap_or(u32::MAX)),
            );
            Json(json!({"files":report.files,"problems":report.problems})).into_response()
        }
        Err(r) => r,
    }
}

pub(super) async fn operation(
    State(s): State<Arc<ServerState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    if let Err(r) = authorize(&s, &headers) {
        return r;
    }
    let id = match op_of(&id) {
        Ok(id) => id,
        Err(r) => return r,
    };
    if let Some(response) = super::source_trash::operation(&s, &id).await {
        return response;
    }
    if let Some(response) = super::exports::operation(&s, &id).await {
        return response;
    }
    let store = match store(&s) {
        Ok(store) => store,
        Err(r) => return r,
    };
    match blocking(store, move |st| st.operation(&id)).await {
        Ok(Some(record)) => Json(json!({
            "operation_id": record.id.as_str(),
            "kind": record.kind,
            "state": record.state.as_str(),
            "terminal": record.state.is_terminal(),
            "succeeded": record.state == OperationState::Succeeded,
            "checkpoint": record.checkpoint,
            "result": record.result,
        }))
        .into_response(),
        Ok(None) => failure(StatusCode::NOT_FOUND, "not_found"),
        Err(r) => r,
    }
}

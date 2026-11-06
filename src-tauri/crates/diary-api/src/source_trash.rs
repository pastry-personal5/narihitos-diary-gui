//! Fixed, authenticated source Trash operations. DTOs expose relative labels and
//! allowlisted codes; no filesystem/native receipt or bearer token enters a result.
#![allow(clippy::result_large_err)]
use super::{ServerState, app_error, failure, session};
use axum::{
    Json,
    extract::{Path, Query, State, rejection::JsonRejection},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use diary_application::{
    operations::{OperationId, StepId},
    source_trash::*,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::{Arc, atomic::Ordering};

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Action {
    Initial,
    Continuation,
}
/// Only fixed discovery filters are permitted through the destination boundary.
pub(super) fn valid_query(query: &str) -> bool {
    if query.len() > 256 {
        return false;
    }
    let mut seen = std::collections::BTreeSet::new();
    query.split('&').all(|part| {
        let Some((key, value)) = part.split_once('=') else {
            return false;
        };
        if !seen.insert(key) || value.is_empty() {
            return false;
        }
        match key {
            "kind" | "state" => {
                value.len() <= 32
                    && value
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'_')
            }
            "limit" => value.len() <= 3 && value.bytes().all(|b| b.is_ascii_digit()),
            "cursor" => {
                value.len() <= 128
                    && value
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
            }
            _ => false,
        }
    })
}
impl From<Action> for DeleteAction {
    fn from(a: Action) -> Self {
        match a {
            Action::Initial => Self::Initial,
            Action::Continuation => Self::Continuation,
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PreviewIn {
    kind: Action,
    operation_id: Option<String>,
    expected_checkpoint: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ConfirmIn {
    kind: Action,
    operation_id: String,
    step_id: String,
    expected_checkpoint: u64,
    plan_hash: String,
    ordinals: Vec<u8>,
    token: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ListIn {
    kind: String,
    state: String,
    cursor: Option<String>,
    limit: Option<usize>,
}

pub(super) fn error(e: DeleteError) -> Response {
    use DeleteError::*;
    let (status, code) = match e {
        Invalid => (StatusCode::BAD_REQUEST, "invalid_input"),
        Unsupported => (StatusCode::UNPROCESSABLE_ENTITY, "trash_unsupported"),
        SourceStale => (StatusCode::CONFLICT, "source_stale"),
        Changed => (StatusCode::CONFLICT, "member_changed"),
        OwnershipAmbiguous => (StatusCode::CONFLICT, "ownership_ambiguous"),
        CloudUnavailable => (StatusCode::SERVICE_UNAVAILABLE, "cloud_unavailable"),
        Busy => (StatusCode::CONFLICT, "resource_reserved"),
        Limit => (StatusCode::TOO_MANY_REQUESTS, "limit"),
        Expired => (StatusCode::CONFLICT, "preview_expired"),
        PreviewStale => (StatusCode::CONFLICT, "preview_stale"),
        Mismatch => (StatusCode::CONFLICT, "operation_mismatch"),
        CheckpointMismatch => (StatusCode::CONFLICT, "checkpoint_mismatch"),
        Uncertain => (StatusCode::CONFLICT, "outcome_uncertain"),
        NotFound => (StatusCode::NOT_FOUND, "not_found"),
        Storage => (StatusCode::INTERNAL_SERVER_ERROR, "storage_error"),
    };
    failure(status, code)
}
fn authorize(s: &ServerState, headers: &HeaderMap) -> Result<String, Response> {
    let id = session(headers).map_err(app_error)?;
    s.coordinator
        .lock()
        .expect("coordinator poisoned")
        .list(id, s.now(), None, 1)
        .map_err(app_error)?;
    Ok(id.into())
}
fn source_id(id: &str) -> bool {
    id.len() == 32
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn payload<T>(value: Result<Json<T>, JsonRejection>) -> Result<T, Response> {
    value.map(|Json(v)| v).map_err(|e| {
        failure(
            if e.status() == StatusCode::PAYLOAD_TOO_LARGE {
                StatusCode::PAYLOAD_TOO_LARGE
            } else {
                StatusCode::BAD_REQUEST
            },
            "invalid_input",
        )
    })
}
fn bounded(value: Value, max: usize) -> Response {
    match serde_json::to_vec(&value) {
        Ok(bytes) if bytes.len() <= max => (
            [(axum::http::header::CONTENT_TYPE, "application/json")],
            bytes,
        )
            .into_response(),
        _ => error(DeleteError::Storage),
    }
}
fn member(m: &DeleteMember) -> Value {
    json!({"ordinal":m.ordinal,"relative_label":m.relative_label,"state":m.state.as_str(),"reason":m.reason,"retry_eligible":m.retry_eligible})
}
pub(super) fn result(r: &DeleteResult) -> Value {
    let reason = if r.state == DeleteState::NeedsConfirmation
        && r.members
            .iter()
            .any(|m| m.state == DeleteMemberState::Moved)
    {
        Some("partial_trash")
    } else if r.state == DeleteState::Uncertain {
        Some("outcome_uncertain")
    } else {
        None
    };
    json!({"kind":"source_trash","operation_id":r.operation_id.as_str(),"source_id":r.source_id,"plan_hash":r.plan_hash,"checkpoint":r.checkpoint,"state":r.state.as_str(),"reason":reason,"members":r.members.iter().map(member).collect::<Vec<_>>()})
}

pub(super) async fn preview(
    State(s): State<Arc<ServerState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    input: Result<Json<PreviewIn>, JsonRejection>,
) -> Response {
    let session = match authorize(&s, &headers) {
        Ok(s) => s,
        Err(e) => return e,
    };
    let input = match payload(input) {
        Ok(i) => i,
        Err(e) => return e,
    };
    if !source_id(&id) || input.expected_checkpoint > i64::MAX as u64 - 1 {
        return error(DeleteError::Invalid);
    }
    let operation_id = match input.operation_id {
        Some(id) => match OperationId::parse(&id) {
            Some(id) => Some(id),
            None => return error(DeleteError::Invalid),
        },
        None => None,
    };
    if matches!(input.kind, Action::Initial)
        && (operation_id.is_some() || input.expected_checkpoint != 0)
        || matches!(input.kind, Action::Continuation)
            && (operation_id.is_none() || input.expected_checkpoint == 0)
    {
        return error(DeleteError::Invalid);
    }
    let Some(service) = s.source_trash.clone() else {
        return error(DeleteError::Unsupported);
    };
    let permit = match s.source_slots.clone().try_acquire_owned() {
        Ok(p) => p,
        Err(_) => return error(DeleteError::Limit),
    };
    let generation = s.config_generation.load(Ordering::SeqCst);
    match tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let check = super::imports::throttled(|| {
            super::imports::cancelled(&s, &session)
                || s.config_generation.load(Ordering::SeqCst) != generation
        });
        let preview = service.preview(
            &session,
            generation,
            DeletePreviewRequest {
                action: input.kind.into(),
                source_id: id,
                operation_id,
                expected_checkpoint: input.expected_checkpoint,
            },
            &check,
        )?;
        if super::imports::cancelled(&s, &session)
            || s.config_generation.load(Ordering::SeqCst) != generation
        {
            return Err(DeleteError::PreviewStale);
        }
        Ok(preview)
    })
    .await
    {
        Ok(Ok(p)) => bounded(
            json!({"token":p.token,"plan_hash":p.plan_hash,"expected_checkpoint":p.expected_checkpoint,"expires_in_ms":p.expires_in_ms,"ordinals":p.ordinals,"members":p.members.iter().map(member).collect::<Vec<_>>()}),
            4096,
        ),
        Ok(Err(e)) => error(e),
        Err(_) => error(DeleteError::Storage),
    }
}
pub(super) async fn confirm(
    State(s): State<Arc<ServerState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    input: Result<Json<ConfirmIn>, JsonRejection>,
) -> Response {
    let session = match authorize(&s, &headers) {
        Ok(s) => s,
        Err(e) => return e,
    };
    let input = match payload(input) {
        Ok(i) => i,
        Err(e) => return e,
    };
    let (Some(operation_id), Some(step_id)) = (
        OperationId::parse(&input.operation_id),
        StepId::parse(&input.step_id),
    ) else {
        return error(DeleteError::Invalid);
    };
    if !source_id(&id)
        || input.token.len() > 128
        || input.plan_hash.len() != 64
        || !input
            .plan_hash
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        || input.ordinals.is_empty()
        || input.ordinals.len() > 2
        || input.ordinals.iter().any(|o| *o > 1)
        || !input.ordinals.windows(2).all(|w| w[0] < w[1])
        || input.expected_checkpoint > i64::MAX as u64 - 1
        || matches!(input.kind, Action::Initial)
            && (input.expected_checkpoint != 0 || input.ordinals != [0, 1])
        || matches!(input.kind, Action::Continuation) && input.expected_checkpoint == 0
    {
        return error(DeleteError::Invalid);
    }
    let Some(service) = s.source_trash.clone() else {
        return error(DeleteError::Unsupported);
    };
    let permit = match s.source_slots.clone().try_acquire_owned() {
        Ok(p) => p,
        Err(_) => return error(DeleteError::Limit),
    };
    let generation = s.config_generation.load(Ordering::SeqCst);
    match tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let check = super::imports::throttled(|| {
            super::imports::cancelled(&s, &session)
                || s.config_generation.load(Ordering::SeqCst) != generation
        });
        service.confirm(
            &session,
            generation,
            DeleteConfirmRequest {
                action: input.kind.into(),
                source_id: id,
                operation_id,
                step_id,
                expected_checkpoint: input.expected_checkpoint,
                plan_hash: input.plan_hash,
                ordinals: input.ordinals,
                token: input.token,
            },
            &check,
        )
    })
    .await
    {
        Ok(Ok(a)) => bounded(
            json!({"operation_id":a.operation_id.as_str(),"step_id":a.step_id.as_str(),"admitted_checkpoint":a.admitted_checkpoint}),
            4096,
        ),
        Ok(Err(e)) => error(e),
        Err(_) => error(DeleteError::Storage),
    }
}
pub(super) async fn unfinished(
    State(s): State<Arc<ServerState>>,
    headers: HeaderMap,
    query: Result<Query<ListIn>, axum::extract::rejection::QueryRejection>,
) -> Response {
    if let Err(e) = authorize(&s, &headers) {
        return e;
    }
    let input = match query {
        Ok(Query(q)) => q,
        Err(_) => return error(DeleteError::Invalid),
    };
    let limit = input.limit.unwrap_or(50);
    if input.kind != "source_trash"
        || input.state != "unfinished"
        || limit == 0
        || limit > 100
        || input.cursor.as_ref().is_some_and(|c| c.len() > 128)
    {
        return error(DeleteError::Invalid);
    }
    let Some(service) = s.source_trash.clone() else {
        return error(DeleteError::Unsupported);
    };
    match tokio::task::spawn_blocking(move || service.unfinished(input.cursor.as_deref(), limit))
        .await
    {
        Ok(Ok(p)) => bounded(
            json!({"operations":p.operations.iter().map(result).collect::<Vec<_>>(),"next_cursor":p.next_cursor}),
            512 * 1024,
        ),
        Ok(Err(e)) => error(e),
        Err(_) => error(DeleteError::Storage),
    }
}

pub(super) async fn operation(s: &Arc<ServerState>, id: &OperationId) -> Option<Response> {
    let service = s.source_trash.clone()?;
    let id = id.clone();
    match tokio::task::spawn_blocking(move || service.operation(&id)).await {
        Ok(Ok(Some(r))) => Some(bounded(
            json!({"operation_id":r.operation_id.as_str(),"kind":"source_trash","state":if r.state==DeleteState::NeedsConfirmation {"paused"} else {r.state.as_str()},"terminal":r.state==DeleteState::Succeeded,"succeeded":r.state==DeleteState::Succeeded,"checkpoint":r.checkpoint,"result":if r.state==DeleteState::Succeeded {Some("source_trash")} else {None},"trash":result(&r)}),
            4096,
        )),
        Ok(Ok(None)) => None,
        Ok(Err(e)) => Some(error(e)),
        Err(_) => Some(error(DeleteError::Storage)),
    }
}

#[cfg(test)]
mod tests;

//! Destination-neutral export routes. DTOs expose IDs, digests and allowlisted codes; the
//! snapshot body, bearer token (outside the one preview response) and remote content never
//! appear. State changes also reach the client as `job_changed` events, after the durable write.
#![allow(clippy::result_large_err)]
use super::{ServerState, app_error, failure, session};
use axum::{
    Json,
    extract::{State, rejection::JsonRejection},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use diary_application::{
    Error, JobState,
    export::*,
    operations::{OperationId, StepId},
};
use diary_domain::authentic::{AuthenticRevision, DiaryDate};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::{Arc, atomic::Ordering};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Revision {
    sequence: u64,
    sha256: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StartIn {
    operation_id: String,
    date: String,
    target: String,
    expected_revision: Revision,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ConfirmIn {
    step_id: String,
    expected_checkpoint: u64,
    token: String,
}

pub(super) fn error(e: ExportError) -> Response {
    use ExportError::*;
    if let CloudUnavailable(reason) = e {
        let (status, code) = match reason {
            Unavailable::PermissionDenied => (StatusCode::FORBIDDEN, "permission_denied"),
            Unavailable::Unauthorized => (StatusCode::UNAUTHORIZED, "export_unauthorized"),
            Unavailable::Throttled => (StatusCode::TOO_MANY_REQUESTS, "export_throttled"),
            Unavailable::Network => (StatusCode::SERVICE_UNAVAILABLE, "export_network"),
            Unavailable::Structural => (StatusCode::CONFLICT, "export_structural"),
            _ => (StatusCode::SERVICE_UNAVAILABLE, "cloud_unavailable"),
        };
        return (
            status,
            Json(json!({"error":{"code":code,"message":code,"retryable":false,"reason":reason.as_str()}})),
        )
            .into_response();
    }
    let (status, code) = match e {
        Invalid => (StatusCode::BAD_REQUEST, "invalid_input"),
        Disabled => (StatusCode::UNPROCESSABLE_ENTITY, "export_disabled"),
        NothingToExport => (StatusCode::NOT_FOUND, "nothing_to_export"),
        StaleRevision => (StatusCode::CONFLICT, "stale_revision"),
        Busy => (StatusCode::CONFLICT, "resource_reserved"),
        Limit => (StatusCode::TOO_MANY_REQUESTS, "limit"),
        Expired => (StatusCode::CONFLICT, "preview_expired"),
        PreviewStale => (StatusCode::CONFLICT, "preview_stale"),
        Mismatch => (StatusCode::CONFLICT, "operation_mismatch"),
        CheckpointMismatch => (StatusCode::CONFLICT, "checkpoint_mismatch"),
        Uncertain => (StatusCode::CONFLICT, "outcome_uncertain"),
        CloudUnavailable(_) => unreachable!("answered above"),
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
        _ => error(ExportError::Storage),
    }
}
fn digest(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
pub(super) fn result(r: &ExportResult) -> Value {
    json!({"kind":"export","operation_id":r.operation_id.as_str(),"target":r.target.as_str(),"date":r.date,"revision":{"sequence":r.revision_sequence,"sha256":r.revision_sha},"state":r.state.as_str(),"checkpoint":r.checkpoint,"reason":r.reason})
}

/// Mirrors a durable export state into the session's job list and publishes one `job_changed`
/// per distinct `(checkpoint, state)`. Best effort: `get_operation` stays authoritative.
pub(super) fn sync(s: &ServerState, session: &str, r: &ExportResult) {
    let id = r.operation_id.as_str();
    {
        let mark = (session.to_owned(), r.checkpoint, r.state.as_str());
        let mut jobs = s.export_jobs.lock().expect("export jobs poisoned");
        if jobs.get(id) == Some(&mark) {
            return;
        }
        jobs.insert(id.to_owned(), mark);
    }
    let mut c = s.coordinator.lock().expect("coordinator poisoned");
    let now = s.now();
    let current = match c.get(session, now, id) {
        Ok(job) => job,
        Err(Error::NotFound) => match c.admit(session, now, id, &r.input_hash, id, 4096) {
            Ok(job) => job,
            Err(_) => return,
        },
        Err(_) => return,
    };
    let target = match r.state {
        ExportState::Succeeded => JobState::Succeeded,
        ExportState::Cancelled => JobState::Cancelled,
        ExportState::Failed | ExportState::Uncertain => JobState::Failed,
        _ => JobState::Running,
    };
    let job = if current.state == JobState::Queued && target != JobState::Running {
        c.transition(session, now, id, JobState::Running)
            .and_then(|_| c.transition(session, now, id, target))
    } else if target == JobState::Running && current.state == JobState::Running {
        c.touch(session, now, id)
    } else {
        c.transition(session, now, id, target)
    };
    if let Ok(job) = job {
        super::imports::publish(s, &mut c, session, &job);
    }
}
pub(super) fn is_export_job(s: &ServerState, id: &str) -> bool {
    s.export_jobs
        .lock()
        .expect("export jobs poisoned")
        .contains_key(id)
}

fn started(started: &Started) -> Response {
    match started {
        Started::Result(r) => {
            let mut v = result(r);
            v["outcome"] = json!(r.state.as_str());
            bounded(v, 4096)
        }
        Started::Preview(p) => bounded(
            json!({"outcome":"needs_confirmation","operation_id":p.operation_id.as_str(),"expected_checkpoint":p.expected_checkpoint,"token":p.token,"observed_fingerprint":p.observed_fingerprint,"reason":p.reason.as_str(),"write":p.write.as_str(),"expires_in_ms":p.expires_in_ms}),
            4096,
        ),
    }
}

pub(super) async fn start(
    State(s): State<Arc<ServerState>>,
    headers: HeaderMap,
    input: Result<Json<StartIn>, JsonRejection>,
) -> Response {
    let session = match authorize(&s, &headers) {
        Ok(s) => s,
        Err(e) => return e,
    };
    let input = match payload(input) {
        Ok(i) => i,
        Err(e) => return e,
    };
    let (Some(operation_id), Some(date), Some(target)) = (
        OperationId::parse(&input.operation_id),
        DiaryDate::parse(&input.date),
        Target::parse(&input.target),
    ) else {
        return error(ExportError::Invalid);
    };
    if !digest(&input.expected_revision.sha256)
        || input.expected_revision.sequence > i64::MAX as u64
    {
        return error(ExportError::Invalid);
    }
    let Some(service) = s.exports.clone() else {
        return error(ExportError::Disabled);
    };
    let permit = match s.source_slots.clone().try_acquire_owned() {
        Ok(p) => p,
        Err(_) => return error(ExportError::Limit),
    };
    let generation = s.config_generation.load(Ordering::SeqCst);
    let request = StartRequest {
        operation_id: operation_id.clone(),
        date,
        target,
        expected: AuthenticRevision {
            sequence: input.expected_revision.sequence,
            sha256: input.expected_revision.sha256,
        },
    };
    let who = session.clone();
    match tokio::task::spawn_blocking(move || {
        let _permit = permit;
        service
            .start(&who, generation, request)
            .map(|st| (st, service.operation(&operation_id)))
    })
    .await
    {
        Ok(Ok((st, latest))) => {
            if let Ok(Some(r)) = latest {
                sync(&s, &session, &r);
            }
            started(&st)
        }
        Ok(Err(e)) => error(e),
        Err(_) => error(ExportError::Storage),
    }
}

pub(super) async fn confirm(
    State(s): State<Arc<ServerState>>,
    headers: HeaderMap,
    axum::extract::Path(id): axum::extract::Path<String>,
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
    let (Some(operation_id), Some(step_id)) =
        (OperationId::parse(&id), StepId::parse(&input.step_id))
    else {
        return error(ExportError::Invalid);
    };
    if input.token.is_empty()
        || input.token.len() > 128
        || input.expected_checkpoint == 0
        || input.expected_checkpoint > i64::MAX as u64 - 1
    {
        return error(ExportError::Invalid);
    }
    let Some(service) = s.exports.clone() else {
        return error(ExportError::Disabled);
    };
    let permit = match s.source_slots.clone().try_acquire_owned() {
        Ok(p) => p,
        Err(_) => return error(ExportError::Limit),
    };
    let generation = s.config_generation.load(Ordering::SeqCst);
    let who = session.clone();
    let op = operation_id.clone();
    match tokio::task::spawn_blocking(move || {
        let _permit = permit;
        service
            .confirm(
                &who,
                generation,
                ConfirmRequest {
                    operation_id,
                    step_id,
                    expected_checkpoint: input.expected_checkpoint,
                    token: input.token,
                },
            )
            .map(|a| (a, service.operation(&op)))
    })
    .await
    {
        Ok(Ok((a, latest))) => {
            // Only an admitted (or recorded) step reaches here, so a rejected or duplicate
            // confirmation publishes nothing new: `sync` ignores an unchanged state.
            if let Ok(Some(r)) = latest {
                sync(&s, &session, &r);
            }
            bounded(
                json!({"operation_id":a.operation_id.as_str(),"step_id":a.step_id.as_str(),"admitted_checkpoint":a.admitted_checkpoint}),
                4096,
            )
        }
        Ok(Err(e)) => error(e),
        Err(_) => error(ExportError::Storage),
    }
}

/// `POST /jobs/{id}/cancel` for an export: declines a pending confirmation or stops a running
/// export before `apply`. Remote state is never rolled back.
pub(super) async fn cancel(s: &Arc<ServerState>, session: &str, job: &str) -> Response {
    let (Some(service), Some(id)) = (s.exports.clone(), OperationId::parse(job)) else {
        return error(ExportError::Invalid);
    };
    match tokio::task::spawn_blocking(move || service.cancel(&id)).await {
        Ok(Ok(r)) => {
            sync(s, session, &r);
            let mut c = s.coordinator.lock().expect("coordinator poisoned");
            match c.get(session, s.now(), job) {
                Ok(j) => Json(super::job_value(s, session, j)).into_response(),
                Err(e) => app_error(e),
            }
        }
        Ok(Err(e)) => error(e),
        Err(_) => error(ExportError::Storage),
    }
}

/// Resolves an `uncertain` export: a matching read-back succeeds, otherwise a one-use preview.
pub(super) async fn abandon(
    State(s): State<Arc<ServerState>>,
    headers: HeaderMap,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Response {
    let session = match authorize(&s, &headers) {
        Ok(s) => s,
        Err(e) => return e,
    };
    let (Some(service), Some(operation_id)) = (s.exports.clone(), OperationId::parse(&id)) else {
        return error(ExportError::Invalid);
    };
    let permit = match s.source_slots.clone().try_acquire_owned() {
        Ok(p) => p,
        Err(_) => return error(ExportError::Limit),
    };
    let generation = s.config_generation.load(Ordering::SeqCst);
    let who = session.clone();
    match tokio::task::spawn_blocking(move || {
        let _permit = permit;
        service
            .abandon(&who, generation, &operation_id)
            .map(|st| (st, service.operation(&operation_id)))
    })
    .await
    {
        Ok(Ok((st, latest))) => {
            if let Ok(Some(r)) = latest {
                sync(&s, &session, &r);
            }
            started(&st)
        }
        Ok(Err(e)) => error(e),
        Err(_) => error(ExportError::Storage),
    }
}

pub(super) async fn abandon_confirm(
    State(s): State<Arc<ServerState>>,
    headers: HeaderMap,
    axum::extract::Path(id): axum::extract::Path<String>,
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
    let (Some(operation_id), Some(step_id)) =
        (OperationId::parse(&id), StepId::parse(&input.step_id))
    else {
        return error(ExportError::Invalid);
    };
    if input.token.is_empty() || input.token.len() > 128 || input.expected_checkpoint == 0 {
        return error(ExportError::Invalid);
    }
    let Some(service) = s.exports.clone() else {
        return error(ExportError::Disabled);
    };
    let permit = match s.source_slots.clone().try_acquire_owned() {
        Ok(p) => p,
        Err(_) => return error(ExportError::Limit),
    };
    let generation = s.config_generation.load(Ordering::SeqCst);
    let who = session.clone();
    match tokio::task::spawn_blocking(move || {
        let _permit = permit;
        service.abandon_confirm(
            &who,
            generation,
            ConfirmRequest {
                operation_id,
                step_id,
                expected_checkpoint: input.expected_checkpoint,
                token: input.token,
            },
        )
    })
    .await
    {
        Ok(Ok(r)) => {
            sync(&s, &session, &r);
            bounded(result(&r), 4096)
        }
        Ok(Err(e)) => error(e),
        Err(_) => error(ExportError::Storage),
    }
}

pub(super) async fn unfinished(
    s: &Arc<ServerState>,
    cursor: Option<String>,
    limit: usize,
) -> Response {
    let Some(service) = s.exports.clone() else {
        return error(ExportError::Disabled);
    };
    match tokio::task::spawn_blocking(move || service.unfinished(cursor.as_deref(), limit)).await {
        Ok(Ok(p)) => bounded(
            json!({"operations":p.operations.iter().map(result).collect::<Vec<_>>(),"next_cursor":p.next_cursor}),
            512 * 1024,
        ),
        Ok(Err(e)) => error(e),
        Err(_) => error(ExportError::Storage),
    }
}

pub(super) async fn operation(s: &Arc<ServerState>, id: &OperationId) -> Option<Response> {
    let service = s.exports.clone()?;
    let id = id.clone();
    match tokio::task::spawn_blocking(move || service.operation(&id)).await {
        Ok(Ok(Some(r))) => {
            let done = r.state == ExportState::Succeeded;
            Some(bounded(
                json!({"operation_id":r.operation_id.as_str(),"kind":"export","state":if r.state==ExportState::NeedsConfirmation {"paused"} else {r.state.as_str()},"terminal":r.state.is_terminal(),"succeeded":done,"checkpoint":r.checkpoint,"result":if done {Some("export")} else {None},"export":result(&r)}),
                4096,
            ))
        }
        Ok(Ok(None)) => None,
        Ok(Err(e)) => Some(error(e)),
        Err(_) => Some(error(ExportError::Storage)),
    }
}

#[cfg(test)]
mod tests;

//! `PUT /sources/{id}/body`: the guarded body save of a producer source. Needs an admitted
//! session; the writer runs off the async executor; errors are allowlisted codes only.
#![allow(clippy::result_large_err)]
use super::{ServerState, app_error, failure, session};
use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use diary_application::{
    operations::OperationId,
    source::{SaveError, SourceSaveRequest},
};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SaveIn {
    operation_id: String,
    base_digest: String,
    body: String,
}

fn code(e: SaveError) -> (StatusCode, &'static str, bool) {
    use SaveError::*;
    match e {
        ExternalChange => (StatusCode::CONFLICT, "external_change", false),
        UnsupportedFormat => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "unsupported_format",
            false,
        ),
        InvalidBody => (StatusCode::UNPROCESSABLE_ENTITY, "invalid_body", false),
        CloudUnavailable => (StatusCode::SERVICE_UNAVAILABLE, "cloud_unavailable", true),
        SourceChanging => (StatusCode::CONFLICT, "source_changing", true),
        SourceStale => (StatusCode::CONFLICT, "source_stale", false),
        Reserved => (StatusCode::CONFLICT, "resource_reserved", false),
        Mismatch => (StatusCode::CONFLICT, "operation_mismatch", false),
        Uncertain => (StatusCode::SERVICE_UNAVAILABLE, "outcome_uncertain", true),
        NotFound => (StatusCode::NOT_FOUND, "not_found", false),
        Storage => (StatusCode::INTERNAL_SERVER_ERROR, "storage_error", false),
    }
}

fn opaque(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 128
        && text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

pub(super) async fn save(
    State(s): State<Arc<ServerState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    payload: Result<Json<SaveIn>, JsonRejection>,
) -> Response {
    let sid = match session(&headers) {
        Ok(sid) => sid,
        Err(e) => return app_error(e),
    };
    if let Err(e) = s
        .coordinator
        .lock()
        .expect("coordinator poisoned")
        .list(sid, s.now(), None, 1)
    {
        return app_error(e);
    }
    let input = match payload {
        Ok(Json(input)) => input,
        Err(e) => {
            return failure(
                if e.status() == StatusCode::PAYLOAD_TOO_LARGE {
                    StatusCode::PAYLOAD_TOO_LARGE
                } else {
                    StatusCode::BAD_REQUEST
                },
                "invalid_input",
            );
        }
    };
    let digest_ok = input.base_digest.len() == 64
        && input
            .base_digest
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
    let (Some(operation_id), true, true) = (
        OperationId::parse(&input.operation_id),
        opaque(&id),
        digest_ok,
    ) else {
        return failure(StatusCode::BAD_REQUEST, "invalid_input");
    };
    let Some(writer) = s.source_writer.clone() else {
        return failure(StatusCode::SERVICE_UNAVAILABLE, "storage_unavailable");
    };
    let request = SourceSaveRequest {
        operation_id,
        source_id: id,
        base_digest: input.base_digest,
        body: input.body,
    };
    match tokio::task::spawn_blocking(move || writer.save_body(request)).await {
        Ok(Ok(saved)) => Json(json!({
            "operation_id": saved.operation_id.as_str(),
            "digest": saved.digest,
            "size": saved.size,
            "unchanged": saved.unchanged,
        }))
        .into_response(),
        Ok(Err(e)) => {
            let (status, code, retryable) = code(e);
            (
                status,
                Json(json!({"error":{"code":code,"message":code,"retryable":retryable}})),
            )
                .into_response()
        }
        Err(_) => failure(StatusCode::INTERNAL_SERVER_ERROR, "internal"),
    }
}

#[cfg(test)]
mod tests;

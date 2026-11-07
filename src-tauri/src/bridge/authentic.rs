//! Authentic commands. Inputs are validated before any request, the route is fixed here, and
//! only allowlisted fields and error codes reach the WebView (never body text, paths or raw
//! server messages).
use super::{Bridge, valid_id};
use diary_application::diagnostics::ErrorTag;
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::json;

const BODY_LIMIT: usize = 1024 * 1024;
const ERROR_LIMIT: usize = 4096;
const RESPONSE_LIMIT: usize = 10 * 1024 * 1024;

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Revision {
    pub sequence: u64,
    pub sha256: String,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct ListItem {
    pub date: String,
    pub revision: Option<Revision>,
    pub problem: Option<String>,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct AuthenticList {
    pub items: Vec<ListItem>,
    pub next_cursor: Option<String>,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct AuthenticFile {
    pub date: String,
    pub exists: bool,
    pub revision: Option<Revision>,
    pub body: String,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct SaveAck {
    pub operation_id: String,
    pub revision: Revision,
    pub buffer_generation: u64,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct RefreshReport {
    pub files: u64,
    pub problems: u64,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct OperationState {
    pub operation_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkpoint: Option<u64>,
    pub state: String,
    pub terminal: bool,
    pub succeeded: bool,
    pub result: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trash: Option<super::source_trash::TrashResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub export: Option<super::export::ExportResult>,
}
/// `current_revision` is part of the contract but P1-M6 conflicts do not carry it, so clients
/// reload after a conflict.
#[derive(Serialize, Debug, PartialEq)]
pub struct AuthenticError {
    pub code: &'static str,
    pub retryable: bool,
}

const SERVER_CODES: &[&str] = &[
    "revision_conflict",
    "already_exists",
    "external_change",
    "resource_reserved",
    "operation_mismatch",
    "operation_failed",
    "outcome_uncertain",
    "date_blocked",
    "request_limit",
    "invalid_content",
    "coordination_failed",
    "storage_unavailable",
    "storage_error",
    "not_found",
    "invalid_input",
    "session_expired",
    "client_busy",
    "limit",
    "unnamed_timezone",
    "timezone_database_unavailable",
    "invalid_diary_date",
    "source_stale",
    "unsupported_format",
    "invalid_body",
    "cloud_unavailable",
    "reimport_required",
    "missing",
    "denied",
    "not_downloaded",
    "not_regular",
    "too_large",
    "source_changing",
    "invalid_path",
    "malformed",
    "invalid_time_authority",
    "fix_filename_directory_externally",
    "cancelled",
    "request_mismatch",
    "trash_unsupported",
    "preview_expired",
    "preview_stale",
    "member_changed",
    "ownership_ambiguous",
    "checkpoint_mismatch",
    "partial_trash",
    "busy",
    "profile_not_ready",
    "context",
    "empty_selection",
    "discard_required",
    "result_unavailable",
    "review_invalidated",
    "stale_baseline",
    "set_mismatch",
    "decided",
    "review_unsupported",
    "review_limit",
    "review_active",
    "export_disabled",
    "export_unauthorized",
    "export_throttled",
    "export_network",
    "export_structural",
    "nothing_to_export",
    "stale_revision",
    "unsupported",
];
const LOCAL_CODES: &[&str] = &[
    "permission_denied",
    "disconnected",
    "server_unavailable",
    "invalid_response",
    "invalid_response_type",
    "response_limit",
];

impl AuthenticError {
    pub(super) fn new(code: &'static str) -> Self {
        let retryable = matches!(
            code,
            "outcome_uncertain"
                | "coordination_failed"
                | "storage_unavailable"
                | "server_unavailable"
        );
        Self { code, retryable }
    }
    fn from_body(bytes: &[u8]) -> Self {
        let code = serde_json::from_slice::<serde_json::Value>(bytes)
            .ok()
            .and_then(|v| v["error"]["code"].as_str().map(str::to_owned))
            .and_then(|c| SERVER_CODES.iter().copied().find(|known| *known == c));
        Self::new(code.unwrap_or("unavailable"))
    }
    pub fn denied() -> Self {
        Self::new("permission_denied")
    }
    fn local(tag: &'static str) -> Self {
        Self::new(
            LOCAL_CODES
                .iter()
                .copied()
                .find(|c| *c == tag)
                .unwrap_or("unavailable"),
        )
    }
}

/// `YYYY-MM-DD` shape only; the server owns calendar validity.
pub(super) fn date_shape(text: &str) -> bool {
    let b = text.as_bytes();
    b.len() == 10
        && b.iter().enumerate().all(|(i, c)| {
            if i == 4 || i == 7 {
                *c == b'-'
            } else {
                c.is_ascii_digit()
            }
        })
}
fn invalid() -> AuthenticError {
    AuthenticError::new("invalid_input")
}
fn revision_json(r: &Revision) -> Result<serde_json::Value, AuthenticError> {
    if r.sequence == 0 || r.sha256.len() != 64 || !r.sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(invalid());
    }
    Ok(json!({"sequence":r.sequence,"sha256":r.sha256}))
}

impl Bridge {
    pub(super) async fn authentic<T: serde::de::DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        payload: Option<serde_json::Value>,
    ) -> Result<T, AuthenticError> {
        let c = self.connection.lock().await;
        let d = c
            .discovery
            .as_ref()
            .ok_or(AuthenticError::local("disconnected"))?;
        let origin = d.origin.clone();
        let token = d.token.clone();
        let session = c.session.clone();
        drop(c);
        let trace = super::new_trace();
        let mut request = self
            .client
            .request(method, format!("{origin}/api/v1{path}"))
            .bearer_auth(&token)
            .header("x-diary-trace-id", &trace);
        if let Some(id) = &session {
            request = request.header("x-diary-session", id);
        }
        if let Some(payload) = payload {
            request = request.json(&payload);
        }
        let mut response = request.send().await.map_err(|_| {
            self.note(&trace, ErrorTag::Unavailable);
            AuthenticError::local("server_unavailable")
        })?;
        let success = response.status().is_success();
        if !success && let Some(tag) = super::status_tag(response.status()) {
            self.note(&trace, tag);
        }
        let limit = if success { RESPONSE_LIMIT } else { ERROR_LIMIT };
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| AuthenticError::local("invalid_response"))?
        {
            if bytes.len().saturating_add(chunk.len()) > limit {
                return Err(if success {
                    AuthenticError::local("response_limit")
                } else {
                    AuthenticError::new("unavailable")
                });
            }
            bytes.extend_from_slice(&chunk);
        }
        if !success {
            return Err(AuthenticError::from_body(&bytes));
        }
        serde_json::from_slice(&bytes).map_err(|_| AuthenticError::local("invalid_response"))
    }

    pub async fn list_authentic(
        &self,
        cursor: Option<String>,
    ) -> Result<AuthenticList, AuthenticError> {
        let path = match cursor {
            Some(c) if date_shape(&c) => format!("/authentic?cursor={c}"),
            Some(_) => return Err(invalid()),
            None => "/authentic".into(),
        };
        self.authentic(Method::GET, &path, None).await
    }
    pub async fn load_authentic(&self, date: String) -> Result<AuthenticFile, AuthenticError> {
        if !date_shape(&date) {
            return Err(invalid());
        }
        self.authentic(Method::GET, &format!("/authentic/{date}"), None)
            .await
    }
    pub async fn save_authentic(
        &self,
        date: String,
        operation_id: String,
        expected_revision: Option<Revision>,
        body: String,
        buffer_generation: u64,
    ) -> Result<SaveAck, AuthenticError> {
        if !date_shape(&date) || !valid_id(&operation_id) || body.len() > BODY_LIMIT {
            return Err(invalid());
        }
        let expected = expected_revision.as_ref().map(revision_json).transpose()?;
        self.authentic(
            Method::PUT,
            &format!("/authentic/{date}"),
            Some(json!({
                "operation_id": operation_id,
                "expected_revision": expected,
                "body": body,
                "buffer_generation": buffer_generation,
            })),
        )
        .await
    }
    pub async fn create_empty_authentic(
        &self,
        date: String,
        operation_id: String,
    ) -> Result<SaveAck, AuthenticError> {
        if !date_shape(&date) || !valid_id(&operation_id) {
            return Err(invalid());
        }
        self.authentic(
            Method::POST,
            &format!("/authentic/{date}/empty"),
            Some(json!({"operation_id": operation_id})),
        )
        .await
    }
    pub async fn refresh_authentic(&self) -> Result<RefreshReport, AuthenticError> {
        self.authentic(Method::POST, "/authentic/refresh", None)
            .await
    }
    pub async fn get_operation(
        &self,
        operation_id: String,
    ) -> Result<OperationState, AuthenticError> {
        if !valid_id(&operation_id) {
            return Err(invalid());
        }
        let result: OperationState = self
            .authentic(Method::GET, &format!("/operations/{operation_id}"), None)
            .await?;
        if result.operation_id != operation_id
            || (result.kind.as_deref() == Some("source_trash")) != result.trash.is_some()
            || (result.kind.as_deref() == Some("export")) != result.export.is_some()
            || result.export.as_ref().is_some_and(|r| {
                !r.valid()
                    || r.operation_id != operation_id
                    || Some(r.checkpoint) != result.checkpoint
            })
            || result.trash.as_ref().is_some_and(|r| {
                !r.valid()
                    || r.operation_id != operation_id
                    || Some(r.checkpoint) != result.checkpoint
            })
        {
            return Err(AuthenticError::new("invalid_response"));
        }
        Ok(result)
    }
}

#[cfg(test)]
pub(crate) mod tests;

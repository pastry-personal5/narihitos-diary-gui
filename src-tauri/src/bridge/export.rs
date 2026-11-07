//! Fixed export commands. DTOs carry IDs, digests and allowlisted codes only; the bearer token
//! exists in the preview response and nowhere else.
use super::validate::{date_shape, invalid, sha256, uuid};
use super::{AuthenticError, Bridge};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportTarget {
    Icloud,
    Notion,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportRevision {
    pub sequence: u64,
    pub sha256: String,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StartExportRequest {
    pub operation_id: String,
    pub date: String,
    pub target: ExportTarget,
    pub expected_revision: ExportRevision,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfirmExportRequest {
    pub step_id: String,
    pub expected_checkpoint: u64,
    pub token: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportState {
    Running,
    NeedsConfirmation,
    Uncertain,
    Succeeded,
    Failed,
    Cancelled,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct ExportPage {
    pub operations: Vec<ExportResult>,
    pub next_cursor: Option<String>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct ExportResult {
    pub kind: String,
    pub operation_id: String,
    pub target: ExportTarget,
    pub date: String,
    pub revision: ExportRevision,
    pub state: ExportState,
    pub checkpoint: u64,
    pub reason: Option<String>,
}
/// `outcome` is the state for a recorded or started operation, or `needs_confirmation` with a
/// preview.
#[derive(Serialize, Deserialize)]
pub struct StartExportResult {
    pub outcome: String,
    pub operation_id: String,
    #[serde(flatten)]
    pub detail: serde_json::Map<String, serde_json::Value>,
}
impl std::fmt::Debug for StartExportResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StartExportResult")
            .field("outcome", &self.outcome)
            .finish_non_exhaustive()
    }
}
#[derive(Debug, Serialize, Deserialize)]
pub struct ExportAcknowledged {
    pub operation_id: String,
    pub step_id: String,
    pub admitted_checkpoint: u64,
}

/// The server owns calendar validity; this only keeps malformed text off the wire.
impl ExportResult {
    pub(super) fn valid(&self) -> bool {
        self.kind == "export"
            && uuid(&self.operation_id)
            && sha256(&self.revision.sha256)
            && self.date.len() == 10
            && self.checkpoint > 0
            && self.checkpoint <= i64::MAX as u64
            && self.reason.as_ref().is_none_or(|r| r.len() <= 32)
    }
}
impl Bridge {
    pub async fn start_export(
        &self,
        request: StartExportRequest,
    ) -> Result<StartExportResult, AuthenticError> {
        if !uuid(&request.operation_id)
            || !sha256(&request.expected_revision.sha256)
            || request.expected_revision.sequence > i64::MAX as u64
            || !date_shape(&request.date)
        {
            return Err(invalid());
        }
        let result: StartExportResult = self
            .authentic(Method::POST, "/exports", Some(json!(request)))
            .await?;
        let token_ok = result.outcome != "needs_confirmation"
            || result
                .detail
                .get("token")
                .and_then(|t| t.as_str())
                .is_some_and(|t| !t.is_empty() && t.len() <= 128);
        if result.operation_id != request.operation_id
            || !matches!(
                result.outcome.as_str(),
                "running"
                    | "needs_confirmation"
                    | "uncertain"
                    | "succeeded"
                    | "failed"
                    | "cancelled"
            )
            || !token_ok
        {
            return Err(AuthenticError::new("invalid_response"));
        }
        Ok(result)
    }
    pub async fn confirm_export(
        &self,
        operation_id: String,
        request: ConfirmExportRequest,
    ) -> Result<ExportAcknowledged, AuthenticError> {
        if !uuid(&operation_id)
            || !uuid(&request.step_id)
            || request.token.is_empty()
            || request.token.len() > 128
            || request.expected_checkpoint == 0
            || request.expected_checkpoint > i64::MAX as u64 - 1
        {
            return Err(invalid());
        }
        let result: ExportAcknowledged = self
            .authentic(
                Method::POST,
                &format!("/exports/{operation_id}/confirmation"),
                Some(json!(request)),
            )
            .await?;
        if result.operation_id != operation_id
            || result.step_id != request.step_id
            || result.admitted_checkpoint == 0
        {
            return Err(AuthenticError::new("invalid_response"));
        }
        Ok(result)
    }

    /// Resolves an `uncertain` export: a matching read-back succeeds, otherwise a preview.
    pub async fn abandon_export(
        &self,
        operation_id: String,
    ) -> Result<StartExportResult, AuthenticError> {
        if !uuid(&operation_id) {
            return Err(invalid());
        }
        let result: StartExportResult = self
            .authentic(
                Method::POST,
                &format!("/exports/{operation_id}/abandon"),
                None,
            )
            .await?;
        let token_ok = result.outcome != "needs_confirmation"
            || result
                .detail
                .get("token")
                .and_then(|t| t.as_str())
                .is_some_and(|t| !t.is_empty() && t.len() <= 128);
        if result.operation_id != operation_id
            || !matches!(result.outcome.as_str(), "needs_confirmation" | "succeeded")
            || !token_ok
        {
            return Err(AuthenticError::new("invalid_response"));
        }
        Ok(result)
    }
    pub async fn confirm_abandon_export(
        &self,
        operation_id: String,
        request: ConfirmExportRequest,
    ) -> Result<ExportResult, AuthenticError> {
        if !uuid(&operation_id)
            || !uuid(&request.step_id)
            || request.token.is_empty()
            || request.token.len() > 128
            || request.expected_checkpoint == 0
            || request.expected_checkpoint > i64::MAX as u64 - 1
        {
            return Err(invalid());
        }
        let result: ExportResult = self
            .authentic(
                Method::POST,
                &format!("/exports/{operation_id}/abandon/confirmation"),
                Some(json!(request)),
            )
            .await?;
        if !result.valid() || result.operation_id != operation_id {
            return Err(AuthenticError::new("invalid_response"));
        }
        Ok(result)
    }

    /// Unfinished exports (paused, running or uncertain), including ones that survived a restart.
    pub async fn list_exports(&self) -> Result<ExportPage, AuthenticError> {
        let page: ExportPage = self
            .authentic(
                Method::GET,
                "/operations?kind=export&state=unfinished&limit=20",
                None,
            )
            .await?;
        if page.operations.len() > 20 || !page.operations.iter().all(ExportResult::valid) {
            return Err(AuthenticError::new("invalid_response"));
        }
        Ok(page)
    }
}

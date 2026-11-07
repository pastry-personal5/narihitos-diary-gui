//! Fixed source Trash commands. Filesystem paths and native receipts are not DTOs.
use super::validate::{digest, invalid, uuid};
use super::{AuthenticError, Bridge};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrashAction {
    Initial,
    Continuation,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrashPreviewRequest {
    pub kind: TrashAction,
    pub operation_id: Option<String>,
    pub expected_checkpoint: u64,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrashConfirmRequest {
    pub kind: TrashAction,
    pub operation_id: String,
    pub step_id: String,
    pub expected_checkpoint: u64,
    pub plan_hash: String,
    pub ordinals: Vec<u8>,
    pub token: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrashState {
    Running,
    NeedsConfirmation,
    Uncertain,
    Succeeded,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrashMemberState {
    Pending,
    Moved,
    Missing,
    Failed,
    Uncertain,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrashReason {
    Unavailable,
    Changed,
    UnsafePath,
    OwnershipAmbiguous,
    RootChanged,
    Uncertain,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct TrashMember {
    pub ordinal: u8,
    pub relative_label: String,
    pub state: TrashMemberState,
    pub reason: Option<TrashReason>,
    pub retry_eligible: bool,
}
#[derive(Serialize, Deserialize)]
pub struct TrashPreview {
    pub token: String,
    pub plan_hash: String,
    pub expected_checkpoint: u64,
    pub expires_in_ms: u64,
    pub ordinals: Vec<u8>,
    pub members: Vec<TrashMember>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct TrashAcknowledged {
    pub operation_id: String,
    pub step_id: String,
    pub admitted_checkpoint: u64,
}
#[derive(Debug, Serialize, Deserialize)]
pub enum TrashKind {
    #[serde(rename = "source_trash")]
    SourceTrash,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct TrashResult {
    pub kind: TrashKind,
    pub operation_id: String,
    pub source_id: String,
    pub plan_hash: String,
    pub checkpoint: u64,
    pub state: TrashState,
    pub members: Vec<TrashMember>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct TrashPage {
    pub operations: Vec<TrashResult>,
    pub next_cursor: Option<String>,
}
fn bad() -> AuthenticError {
    AuthenticError::new("invalid_response")
}
fn ordinals(ordinals: &[u8]) -> bool {
    !ordinals.is_empty()
        && ordinals.len() <= 2
        && ordinals.iter().all(|o| *o <= 1)
        && ordinals.windows(2).all(|w| w[0] < w[1])
}
fn label(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 1024
        && !s.contains('\\')
        && !s.chars().any(char::is_control)
        && s.split('/').all(|p| !matches!(p, "" | "." | ".."))
}
fn members(m: &[TrashMember]) -> bool {
    m.len() == 2
        && m.iter().enumerate().all(|(i, m)| {
            m.ordinal as usize == i
                && label(&m.relative_label)
                && (!matches!(
                    m.state,
                    TrashMemberState::Moved
                        | TrashMemberState::Missing
                        | TrashMemberState::Uncertain
                ) || !m.retry_eligible)
                && (m.state != TrashMemberState::Missing || i == 1)
        })
}
impl TrashResult {
    pub(super) fn valid(&self) -> bool {
        uuid(&self.operation_id)
            && digest(&self.source_id, 32)
            && digest(&self.plan_hash, 64)
            && self.checkpoint > 0
            && self.checkpoint <= i64::MAX as u64
            && members(&self.members)
            && (self.state != TrashState::Succeeded
                || self.members.iter().all(|m| {
                    matches!(m.state, TrashMemberState::Moved | TrashMemberState::Missing)
                }))
            && (self.state != TrashState::Uncertain
                || self
                    .members
                    .iter()
                    .any(|m| m.state == TrashMemberState::Uncertain))
    }
}
impl Bridge {
    pub async fn preview_source_delete(
        &self,
        source_id: String,
        request: TrashPreviewRequest,
    ) -> Result<TrashPreview, AuthenticError> {
        if !digest(&source_id, 32)
            || request.expected_checkpoint > i64::MAX as u64 - 1
            || match request.kind {
                TrashAction::Initial => {
                    request.operation_id.is_some() || request.expected_checkpoint != 0
                }
                TrashAction::Continuation => {
                    request.operation_id.as_ref().is_none_or(|s| !uuid(s))
                        || request.expected_checkpoint == 0
                }
            }
        {
            return Err(invalid());
        }
        let result: TrashPreview = self
            .authentic(
                Method::POST,
                &format!("/sources/{source_id}/trash-preview"),
                Some(json!(request)),
            )
            .await?;
        if result.token.is_empty()
            || result.token.len() > 128
            || !digest(&result.plan_hash, 64)
            || result.expected_checkpoint != request.expected_checkpoint
            || result.expires_in_ms != 60_000
            || !ordinals(&result.ordinals)
            || !members(&result.members)
        {
            return Err(bad());
        }
        Ok(result)
    }
    pub async fn confirm_source_delete(
        &self,
        source_id: String,
        request: TrashConfirmRequest,
    ) -> Result<TrashAcknowledged, AuthenticError> {
        if !digest(&source_id, 32)
            || !uuid(&request.operation_id)
            || !uuid(&request.step_id)
            || !digest(&request.plan_hash, 64)
            || request.token.len() > 128
            || !ordinals(&request.ordinals)
            || request.expected_checkpoint > i64::MAX as u64 - 1
            || (request.kind == TrashAction::Initial
                && (request.expected_checkpoint != 0 || request.ordinals != [0, 1]))
            || (request.kind == TrashAction::Continuation && request.expected_checkpoint == 0)
        {
            return Err(invalid());
        }
        let result: TrashAcknowledged = self
            .authentic(
                Method::POST,
                &format!("/sources/{source_id}/trash"),
                Some(json!(request)),
            )
            .await?;
        if result.operation_id != request.operation_id
            || result.step_id != request.step_id
            || result.admitted_checkpoint != request.expected_checkpoint + 1
        {
            return Err(bad());
        }
        Ok(result)
    }
    pub async fn list_source_deletions(
        &self,
        cursor: Option<String>,
        limit: Option<u16>,
    ) -> Result<TrashPage, AuthenticError> {
        let limit = limit.unwrap_or(50);
        if limit == 0
            || limit > 100
            || cursor.as_ref().is_some_and(|c| {
                c.is_empty()
                    || c.len() > 128
                    || !c
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
            })
        {
            return Err(invalid());
        }
        let path = format!(
            "/operations?kind=source_trash&state=unfinished&limit={limit}{}",
            cursor
                .as_ref()
                .map(|c| format!("&cursor={c}"))
                .unwrap_or_default()
        );
        let result: TrashPage = self.authentic(Method::GET, &path, None).await?;
        if result.operations.len() > limit as usize
            || result
                .operations
                .iter()
                .any(|o| !o.valid() || o.state == TrashState::Succeeded)
            || result.next_cursor.as_ref().is_some_and(|c| {
                c.is_empty()
                    || c.len() > 128
                    || !c
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
            })
        {
            return Err(bad());
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn response_members_are_bounded_relative_and_cannot_expose_receipts() {
        for unsafe_label in [
            "/Users/private/audio.m4a",
            "../audio.m4a",
            "a/../audio.m4a",
            "a//audio.m4a",
            "a\\audio.m4a",
            "a\n.m4a",
        ] {
            assert!(!label(unsafe_label));
        }
        assert!(label("2024/02/29/source.assets/audio.m4a"));
        assert!(!label(&"a".repeat(1025)));
        assert!(serde_json::from_value::<TrashReason>(json!("native_exception")).is_err());
        assert!(serde_json::from_value::<TrashState>(json!("paused")).is_err());
    }
}

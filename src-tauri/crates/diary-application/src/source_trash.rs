//! Transport-independent confirmed source/audio Trash and durable recovery lookup.
use crate::operations::{OperationId, StepId};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteAction {
    Initial,
    Continuation,
}
#[derive(Debug, Clone)]
pub struct DeletePreviewRequest {
    pub action: DeleteAction,
    pub source_id: String,
    pub operation_id: Option<OperationId>,
    pub expected_checkpoint: u64,
}
#[derive(Clone)]
pub struct DeleteConfirmRequest {
    pub action: DeleteAction,
    pub source_id: String,
    pub operation_id: OperationId,
    pub step_id: StepId,
    pub expected_checkpoint: u64,
    pub plan_hash: String,
    pub ordinals: Vec<u8>,
    pub token: String,
}
impl fmt::Debug for DeleteConfirmRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DeleteConfirmRequest")
            .field("operation_id", &self.operation_id)
            .finish_non_exhaustive()
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteState {
    Running,
    NeedsConfirmation,
    Uncertain,
    Succeeded,
}
impl DeleteState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::NeedsConfirmation => "needs_confirmation",
            Self::Uncertain => "uncertain",
            Self::Succeeded => "succeeded",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteMemberState {
    Pending,
    Moved,
    Missing,
    Failed,
    Uncertain,
}
impl DeleteMemberState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Moved => "moved",
            Self::Missing => "missing",
            Self::Failed => "failed",
            Self::Uncertain => "uncertain",
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteMember {
    pub ordinal: u8,
    pub relative_label: String,
    pub state: DeleteMemberState,
    pub reason: Option<&'static str>,
    pub retry_eligible: bool,
}
#[derive(Clone)]
pub struct DeletePreview {
    pub token: String,
    pub plan_hash: String,
    pub expected_checkpoint: u64,
    pub expires_in_ms: u64,
    pub ordinals: Vec<u8>,
    pub members: Vec<DeleteMember>,
}
impl fmt::Debug for DeletePreview {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DeletePreview")
            .field("members", &self.members)
            .finish_non_exhaustive()
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteAcknowledged {
    pub operation_id: OperationId,
    pub step_id: StepId,
    /// The checkpoint admitted by this step, including on duplicate requests.
    pub admitted_checkpoint: u64,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteResult {
    pub operation_id: OperationId,
    pub source_id: String,
    pub plan_hash: String,
    pub checkpoint: u64,
    pub state: DeleteState,
    pub members: Vec<DeleteMember>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeletePage {
    pub operations: Vec<DeleteResult>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteError {
    Invalid,
    Unsupported,
    SourceStale,
    Changed,
    OwnershipAmbiguous,
    CloudUnavailable,
    Busy,
    Limit,
    Expired,
    PreviewStale,
    Mismatch,
    CheckpointMismatch,
    Uncertain,
    NotFound,
    Storage,
}
pub trait SourceTrash: Send + Sync {
    /// Adapter evidence/readiness only; exact per-file ownership still needs preview.
    fn available(&self) -> bool {
        false
    }
    fn preview(
        &self,
        session: &str,
        generation: u64,
        request: DeletePreviewRequest,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<DeletePreview, DeleteError>;
    fn confirm(
        &self,
        session: &str,
        generation: u64,
        request: DeleteConfirmRequest,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<DeleteAcknowledged, DeleteError>;
    fn operation(&self, id: &OperationId) -> Result<Option<DeleteResult>, DeleteError>;
    fn unfinished(&self, cursor: Option<&str>, limit: usize) -> Result<DeletePage, DeleteError>;
}

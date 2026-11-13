//! Destination-neutral export model: the saved-revision snapshot, remote observation, the total
//! decision table, confirmation types and the adapter port. No destination, token or body is
//! stored here; the service lives in `diary-infrastructure`.
use crate::operations::{OperationId, StepId};
/// Re-exported so adapter decorators outside this crate need no extra dependency.
pub use diary_domain::authentic::DiaryDate as ExportDate;
use diary_domain::authentic::{AuthenticRevision, DiaryDate, MAX_BODY_BYTES};
use std::fmt;

pub const TOKEN_TTL_MS: u64 = 60_000;
/// The rendered file is the body plus front matter.
pub const MAX_SNAPSHOT_BYTES: usize = MAX_BODY_BYTES + 65_536;
pub const KIND: &str = "export";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentKind {
    Authentic,
    Illustrated,
}
impl ContentKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Authentic => "authentic",
            Self::Illustrated => "illustrated",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "authentic" => Some(Self::Authentic),
            "illustrated" => Some(Self::Illustrated),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Icloud,
    Notion,
}
impl Target {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Icloud => "icloud",
            Self::Notion => "notion",
        }
    }
    pub fn parse(value: &str) -> Option<Self> {
        [Self::Icloud, Self::Notion]
            .into_iter()
            .find(|t| t.as_str() == value)
    }
}

/// The acknowledged saved revision's complete file bytes. Unsaved buffers, candidates and
/// sources never become snapshots.
#[derive(Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub content_kind: ContentKind,
    pub illustrated_version: Option<String>,
    /// Prepared JPEGs only, bounded before admission; empty for Authentic.
    pub photos: Vec<Vec<u8>>,
    pub date: DiaryDate,
    pub authentic_id: String,
    pub revision: AuthenticRevision,
    pub bytes: Vec<u8>,
}
impl fmt::Debug for Snapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Snapshot")
            .field("revision", &self.revision)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Write {
    /// No-clobber creation of an absent target.
    Create,
    /// Replacement guarded by the observed fingerprint.
    Replace,
}
impl Write {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Replace => "replace",
        }
    }
}

/// What a destination holds for one date. `fingerprint` is the version evidence the service
/// offers; `digest` covers the complete normalized relevant content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observation {
    pub present: bool,
    /// False for File Provider states such as download-needed: never absence.
    pub available: bool,
    pub fingerprint: String,
    pub digest: String,
    pub ids: String,
    /// Why `available` is false; `None` when available.
    pub unavailable: Option<Unavailable>,
}
impl Observation {
    pub fn absent() -> Self {
        Self {
            present: false,
            available: true,
            fingerprint: String::new(),
            digest: String::new(),
            ids: String::new(),
            unavailable: None,
        }
    }
    pub fn unavailable(reason: Unavailable) -> Self {
        Self {
            available: false,
            unavailable: Some(reason),
            ..Self::absent()
        }
    }
}

/// Allowlisted reasons a destination cannot be observed; never free text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unavailable {
    /// A File Provider placeholder or non-current download state.
    DownloadNeeded,
    Unreadable,
    /// macOS privacy (TCC) denied access to the root.
    PermissionDenied,
    /// Coordination failed or timed out.
    CoordinationFailed,
    /// The destination refused the credential (Notion 401/403 or an unshared page).
    Unauthorized,
    /// The destination asked to wait longer than the retry budget.
    Throttled,
    /// The destination could not be reached before anything was sent.
    Network,
    /// Duplicate or disordered remote hierarchy that needs external repair.
    Structural,
}
impl Unavailable {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DownloadNeeded => "download_needed",
            Self::Unreadable => "unreadable",
            Self::PermissionDenied => "permission_denied",
            Self::CoordinationFailed => "coordination_failed",
            Self::Unauthorized => "unauthorized",
            Self::Throttled => "throttled",
            Self::Network => "network",
            Self::Structural => "structural_conflict",
        }
    }
    pub fn parse(value: &str) -> Option<Self> {
        [
            Self::DownloadNeeded,
            Self::Unreadable,
            Self::PermissionDenied,
            Self::CoordinationFailed,
            Self::Unauthorized,
            Self::Throttled,
            Self::Network,
            Self::Structural,
        ]
        .into_iter()
        .find(|r| r.as_str() == value)
    }
}

/// The last verified publication for one `(destination, authentic id)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Receipt {
    pub revision_sha: String,
    pub projection_digest: String,
    pub remote_ids: String,
    pub remote_fingerprint: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// The remote differs from the last verified receipt.
    Changed,
    /// A known receipt's target is gone.
    Deleted,
    /// An existing target no receipt owns.
    Unowned,
    /// Abandoning an `uncertain` export whose read-back does not match the saved revision.
    Uncertain,
}
impl Reason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Changed => "changed",
            Self::Deleted => "deleted",
            Self::Unowned => "unowned",
            Self::Uncertain => "uncertain",
        }
    }
    pub fn parse(value: &str) -> Option<Self> {
        [Self::Changed, Self::Deleted, Self::Unowned, Self::Uncertain]
            .into_iter()
            .find(|r| r.as_str() == value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Unchanged local revision and unchanged remote digest.
    Noop,
    Write(Write),
    /// Preview only; confirmation authorizes exactly this replacement or recreation.
    Confirm(Reason, Write),
    Unavailable,
    /// An unresolved earlier write reserves the target.
    Uncertain,
}

/// Total over every input. `digest` is the snapshot digest.
pub fn decide(
    receipt: Option<&Receipt>,
    digest: &str,
    remote: &Observation,
    unresolved: bool,
) -> Decision {
    if !remote.available {
        return Decision::Unavailable;
    }
    if unresolved {
        return Decision::Uncertain;
    }
    match (receipt, remote.present) {
        (None, false) => Decision::Write(Write::Create),
        (None, true) => Decision::Confirm(Reason::Unowned, Write::Replace),
        (Some(_), false) => Decision::Confirm(Reason::Deleted, Write::Create),
        (Some(r), true) if r.remote_fingerprint != remote.fingerprint => {
            Decision::Confirm(Reason::Changed, Write::Replace)
        }
        (Some(r), true) if r.projection_digest == digest && remote.digest == digest => {
            Decision::Noop
        }
        (Some(_), true) => Decision::Write(Write::Replace),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Applied {
    Done,
    /// Definitely not applied.
    Failed,
    /// A lost reply or ambiguous failure: read back, never replay.
    Unknown,
}

/// The destination port. A real adapter (P1-M26/M27) supplies availability evidence; the
/// fake in `diary-infrastructure` exists only for tests and the fault-injection build.
pub trait ExportAdapter: Send + Sync {
    fn progress(&self, _date: &DiaryDate) -> Option<ExportProgress> {
        None
    }
    fn content_kind(&self) -> ContentKind {
        ContentKind::Authentic
    }
    fn target(&self) -> Target;
    /// Canonical root path or root page ID; part of the destination key.
    fn root(&self) -> String;
    fn observe(&self, date: &DiaryDate) -> Observation;
    /// `expected` is the observation the confirmation authorized; the adapter re-checks it
    /// atomically with the write and returns `Failed` without writing on a mismatch.
    fn apply(
        &self,
        date: &DiaryDate,
        bytes: &[u8],
        write: Write,
        expected: &Observation,
    ) -> Applied;
    fn read_back(&self, date: &DiaryDate) -> Observation;
    fn apply_snapshot(
        &self,
        snapshot: &Snapshot,
        write: Write,
        expected: &Observation,
        _cancelled: &dyn Fn() -> bool,
    ) -> Applied {
        self.apply(&snapshot.date, &snapshot.bytes, write, expected)
    }
    /// A matching body alone cannot establish ownership after an ambiguous write.
    fn verified(&self, _date: &DiaryDate, _seen: &Observation) -> bool {
        true
    }
    /// `Err` while the destination's identity cannot be established yet (Notion fetches its
    /// workspace lazily). The service builds no destination key and creates no intent until
    /// this succeeds, so a placeholder `root()` can never be persisted.
    fn ready(&self) -> Result<(), Unavailable> {
        Ok(())
    }
    /// The digest a remote holding exactly this snapshot reports in `Observation::digest`.
    /// `None` means the snapshot's own SHA-256 (byte-for-byte destinations such as iCloud).
    fn digest_of(&self, _snapshot: &[u8]) -> Option<String> {
        None
    }
    /// Count of constructs the projection kept only as literal text; surfaced as a message.
    fn warnings(&self, _snapshot: &[u8]) -> u32 {
        0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportState {
    Running,
    NeedsConfirmation,
    Uncertain,
    Succeeded,
    Failed,
    Cancelled,
}
impl ExportState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::NeedsConfirmation => "needs_confirmation",
            Self::Uncertain => "uncertain",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Cancelled)
    }
}

#[derive(Debug, Clone)]
pub struct StartRequest {
    pub operation_id: OperationId,
    pub date: DiaryDate,
    pub target: Target,
    pub expected: AuthenticRevision,
}
#[derive(Debug, Clone)]
pub struct IllustratedStartRequest {
    pub operation_id: OperationId,
    pub date: DiaryDate,
    pub expected_version: String,
}
#[derive(Clone)]
pub struct ConfirmRequest {
    pub operation_id: OperationId,
    pub step_id: StepId,
    pub expected_checkpoint: u64,
    pub token: String,
}
impl fmt::Debug for ConfirmRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConfirmRequest")
            .field("operation_id", &self.operation_id)
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub struct Preview {
    pub operation_id: OperationId,
    pub expected_checkpoint: u64,
    /// The bearer exists only in this response and transient server memory.
    pub token: String,
    pub observed_fingerprint: String,
    pub reason: Reason,
    pub write: Write,
    pub expires_in_ms: u64,
}
impl fmt::Debug for Preview {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Preview")
            .field("operation_id", &self.operation_id)
            .field("reason", &self.reason)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportResult {
    pub progress: Option<ExportProgress>,
    pub content_kind: ContentKind,
    pub illustrated_version: Option<String>,
    pub operation_id: OperationId,
    /// Business-input hash, for job correlation only.
    pub input_hash: String,
    pub target: Target,
    pub date: String,
    pub revision_sequence: u64,
    pub revision_sha: String,
    pub state: ExportState,
    pub checkpoint: u64,
    /// `changed|deleted|unowned` while confirmation is needed; a bounded tag after failure.
    pub reason: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportProgress {
    pub stage: String,
    pub photos_uploaded: u32,
}

#[derive(Debug, Clone)]
pub enum Started {
    /// A new or recorded operation: running, succeeded (including a verified no-op), uncertain
    /// or failed.
    Result(ExportResult),
    Preview(Preview),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Admitted {
    pub operation_id: OperationId,
    pub step_id: StepId,
    /// The checkpoint admitted by this step, including on duplicate requests.
    pub admitted_checkpoint: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportPage {
    pub operations: Vec<ExportResult>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportError {
    Invalid,
    /// No adapter is registered for the target.
    Disabled,
    NothingToExport,
    StaleRevision,
    /// Another unfinished export holds the destination.
    Busy,
    Limit,
    Expired,
    PreviewStale,
    Mismatch,
    CheckpointMismatch,
    Uncertain,
    /// The destination cannot be observed; the typed reason is allowlisted.
    CloudUnavailable(Unavailable),
    NotFound,
    Storage,
}

/// Notified after a durable state change, never before.
pub type Notifier = std::sync::Arc<dyn Fn(&ExportResult) + Send + Sync>;

pub trait Exports: Send + Sync {
    fn available(&self, target: Target) -> bool;
    fn available_kind(&self, kind: ContentKind, target: Target) -> bool {
        kind == ContentKind::Authentic && self.available(target)
    }
    fn start_illustrated(
        &self,
        _session: &str,
        _generation: u64,
        _request: IllustratedStartRequest,
    ) -> Result<Started, ExportError> {
        Err(ExportError::Disabled)
    }
    fn start(
        &self,
        session: &str,
        generation: u64,
        request: StartRequest,
    ) -> Result<Started, ExportError>;
    fn confirm(
        &self,
        session: &str,
        generation: u64,
        request: ConfirmRequest,
    ) -> Result<Admitted, ExportError>;
    /// Declines a pending confirmation or stops a running export before `apply`. An export
    /// already past its side effect finishes by read-back. Never rolls back remote state.
    fn cancel(&self, id: &OperationId) -> Result<ExportResult, ExportError>;
    /// Resolves an `uncertain` export: a read-back equal to the saved revision succeeds with a
    /// receipt; otherwise a preview whose confirmation records `abandoned`.
    fn abandon(
        &self,
        session: &str,
        generation: u64,
        id: &OperationId,
    ) -> Result<Started, ExportError>;
    fn abandon_confirm(
        &self,
        session: &str,
        generation: u64,
        request: ConfirmRequest,
    ) -> Result<ExportResult, ExportError>;
    fn operation(&self, id: &OperationId) -> Result<Option<ExportResult>, ExportError>;
    fn unfinished(&self, cursor: Option<&str>, limit: usize) -> Result<ExportPage, ExportError>;
    /// Running, paused or uncertain exports, including ones that survived a restart.
    fn has_unfinished(&self) -> Result<bool, ExportError>;
    fn set_notifier(&self, notifier: Notifier);
}

#[cfg(test)]
mod tests;

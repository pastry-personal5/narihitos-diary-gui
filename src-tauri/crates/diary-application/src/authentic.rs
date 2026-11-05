//! Port for Authentic persistence: canonical files, guarded saves, the saved-date catalog and
//! durable operation lookup. The infrastructure crate implements it over files and SQLite.
use crate::operations::{OperationId, Record};
use diary_domain::authentic::{AuthenticHeader, AuthenticRevision, DiaryDate, FormatError};

pub const PAGE_LIMIT: usize = 200;

/// Resource key serializing every mutation of one date.
pub fn resource_key(date: &DiaryDate) -> String {
    format!("authentic:{date}")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveRequest {
    pub operation_id: OperationId,
    pub date: DiaryDate,
    /// `None` means the caller expects the date to have no file.
    pub expected: Option<AuthenticRevision>,
    pub body: String,
    /// Echoed for client correlation only; never part of the operation's business input.
    pub buffer_generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateRequest {
    pub operation_id: OperationId,
    pub date: DiaryDate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveOutcome {
    pub operation_id: OperationId,
    pub revision: AuthenticRevision,
    pub buffer_generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Loaded {
    /// No file for the date: an empty buffer that is not persisted.
    Virtual(DiaryDate),
    File {
        revision: AuthenticRevision,
        header: Box<AuthenticHeader>,
        body: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogRow {
    pub date: DiaryDate,
    pub revision: Option<AuthenticRevision>,
    /// `None` when the file parses; otherwise the readable diagnostic that blocks saving.
    pub problem: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    pub rows: Vec<CatalogRow>,
    /// Pass back as `after` for the next page.
    pub next: Option<DiaryDate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RebuildReport {
    pub files: usize,
    pub problems: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictKind {
    /// The expected revision is not the file's current revision.
    Stale,
    /// Creation expected absence but a file exists.
    Exists,
    /// The file changed between observation and publication, or a recovery found foreign bytes.
    External,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreError {
    Conflict(ConflictKind),
    /// Another unfinished operation holds this date.
    Reserved,
    /// Same operation ID with different business input.
    Mismatch,
    /// The operation was recorded as failed or cancelled; the message is a bounded tag.
    Failed(String),
    /// Outcome unknown until reconciliation; retry the original operation ID.
    Uncertain,
    /// The date has an unreadable or inconsistent file that must be fixed first.
    Blocked(String),
    Format(FormatError),
    /// Coordinated file access was refused or unavailable; nothing was written.
    Coordination,
    /// Recovery mode or an unusable database.
    Unavailable,
    Storage,
}

pub trait AuthenticStore: Send + Sync {
    fn load(&self, date: &DiaryDate) -> Result<Loaded, StoreError>;
    /// Dates strictly after `after`, ascending, at most `limit` (≤ `PAGE_LIMIT`).
    fn list(&self, after: Option<DiaryDate>, limit: usize) -> Result<Page, StoreError>;
    fn save(&self, request: SaveRequest) -> Result<SaveOutcome, StoreError>;
    fn create_empty(&self, request: CreateRequest) -> Result<SaveOutcome, StoreError>;
    /// Rescans the Authentic tree and rebuilds the catalog; never touches operations.
    fn refresh(&self) -> Result<RebuildReport, StoreError>;
    fn operation(&self, id: &OperationId) -> Result<Option<Record>, StoreError>;
}

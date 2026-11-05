//! Durable operation identities, step deduplication, reservations and startup classification.
//! The port is implemented by the SQLite ledger in `diary-infrastructure`; nothing here stores
//! tokens, raw output or content.
use crate::diagnostics::valid_uuid;

pub const MAX_OUTCOME_BYTES: usize = 1024;
pub const MAX_RESOURCES: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Readiness {
    Ready,
    /// Authoritative state is missing or damaged: reads only, no mutation.
    Recovery,
    /// Unexpected files: nothing was modified and a person must inspect.
    InspectionRequired,
}
impl Readiness {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Recovery => "recovery",
            Self::InspectionRequired => "inspection_required",
        }
    }
}

macro_rules! uuid_id {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct $name(String);
        impl $name {
            /// A lowercase hyphenated UUID.
            pub fn parse(value: &str) -> Option<Self> {
                valid_uuid(value).then(|| Self(value.to_owned()))
            }
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}
uuid_id!(OperationId);
uuid_id!(StepId);

/// Canonical resource name such as `authentic:2026-11-05`; opaque to this layer.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ResourceKey(String);
impl ResourceKey {
    pub fn parse(value: &str) -> Option<Self> {
        (!value.is_empty()
            && value.len() <= 256
            && value.bytes().all(|b| (0x21..0x7f).contains(&b)))
        .then(|| Self(value.to_owned()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationState {
    Running,
    Paused,
    Uncertain,
    Succeeded,
    Failed,
    Cancelled,
}
impl OperationState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Paused => "paused",
            Self::Uncertain => "uncertain",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
    pub fn parse(value: &str) -> Option<Self> {
        [
            Self::Running,
            Self::Paused,
            Self::Uncertain,
            Self::Succeeded,
            Self::Failed,
            Self::Cancelled,
        ]
        .into_iter()
        .find(|s| s.as_str() == value)
    }
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Cancelled)
    }
}

/// What admitted an operation. A restart compares it with the current workspace; a relative
/// path is never reinterpreted against a different root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    pub workspace_id: String,
    pub root: String,
    /// `device:inode` of the workspace root directory.
    pub native: String,
    pub adapter_version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewStep {
    pub step_id: StepId,
    /// 64 lowercase hex characters.
    pub input_hash: String,
    pub expected_checkpoint: u64,
    pub outcome: String,
    /// State after the step; `None` keeps the current state.
    pub state: Option<OperationState>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewOperation {
    pub id: OperationId,
    /// Lowercase snake-case kind, at most 64 bytes.
    pub kind: String,
    pub hash_version: u32,
    /// 64 lowercase hex characters, see `hash_input` in the infrastructure ledger.
    pub input_hash: String,
    pub resources: Vec<ResourceKey>,
    pub identity: Identity,
    /// Recorded atomically with the operation (an initial confirmation).
    pub first_step: Option<NewStep>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub id: OperationId,
    pub kind: String,
    pub hash_version: u32,
    pub input_hash: String,
    pub state: OperationState,
    pub checkpoint: u64,
    pub result: Option<String>,
    pub identity: Identity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Admission {
    New(Record),
    /// Same operation ID and business input: the recorded state, before any new precondition.
    Existing(Record),
    /// Same ID, different business input (`operation_mismatch`).
    Mismatch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepOutcome {
    Applied(Record),
    /// A duplicate step returns what it recorded, ahead of any checkpoint check.
    Recorded {
        outcome: String,
        checkpoint: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LedgerError {
    Invalid,
    NotFound,
    /// An unfinished operation holds an overlapping resource key.
    Reserved(OperationId),
    Terminal,
    CheckpointMismatch {
        current: u64,
    },
    /// Same step ID with different input.
    StepMismatch,
    /// Recovery mode or an unusable database: no write is possible.
    Unavailable,
    Storage,
}

pub trait OperationLedger {
    /// Records the operation before any side effect, or returns the existing one.
    fn admit(&self, op: NewOperation) -> Result<Admission, LedgerError>;
    fn get(&self, id: &OperationId) -> Result<Option<Record>, LedgerError>;
    fn advance(&self, id: &OperationId, step: NewStep) -> Result<StepOutcome, LedgerError>;
    /// Terminal state and bounded result; releases reservations. The row stays as a tombstone.
    fn finish(
        &self,
        id: &OperationId,
        state: OperationState,
        result: &str,
    ) -> Result<(), LedgerError>;
    /// Every non-terminal operation (these hold reservations).
    fn unfinished(&self) -> Result<Vec<Record>, LedgerError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blocker {
    RootChanged,
    AdapterChanged,
    /// No reconciler is registered for this kind, so its side effects cannot be judged.
    NoReconciler,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Class {
    /// Same workspace and root: a registered reconciler may continue it.
    Resume,
    /// Paused with a readable reason; its reservation is kept.
    Blocked(Blocker),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Classified {
    pub id: OperationId,
    pub kind: String,
    pub class: Class,
}

/// Pure startup classification of unfinished operations. Never marks one failed or succeeded.
pub fn classify(
    unfinished: &[Record],
    current: &Identity,
    reconcilers: &[&str],
) -> Vec<Classified> {
    unfinished
        .iter()
        .map(|r| {
            let i = &r.identity;
            let class = if i.workspace_id != current.workspace_id
                || i.root != current.root
                || i.native != current.native
            {
                Class::Blocked(Blocker::RootChanged)
            } else if i.adapter_version != current.adapter_version {
                Class::Blocked(Blocker::AdapterChanged)
            } else if !reconcilers.contains(&r.kind.as_str()) {
                Class::Blocked(Blocker::NoReconciler)
            } else {
                Class::Resume
            };
            Classified {
                id: r.id.clone(),
                kind: r.kind.clone(),
                class,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests;

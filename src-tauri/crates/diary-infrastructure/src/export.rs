//! Export coordination service: saved-revision snapshot, durable intent before any destination
//! call, session-bound previews, token-consuming step admission and crash recovery. The
//! optional iCloud adapter is registered from validated configuration.
use crate::{ledger::hash_input, lifecycle::random_uuid, sqlite::Db};
use diary_application::{
    authentic::{AuthenticStore, Loaded},
    export::*,
    operations::*,
};
use diary_domain::authentic::{DiaryDate, render};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use store::{Intent, Publication, Row, StoreError};

mod confirm;
#[cfg(any(test, feature = "fault-injection"))]
pub mod fake;
pub mod icloud;
mod illustrated_snapshot;
mod jpeg;
pub mod notion;
mod recovery;
mod run;
mod snapshot;
mod start;
pub mod store;

const MAX_PREVIEWS: usize = 64;

pub(crate) fn hex_sha(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn monotonic() -> u64 {
    let mut value = std::mem::MaybeUninit::<libc::timespec>::uninit();
    // SAFETY: successful clock_gettime initializes the timespec; failure expires work.
    if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC_RAW, value.as_mut_ptr()) } != 0 {
        return u64::MAX;
    }
    let value = unsafe { value.assume_init() };
    (value.tv_sec as u64)
        .saturating_mul(1000)
        .saturating_add(value.tv_nsec as u64 / 1_000_000)
}
fn stored(e: StoreError) -> ExportError {
    match e {
        StoreError::Invalid => ExportError::Invalid,
        StoreError::TokenUsed => ExportError::Expired,
        StoreError::Ledger(LedgerError::StepMismatch) => ExportError::Mismatch,
        StoreError::Ledger(LedgerError::CheckpointMismatch { .. } | LedgerError::Terminal) => {
            ExportError::CheckpointMismatch
        }
        StoreError::Ledger(LedgerError::NotFound) => ExportError::NotFound,
        _ => ExportError::Storage,
    }
}
fn uuid() -> Result<String, ExportError> {
    random_uuid().map_err(|_| ExportError::Storage)
}

struct Prepared {
    operation: OperationId,
    session: String,
    generation: u64,
    checkpoint: u64,
    fingerprint: String,
    write: Write,
    abandon: bool,
    expires: u64,
}
/// Everything the background step needs; the snapshot never leaves memory except in SQLite.
struct Job {
    id: OperationId,
    target: Target,
    snapshot: Snapshot,
    write: Write,
    expected: Observation,
}

struct ExportCoordinatorState {
    db: Arc<Db>,
    identity: Identity,
    authentic: Arc<dyn AuthenticStore>,
    illustrated: Option<diary_application::illustrated::SharedStore>,
    adapters: Vec<Arc<dyn ExportAdapter>>,
    previews: Mutex<BTreeMap<String, Prepared>>,
    /// Running exports asked to stop before `apply`.
    cancels: Mutex<std::collections::BTreeSet<String>>,
    notifier: Mutex<Option<Notifier>>,
    ready: AtomicBool,
    clock: Box<dyn Fn() -> u64 + Send + Sync>,
    #[cfg(any(test, feature = "fault-injection"))]
    fault: Mutex<Option<String>>,
}

#[derive(Clone)]
pub struct ExportCoordinator(Arc<ExportCoordinatorState>);

impl ExportCoordinator {
    pub fn new(
        db: Arc<Db>,
        identity: Identity,
        authentic: Arc<dyn AuthenticStore>,
        adapters: Vec<Arc<dyn ExportAdapter>>,
    ) -> Self {
        Self(Arc::new(ExportCoordinatorState {
            db,
            identity,
            authentic,
            illustrated: None,
            adapters,
            previews: Mutex::new(BTreeMap::new()),
            cancels: Mutex::new(Default::default()),
            notifier: Mutex::new(None),
            ready: AtomicBool::new(false),
            clock: Box::new(monotonic),
            #[cfg(any(test, feature = "fault-injection"))]
            fault: Mutex::new(None),
        }))
    }
    pub fn with_illustrated(mut self, store: diary_application::illustrated::SharedStore) -> Self {
        Arc::get_mut(&mut self.0)
            .expect("unshared coordinator")
            .illustrated = Some(store);
        self
    }
    #[cfg(test)]
    pub(crate) fn with_fault(self, point: &str) -> Self {
        *self.0.fault.lock().unwrap() = Some(format!("export:{point}"));
        self
    }
    /// Reconciles interrupted exports before exposing mutation. Never calls `apply`.
    pub fn recover(&self) -> Result<usize, ExportError> {
        self.0.recover()
    }
}

impl ExportCoordinatorState {
    fn hit(&self, point: &str) -> Result<(), ExportError> {
        #[cfg(any(test, feature = "fault-injection"))]
        {
            let injected = self.fault.lock().ok().and_then(|f| f.clone());
            #[cfg(feature = "fault-injection")]
            let injected = injected.or_else(|| std::env::var("DIARY_FAULT").ok());
            if let Some(v) = injected {
                if v == format!("export:{point}:kill") {
                    // Intentional process death: no unwind, no destructors.
                    // SAFETY: test-only fault request for immediate termination.
                    unsafe { libc::_exit(86) }
                }
                if v == format!("export:{point}:io") {
                    return Err(ExportError::Storage);
                }
            }
        }
        let _ = point;
        Ok(())
    }

    fn adapter(
        &self,
        kind: ContentKind,
        target: Target,
    ) -> Result<&Arc<dyn ExportAdapter>, ExportError> {
        self.adapters
            .iter()
            .find(|a| a.target() == target && a.content_kind() == kind)
            .ok_or(ExportError::Disabled)
    }

    fn key(&self, a: &dyn ExportAdapter) -> Result<String, ExportError> {
        a.ready().map_err(ExportError::CloudUnavailable)?;
        Ok(hash_input(
            "export_destination",
            1,
            &[
                ("target", a.target().as_str()),
                ("root", &a.root()),
                ("workspace", &self.identity.workspace_id),
            ],
        ))
    }

    fn result(&self, rec: &Record, row: &Row) -> ExportResult {
        let state = match rec.state {
            OperationState::Running => ExportState::Running,
            OperationState::Paused => ExportState::NeedsConfirmation,
            OperationState::Uncertain => ExportState::Uncertain,
            OperationState::Succeeded => ExportState::Succeeded,
            OperationState::Failed => ExportState::Failed,
            OperationState::Cancelled => ExportState::Cancelled,
        };
        ExportResult {
            progress: if state == ExportState::Running {
                DiaryDate::parse(&row.date).and_then(|d| {
                    self.adapter(row.content_kind, row.target)
                        .ok()?
                        .progress(&d)
                })
            } else {
                None
            },
            content_kind: row.content_kind,
            illustrated_version: row.illustrated_version.clone(),
            operation_id: rec.id.clone(),
            input_hash: rec.input_hash.clone(),
            target: row.target,
            date: row.date.clone(),
            revision_sequence: row.revision_sequence,
            revision_sha: row.revision_sha.clone(),
            state,
            checkpoint: rec.checkpoint,
            reason: match state {
                ExportState::Failed | ExportState::Cancelled => rec.result.clone(),
                _ => row.reason.clone(),
            },
        }
    }

    fn load(&self, id: &OperationId) -> Result<(Record, Row), ExportError> {
        let rec = self
            .db
            .get(id)
            .map_err(|_| ExportError::Storage)?
            .ok_or(ExportError::NotFound)?;
        if rec.kind != KIND || rec.identity.workspace_id != self.identity.workspace_id {
            return Err(ExportError::NotFound);
        }
        let row = self
            .db
            .export_row(id)
            .map_err(stored)?
            .ok_or(ExportError::Storage)?;
        Ok((rec, row))
    }

    fn notify(&self, id: &OperationId) {
        let notifier = self.notifier.lock().ok().and_then(|n| n.clone());
        if let (Some(n), Ok((rec, row))) = (notifier, self.load(id)) {
            n(&self.result(&rec, &row));
        }
    }

    fn publication(row: &Row, revision_sha: &str, seen: &Observation) -> (Receipt, String, String) {
        (
            Receipt {
                revision_sha: revision_sha.into(),
                projection_digest: seen.digest.clone(),
                remote_ids: seen.ids.clone(),
                remote_fingerprint: seen.fingerprint.clone(),
            },
            row.destination_key.clone(),
            row.authentic_id.clone(),
        )
    }

    /// What a remote holding exactly this snapshot reports as its digest.
    fn expected(adapter: &dyn ExportAdapter, bytes: &[u8], revision_sha: &str) -> String {
        adapter
            .digest_of(bytes)
            .unwrap_or_else(|| revision_sha.to_string())
    }

    fn row_expected(adapter: &dyn ExportAdapter, row: &Row) -> String {
        match &row.snapshot {
            Some(bytes) => Self::expected(adapter, bytes, &row.revision_sha),
            None => row.revision_sha.clone(),
        }
    }

    fn why(seen: &Observation) -> Unavailable {
        seen.unavailable.unwrap_or(Unavailable::Unreadable)
    }
}

impl Exports for ExportCoordinator {
    fn available(&self, target: Target) -> bool {
        self.0.adapter(ContentKind::Authentic, target).is_ok()
            && self.0.ready.load(Ordering::SeqCst)
    }
    fn available_kind(&self, kind: ContentKind, target: Target) -> bool {
        self.0.adapter(kind, target).is_ok() && self.0.ready.load(Ordering::SeqCst)
    }
    fn start_illustrated(
        &self,
        session: &str,
        generation: u64,
        request: IllustratedStartRequest,
    ) -> Result<Started, ExportError> {
        self.0.start_illustrated(session, generation, request)
    }
    fn start(
        &self,
        session: &str,
        generation: u64,
        request: StartRequest,
    ) -> Result<Started, ExportError> {
        self.0.start(session, generation, request)
    }
    fn confirm(
        &self,
        session: &str,
        generation: u64,
        request: ConfirmRequest,
    ) -> Result<Admitted, ExportError> {
        let retry = request.clone();
        self.0
            .confirm(session, generation, request)
            .or_else(|error| {
                if !matches!(
                    error,
                    ExportError::Expired
                        | ExportError::PreviewStale
                        | ExportError::CheckpointMismatch
                ) {
                    return Err(error);
                }
                // A concurrent copy may commit after our preflight lookup, then consume the
                // token or advance the checkpoint before we validate it. Its recorded reply wins.
                let (rec, _) = self.0.load(&retry.operation_id)?;
                let hash = hash_input(
                    "export_confirm",
                    1,
                    &[
                        ("operation", &rec.input_hash),
                        ("checkpoint", &retry.expected_checkpoint.to_string()),
                    ],
                );
                match self
                    .0
                    .db
                    .export_step(&retry.operation_id, retry.step_id.as_str(), &hash)
                    .map_err(stored)?
                {
                    Some(checkpoint) => Ok(Admitted {
                        operation_id: retry.operation_id,
                        step_id: retry.step_id,
                        admitted_checkpoint: checkpoint,
                    }),
                    None => Err(error),
                }
            })
    }
    fn cancel(&self, id: &OperationId) -> Result<ExportResult, ExportError> {
        self.0.cancel(id)
    }
    fn abandon(
        &self,
        session: &str,
        generation: u64,
        id: &OperationId,
    ) -> Result<Started, ExportError> {
        self.0.abandon(session, generation, id)
    }
    fn abandon_confirm(
        &self,
        session: &str,
        generation: u64,
        request: ConfirmRequest,
    ) -> Result<ExportResult, ExportError> {
        self.0.abandon_confirm(session, generation, request)
    }
    fn operation(&self, id: &OperationId) -> Result<Option<ExportResult>, ExportError> {
        match self.0.load(id) {
            Ok((rec, row)) => Ok(Some(self.0.result(&rec, &row))),
            Err(ExportError::NotFound) => Ok(None),
            Err(e) => Err(e),
        }
    }
    fn unfinished(&self, cursor: Option<&str>, limit: usize) -> Result<ExportPage, ExportError> {
        if limit == 0 || limit > 100 {
            return Err(ExportError::Invalid);
        }
        let prefix = hash_input(
            "export_cursor",
            1,
            &[
                ("workspace", &self.0.identity.workspace_id),
                ("filter", "export:unfinished"),
            ],
        );
        let after = match cursor {
            None => String::new(),
            Some(s) => {
                let (scope, id) = s.split_once('.').ok_or(ExportError::Invalid)?;
                if scope != prefix || OperationId::parse(id).is_none() {
                    return Err(ExportError::Invalid);
                }
                id.into()
            }
        };
        let ids = self
            .0
            .db
            .export_unfinished_ids(&self.0.identity.workspace_id, &after, limit + 1)
            .map_err(stored)?;
        let next_cursor = (ids.len() > limit).then(|| format!("{prefix}.{}", ids[limit - 1]));
        let mut operations = Vec::new();
        for id in ids.into_iter().take(limit) {
            let id = OperationId::parse(&id).ok_or(ExportError::Storage)?;
            if let Some(r) = self.operation(&id)? {
                operations.push(r);
            }
        }
        Ok(ExportPage {
            operations,
            next_cursor,
        })
    }
    fn has_unfinished(&self) -> Result<bool, ExportError> {
        self.0.db.export_any_unfinished().map_err(stored)
    }
    fn set_notifier(&self, notifier: Notifier) {
        if let Ok(mut n) = self.0.notifier.lock() {
            *n = Some(notifier);
        }
    }
}

#[cfg(test)]
mod tests;

//! Export coordination service: saved-revision snapshot, durable intent before any destination
//! call, session-bound previews, token-consuming step admission and crash recovery. No real
//! destination exists yet; production registers no adapter, so every target is `disabled`.
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

#[cfg(any(test, feature = "fault-injection"))]
pub mod fake;
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
    expires: u64,
}
/// Everything the background step needs; the snapshot never leaves memory except in SQLite.
struct Job {
    id: OperationId,
    snapshot: Snapshot,
    write: Write,
    expected: Observation,
}

struct Inner {
    db: Arc<Db>,
    identity: Identity,
    authentic: Arc<dyn AuthenticStore>,
    adapter: Option<Arc<dyn ExportAdapter>>,
    previews: Mutex<BTreeMap<String, Prepared>>,
    notifier: Mutex<Option<Notifier>>,
    ready: AtomicBool,
    clock: Box<dyn Fn() -> u64 + Send + Sync>,
    #[cfg(any(test, feature = "fault-injection"))]
    fault: Mutex<Option<String>>,
}

#[derive(Clone)]
pub struct Service(Arc<Inner>);

impl Service {
    pub fn new(
        db: Arc<Db>,
        identity: Identity,
        authentic: Arc<dyn AuthenticStore>,
        adapter: Option<Arc<dyn ExportAdapter>>,
    ) -> Self {
        Self(Arc::new(Inner {
            db,
            identity,
            authentic,
            adapter,
            previews: Mutex::new(BTreeMap::new()),
            notifier: Mutex::new(None),
            ready: AtomicBool::new(false),
            clock: Box::new(monotonic),
            #[cfg(any(test, feature = "fault-injection"))]
            fault: Mutex::new(None),
        }))
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

impl Inner {
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
    fn adapter(&self, target: Target) -> Result<&Arc<dyn ExportAdapter>, ExportError> {
        self.adapter
            .as_ref()
            .filter(|a| a.target() == target)
            .ok_or(ExportError::Disabled)
    }
    fn key(&self, a: &dyn ExportAdapter) -> String {
        hash_input(
            "export_destination",
            1,
            &[
                ("target", a.target().as_str()),
                ("root", &a.root()),
                ("workspace", &self.identity.workspace_id),
            ],
        )
    }
    fn result(&self, rec: &Record, row: &Row) -> ExportResult {
        let state = match rec.state {
            OperationState::Running => ExportState::Running,
            OperationState::Paused => ExportState::NeedsConfirmation,
            OperationState::Uncertain => ExportState::Uncertain,
            OperationState::Succeeded => ExportState::Succeeded,
            OperationState::Failed | OperationState::Cancelled => ExportState::Failed,
        };
        ExportResult {
            operation_id: rec.id.clone(),
            input_hash: rec.input_hash.clone(),
            target: row.target,
            date: row.date.clone(),
            revision_sequence: row.revision_sequence,
            revision_sha: row.revision_sha.clone(),
            state,
            checkpoint: rec.checkpoint,
            reason: match state {
                ExportState::Failed => rec.result.clone(),
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
    fn snapshot(&self, request: &StartRequest) -> Result<Snapshot, ExportError> {
        match self.authentic.load(&request.date) {
            Ok(Loaded::File {
                revision,
                header,
                body,
            }) => {
                if revision != request.expected {
                    return Err(ExportError::StaleRevision);
                }
                let bytes = render(&header, &body).map_err(|_| ExportError::Storage)?;
                if bytes.len() > MAX_SNAPSHOT_BYTES {
                    return Err(ExportError::Limit);
                }
                if hex_sha(&bytes) != revision.sha256 {
                    return Err(ExportError::Storage);
                }
                Ok(Snapshot {
                    date: request.date,
                    authentic_id: header.id.clone(),
                    revision,
                    bytes,
                })
            }
            Ok(Loaded::Virtual(_)) => Err(ExportError::NothingToExport),
            Err(_) => Err(ExportError::Storage),
        }
    }
    /// A new step on an existing operation; the input hash is the operation's.
    fn step(
        &self,
        rec: &Record,
        outcome: &str,
        state: OperationState,
    ) -> Result<NewStep, ExportError> {
        Ok(NewStep {
            step_id: StepId::parse(&uuid()?).ok_or(ExportError::Storage)?,
            input_hash: rec.input_hash.clone(),
            expected_checkpoint: rec.checkpoint,
            outcome: outcome.into(),
            state: Some(state),
        })
    }
    fn park(
        &self,
        id: &OperationId,
        state: OperationState,
        outcome: &str,
        reason: Option<&str>,
    ) -> Result<(), ExportError> {
        let rec = self
            .db
            .get(id)
            .map_err(|_| ExportError::Storage)?
            .ok_or(ExportError::NotFound)?;
        let step = self.step(&rec, outcome, state)?;
        self.db.park_export(id, &step, reason).map_err(stored)?;
        Ok(())
    }
    fn issue_preview(
        &self,
        session: &str,
        generation: u64,
        id: &OperationId,
        checkpoint: u64,
        observed: &Observation,
        (reason, write): (Reason, Write),
    ) -> Result<Preview, ExportError> {
        let token = format!("{}{}", uuid()?, uuid()?);
        let now = (self.clock)();
        let mut previews = self.previews.lock().map_err(|_| ExportError::Storage)?;
        previews
            .retain(|_, p| p.session == session && p.generation == generation && p.expires > now);
        if previews.len() >= MAX_PREVIEWS {
            return Err(ExportError::Limit);
        }
        previews.insert(
            token.clone(),
            Prepared {
                operation: id.clone(),
                session: session.into(),
                generation,
                checkpoint,
                fingerprint: observed.fingerprint.clone(),
                write,
                expires: now.saturating_add(TOKEN_TTL_MS),
            },
        );
        Ok(Preview {
            operation_id: id.clone(),
            expected_checkpoint: checkpoint,
            token,
            observed_fingerprint: observed.fingerprint.clone(),
            reason,
            write,
            expires_in_ms: TOKEN_TTL_MS,
        })
    }
    fn publication(row: &Row, revision_sha: &str, seen: &Observation) -> (Receipt, String, String) {
        (
            Receipt {
                revision_sha: revision_sha.into(),
                projection_digest: revision_sha.into(),
                remote_ids: seen.ids.clone(),
                remote_fingerprint: seen.fingerprint.clone(),
            },
            row.destination_key.clone(),
            row.authentic_id.clone(),
        )
    }
    fn succeed(&self, id: &OperationId, row: &Row, seen: &Observation) -> Result<(), ExportError> {
        let (receipt, key, authentic_id) = Self::publication(row, &row.revision_sha, seen);
        self.db
            .finish_export(
                id,
                OperationState::Succeeded,
                "export",
                Some(Publication {
                    destination_key: &key,
                    authentic_id: &authentic_id,
                    date: &row.date,
                    receipt: &receipt,
                }),
            )
            .map_err(stored)
    }

    fn holder_blocks(
        &self,
        key: &str,
        own: Option<&OperationId>,
    ) -> Result<(bool, bool), ExportError> {
        // (unresolved, reserved by someone else)
        match self.db.export_holder(key).map_err(stored)? {
            Some((id, _)) if own == Some(&id) => Ok((false, false)),
            Some((_, state)) => Ok((state == OperationState::Uncertain, true)),
            None => Ok((false, false)),
        }
    }

    fn start(
        self: &Arc<Self>,
        session: &str,
        generation: u64,
        request: StartRequest,
    ) -> Result<Started, ExportError> {
        let adapter = self.adapter(request.target)?.clone();
        if !self.ready.load(Ordering::SeqCst) {
            return Err(ExportError::Storage);
        }
        if session.is_empty() || session.len() > 128 {
            return Err(ExportError::Invalid);
        }
        let key = self.key(&*adapter);
        let date = request.date.to_string();
        let hash = hash_input(
            KIND,
            1,
            &[
                ("date", &date),
                ("target", request.target.as_str()),
                ("destination", &key),
                ("sequence", &request.expected.sequence.to_string()),
                ("sha", &request.expected.sha256),
            ],
        );
        // The recorded outcome of a retry wins over every new-action precondition.
        if let Some(rec) = self
            .db
            .get(&request.operation_id)
            .map_err(|_| ExportError::Storage)?
        {
            if rec.kind != KIND || rec.hash_version != 1 || rec.input_hash != hash {
                return Err(ExportError::Mismatch);
            }
            let (rec, row) = self.load(&request.operation_id)?;
            if rec.state == OperationState::Paused {
                return self.repreview(session, generation, &rec, &row, &*adapter);
            }
            return Ok(Started::Result(self.result(&rec, &row)));
        }
        let snapshot = self.snapshot(&request)?;
        self.hit("before_intent")?;
        let first = NewStep {
            step_id: StepId::parse(&uuid()?).ok_or(ExportError::Storage)?,
            input_hash: hash.clone(),
            expected_checkpoint: 0,
            outcome: "admitted".into(),
            state: None,
        };
        let op = NewOperation {
            id: request.operation_id.clone(),
            kind: KIND.into(),
            hash_version: 1,
            input_hash: hash,
            identity: self.identity.clone(),
            // Even observation and no-op verification must serialize with other exports.
            resources: vec![
                ResourceKey::parse(&format!("export:{key}")).ok_or(ExportError::Storage)?,
            ],
            first_step: Some(first),
        };
        let intent = Intent {
            target: request.target,
            destination_key: key.clone(),
            authentic_id: snapshot.authentic_id.clone(),
            date: date.clone(),
            revision_sequence: snapshot.revision.sequence,
            revision_sha: snapshot.revision.sha256.clone(),
            snapshot: snapshot.bytes.clone(),
            reason: None,
        };
        match self.db.admit_export(&op, &intent) {
            Ok(Admission::New(_)) => {}
            Ok(Admission::Existing(_)) => {
                let (rec, row) = self.load(&request.operation_id)?;
                return Ok(Started::Result(self.result(&rec, &row)));
            }
            Ok(Admission::Mismatch) => return Err(ExportError::Mismatch),
            Err(StoreError::Ledger(LedgerError::Reserved(holder))) => {
                let uncertain = matches!(
                    self.db.get(&holder),
                    Ok(Some(r)) if r.state == OperationState::Uncertain
                );
                return Err(if uncertain {
                    ExportError::Uncertain
                } else {
                    ExportError::Busy
                });
            }
            Err(e) => return Err(stored(e)),
        }
        self.hit("after_intent")?;
        let seen = adapter.observe(&request.date);
        let receipt = self
            .db
            .export_receipt(&key, &snapshot.authentic_id)
            .map_err(stored)?;
        let (confirm, write) =
            match decide(receipt.as_ref(), &snapshot.revision.sha256, &seen, false) {
                Decision::Unavailable => {
                    self.db
                        .finish_export(
                            &request.operation_id,
                            OperationState::Failed,
                            "cloud_unavailable",
                            None,
                        )
                        .map_err(stored)?;
                    self.notify(&request.operation_id);
                    return Err(ExportError::CloudUnavailable);
                }
                Decision::Uncertain => return Err(ExportError::Uncertain),
                Decision::Noop => (None, None),
                Decision::Write(w) => (None, Some(w)),
                Decision::Confirm(r, w) => (Some(r), Some(w)),
            };
        let (_, row) = self.load(&request.operation_id)?;
        match (confirm, write) {
            (None, None) => {
                self.succeed(&request.operation_id, &row, &seen)?;
            }
            (Some(reason), Some(write)) => {
                self.park(
                    &request.operation_id,
                    OperationState::Paused,
                    "needs_confirmation",
                    Some(reason.as_str()),
                )?;
                let (rec, _) = self.load(&request.operation_id)?;
                let preview = self.issue_preview(
                    session,
                    generation,
                    &request.operation_id,
                    rec.checkpoint,
                    &seen,
                    (reason, write),
                )?;
                self.notify(&request.operation_id);
                return Ok(Started::Preview(preview));
            }
            (None, Some(write)) => {
                self.spawn(Job {
                    id: request.operation_id.clone(),
                    snapshot,
                    write,
                    expected: seen,
                });
            }
            (Some(_), None) => return Err(ExportError::Storage),
        }
        let (rec, row) = self.load(&request.operation_id)?;
        self.notify(&request.operation_id);
        Ok(Started::Result(self.result(&rec, &row)))
    }

    /// A fresh preview for a paused operation: re-observe, then authorize only what is seen now.
    fn repreview(
        self: &Arc<Self>,
        session: &str,
        generation: u64,
        rec: &Record,
        row: &Row,
        adapter: &dyn ExportAdapter,
    ) -> Result<Started, ExportError> {
        if self.key(adapter) != row.destination_key {
            return Err(ExportError::Disabled);
        }
        let date = DiaryDate::parse(&row.date).ok_or(ExportError::Storage)?;
        let seen = adapter.observe(&date);
        let receipt = self
            .db
            .export_receipt(&row.destination_key, &row.authentic_id)
            .map_err(stored)?;
        let (unresolved, _) = self.holder_blocks(&row.destination_key, Some(&rec.id))?;
        match decide(receipt.as_ref(), &row.revision_sha, &seen, unresolved) {
            Decision::Unavailable => Err(ExportError::CloudUnavailable),
            Decision::Uncertain => Err(ExportError::Uncertain),
            Decision::Noop => {
                self.succeed(&rec.id, row, &seen)?;
                self.notify(&rec.id);
                let (rec, row) = self.load(&rec.id)?;
                Ok(Started::Result(self.result(&rec, &row)))
            }
            Decision::Write(w) | Decision::Confirm(_, w) => {
                let reason = match decide(receipt.as_ref(), &row.revision_sha, &seen, false) {
                    Decision::Confirm(r, _) => r,
                    _ => Reason::Changed,
                };
                Ok(Started::Preview(self.issue_preview(
                    session,
                    generation,
                    &rec.id,
                    rec.checkpoint,
                    &seen,
                    (reason, w),
                )?))
            }
        }
    }

    fn confirm(
        self: &Arc<Self>,
        session: &str,
        generation: u64,
        request: ConfirmRequest,
    ) -> Result<Admitted, ExportError> {
        if request.expected_checkpoint == 0
            || request.expected_checkpoint > i64::MAX as u64 - 1
            || request.token.is_empty()
            || request.token.len() > 128
        {
            return Err(ExportError::Invalid);
        }
        let (rec, row) = self.load(&request.operation_id)?;
        let hash = hash_input(
            "export_confirm",
            1,
            &[
                ("operation", &rec.input_hash),
                ("checkpoint", &request.expected_checkpoint.to_string()),
            ],
        );
        // A duplicate answers from the recorded step, before token, expiry or checkpoint checks.
        if let Some(checkpoint) = self
            .db
            .export_step(&request.operation_id, request.step_id.as_str(), &hash)
            .map_err(stored)?
        {
            return Ok(Admitted {
                operation_id: request.operation_id,
                step_id: request.step_id,
                admitted_checkpoint: checkpoint,
            });
        }
        let adapter = self.adapter(row.target)?.clone();
        if self.key(&*adapter) != row.destination_key {
            return Err(ExportError::Disabled);
        }
        if !self.ready.load(Ordering::SeqCst) {
            return Err(ExportError::Storage);
        }
        match rec.state {
            OperationState::Uncertain => return Err(ExportError::Uncertain),
            OperationState::Paused if rec.checkpoint == request.expected_checkpoint => {}
            _ => return Err(ExportError::CheckpointMismatch),
        }
        let prepared = {
            let previews = self.previews.lock().map_err(|_| ExportError::Storage)?;
            let p = previews.get(&request.token).ok_or(ExportError::Expired)?;
            if p.expires <= (self.clock)() {
                return Err(ExportError::Expired);
            }
            if p.session != session
                || p.generation != generation
                || p.operation != request.operation_id
                || p.checkpoint != request.expected_checkpoint
            {
                return Err(ExportError::PreviewStale);
            }
            (p.fingerprint.clone(), p.write)
        };
        let date = DiaryDate::parse(&row.date).ok_or(ExportError::Storage)?;
        let seen = adapter.observe(&date);
        if !seen.available {
            return Err(ExportError::CloudUnavailable);
        }
        if seen.fingerprint != prepared.0 {
            return Err(ExportError::PreviewStale);
        }
        let snapshot_bytes = row.snapshot.clone().ok_or(ExportError::Storage)?;
        let step = NewStep {
            step_id: request.step_id.clone(),
            input_hash: hash,
            expected_checkpoint: request.expected_checkpoint,
            outcome: format!("admitted:{}", request.expected_checkpoint + 1),
            state: Some(OperationState::Running),
        };
        let fingerprint = hex_sha(request.token.as_bytes());
        let admission = self
            .db
            .confirm_export(&request.operation_id, &step, &fingerprint)
            .map_err(stored)?;
        if let StepOutcome::Recorded { checkpoint, .. } = admission {
            return Ok(Admitted {
                operation_id: request.operation_id,
                step_id: request.step_id,
                admitted_checkpoint: checkpoint,
            });
        }
        if let Ok(mut previews) = self.previews.lock() {
            previews.remove(&request.token);
        }
        self.notify(&request.operation_id);
        self.spawn(Job {
            id: request.operation_id.clone(),
            snapshot: Snapshot {
                date,
                authentic_id: row.authentic_id.clone(),
                revision: diary_domain::authentic::AuthenticRevision {
                    sequence: row.revision_sequence,
                    sha256: row.revision_sha.clone(),
                },
                bytes: snapshot_bytes,
            },
            write: prepared.1,
            expected: seen,
        });
        Ok(Admitted {
            operation_id: request.operation_id,
            step_id: request.step_id,
            admitted_checkpoint: request.expected_checkpoint + 1,
        })
    }

    fn spawn(self: &Arc<Self>, job: Job) {
        let inner = self.clone();
        std::thread::spawn(move || {
            // An error leaves the durable state for recovery; nothing is replayed here.
            let _ = inner.run(&job);
            inner.notify(&job.id);
        });
    }

    fn run(&self, job: &Job) -> Result<(), ExportError> {
        self.hit("after_admission")?;
        let adapter = self.adapter.as_ref().ok_or(ExportError::Disabled)?.clone();
        let date = job.snapshot.date;
        // Re-read immediately before mutation; a changed observation needs a new preview.
        let now = adapter.observe(&date);
        if !now.available
            || now.present != job.expected.present
            || now.fingerprint != job.expected.fingerprint
        {
            return self.park(
                &job.id,
                OperationState::Paused,
                "parked",
                Some(if now.available {
                    "changed"
                } else {
                    "unavailable"
                }),
            );
        }
        self.db
            .export_chunk(&job.id, "applying", None)
            .map_err(stored)?;
        self.hit("before_apply")?;
        let applied = adapter.apply(&date, &job.snapshot.bytes, job.write);
        self.hit("after_apply")?;
        if applied == Applied::Failed {
            self.db
                .export_chunk(&job.id, "failed", None)
                .map_err(stored)?;
            return self
                .db
                .finish_export(&job.id, OperationState::Failed, "apply_failed", None)
                .map_err(stored);
        }
        let back = adapter.read_back(&date);
        let sha = &job.snapshot.revision.sha256;
        if applied == Applied::Done && back.available && back.present && &back.digest == sha
            || applied == Applied::Unknown && back.available && back.present && &back.digest == sha
        {
            self.hit("before_receipt")?;
            let (_, row) = self.load(&job.id)?;
            self.db
                .export_chunk(&job.id, "done", Some(&back.ids))
                .map_err(stored)?;
            return self.succeed(&job.id, &row, &back);
        }
        // Ambiguous: no definitive evidence either way. Never replay.
        self.db
            .export_chunk(&job.id, "unknown", None)
            .map_err(stored)?;
        self.park(
            &job.id,
            OperationState::Uncertain,
            "uncertain",
            Some("uncertain"),
        )
    }

    fn recover(&self) -> Result<usize, ExportError> {
        self.ready.store(false, Ordering::SeqCst);
        let mut count = 0;
        for rec in self.db.unfinished().map_err(|_| ExportError::Storage)? {
            if rec.kind != KIND || rec.state != OperationState::Running {
                continue;
            }
            let row = self
                .db
                .export_row(&rec.id)
                .map_err(stored)?
                .ok_or(ExportError::Storage)?;
            let chunk = self.db.export_chunk_status(&rec.id).map_err(stored)?;
            if chunk.as_deref() == Some("pending") {
                // The side effect never started: ask for a fresh confirmation.
                self.park(
                    &rec.id,
                    OperationState::Paused,
                    "recovered",
                    Some("interrupted"),
                )?;
            } else {
                let proven = self
                    .adapter
                    .as_ref()
                    .filter(|a| a.target() == row.target && self.key(&***a) == row.destination_key)
                    .and_then(|a| {
                        let date = DiaryDate::parse(&row.date)?;
                        let back = a.read_back(&date);
                        (back.available && back.present && back.digest == row.revision_sha)
                            .then_some(back)
                    });
                match proven {
                    Some(back) => self.succeed(&rec.id, &row, &back)?,
                    None => self.park(
                        &rec.id,
                        OperationState::Uncertain,
                        "recovered",
                        Some("uncertain"),
                    )?,
                }
            }
            count += 1;
        }
        self.ready.store(true, Ordering::SeqCst);
        Ok(count)
    }
}

impl Exports for Service {
    fn available(&self, target: Target) -> bool {
        self.0.adapter(target).is_ok() && self.0.ready.load(Ordering::SeqCst)
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

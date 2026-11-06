//! Session-bound previews and durable, ordered execution. No bearer credential is
//! persisted. All new side effects require explicit confirmation of a fresh plan.
use super::{
    native::{self, MoveResult},
    store::{MemberRecord, MemberStatus, StoreError, StoredPlan, progress::continuation_ordinals},
    *,
};
use crate::{
    ledger::hash_input,
    lifecycle::random_uuid,
    sqlite::{Db, DbError},
};
use diary_application::{operations::*, source_trash::*};
use rusqlite::OptionalExtension;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

const PREVIEW_MS: u64 = 60_000;
const MAX_PREVIEWS: usize = 64;
struct Prepared {
    plan: StoredPlan,
    session: String,
    generation: u64,
    action: DeleteAction,
    operation_id: Option<OperationId>,
    checkpoint: u64,
    ordinals: Vec<u8>,
    expires: u64,
}
pub struct Service {
    db: Arc<Db>,
    identity: Identity,
    sources: Vec<Source>,
    previews: Mutex<BTreeMap<String, Prepared>>,
    enabled: bool,
    ready: AtomicBool,
    clock: Box<dyn Fn() -> u64 + Send + Sync>,
    #[cfg(test)]
    mover: Option<Box<TestMover>>,
}
#[cfg(test)]
type TestMover = dyn Fn(&ResolvedRoot, &diary_domain::source::path::SourcePath, &FileDigest) -> MoveResult
    + Send
    + Sync;

fn now() -> u64 {
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
fn fingerprint(token: &str) -> String {
    Sha256::digest(token.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn prep(e: PreparationError) -> DeleteError {
    match e {
        PreparationError::Unsupported | PreparationError::UnsafePath => DeleteError::Unsupported,
        PreparationError::Unavailable => DeleteError::CloudUnavailable,
        PreparationError::Changed => DeleteError::Changed,
        PreparationError::OwnershipAmbiguous => DeleteError::OwnershipAmbiguous,
        PreparationError::Limit => DeleteError::Limit,
        PreparationError::Cancelled => DeleteError::PreviewStale,
        PreparationError::RootChanged => DeleteError::SourceStale,
    }
}
fn stored(e: StoreError) -> DeleteError {
    match e {
        StoreError::Invalid => DeleteError::Invalid,
        StoreError::TokenUsed => DeleteError::Expired,
        StoreError::Ledger(LedgerError::Reserved(_)) => DeleteError::Busy,
        StoreError::Ledger(LedgerError::StepMismatch) => DeleteError::Mismatch,
        StoreError::Ledger(LedgerError::CheckpointMismatch { .. }) => {
            DeleteError::CheckpointMismatch
        }
        StoreError::Ledger(LedgerError::NotFound) => DeleteError::NotFound,
        _ => DeleteError::Storage,
    }
}
fn reason(e: PreparationError) -> &'static str {
    match e {
        PreparationError::Changed => "changed",
        PreparationError::UnsafePath | PreparationError::Unsupported => "unsafe_path",
        PreparationError::OwnershipAmbiguous => "ownership_ambiguous",
        PreparationError::RootChanged => "root_changed",
        _ => "unavailable",
    }
}
fn hash(request: &DeleteConfirmRequest) -> Result<String, DeleteError> {
    if request.plan_hash.len() != 64
        || !request
            .plan_hash
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        || request.ordinals.is_empty()
        || request.ordinals.len() > 2
        || !request.ordinals.windows(2).all(|w| w[0] < w[1])
        || request.ordinals.iter().any(|o| *o > 1)
        || request.expected_checkpoint > i64::MAX as u64 - 1
        || (request.action == DeleteAction::Initial
            && (request.expected_checkpoint != 0 || request.ordinals != [0, 1]))
        || (request.action == DeleteAction::Continuation && request.expected_checkpoint == 0)
    {
        return Err(DeleteError::Invalid);
    }
    let ordinals = request
        .ordinals
        .iter()
        .map(u8::to_string)
        .collect::<Vec<_>>()
        .join(",");
    Ok(hash_input(
        "source_trash",
        1,
        &[
            (
                "action",
                if request.action == DeleteAction::Initial {
                    "initial"
                } else {
                    "continuation"
                },
            ),
            ("source_id", &request.source_id),
            ("plan_hash", &request.plan_hash),
            ("checkpoint", &request.expected_checkpoint.to_string()),
            ("ordinals", &ordinals),
        ],
    ))
}

impl Service {
    pub fn new(db: Arc<Db>, identity: Identity, sources: Vec<Source>) -> Self {
        Self {
            db,
            identity,
            sources,
            previews: Mutex::new(BTreeMap::new()),
            enabled: TRASH_PAIR_VERIFIED,
            ready: AtomicBool::new(false),
            clock: Box::new(now),
            #[cfg(test)]
            mover: None,
        }
    }
    fn load(
        &self,
        id: &OperationId,
    ) -> Result<(Record, StoredPlan, Vec<MemberRecord>), DeleteError> {
        let op = self
            .db
            .get(id)
            .map_err(|_| DeleteError::Storage)?
            .ok_or(DeleteError::NotFound)?;
        if op.kind != "source_trash" || op.identity.workspace_id != self.identity.workspace_id {
            return Err(DeleteError::NotFound);
        }
        let (plan, members) = self
            .db
            .trash_plan(id)
            .map_err(stored)?
            .ok_or(DeleteError::Storage)?;
        if members.iter().any(|m| m.checkpoint > op.checkpoint) {
            return Err(DeleteError::Storage);
        }
        Ok((op, plan, members))
    }
    fn initial(&self, id: &str, cancelled: &dyn Fn() -> bool) -> Result<StoredPlan, DeleteError> {
        let binding: Option<(Vec<u8>, String)> = self
            .db
            .with(|c| {
                c.query_row(
                    "SELECT namespace,path_key FROM source_identities WHERE source_id=?1",
                    [id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()
                .map_err(|_| DbError::Storage)
            })
            .map_err(|_| DeleteError::Storage)?;
        let (namespace, key) = binding.ok_or(DeleteError::SourceStale)?;
        let roots = resolve_roots(&self.sources);
        for (index, root) in roots.into_iter().enumerate() {
            let root = root.map_err(|_| DeleteError::SourceStale)?;
            if Sha256::digest(
                root.canonical
                    .to_str()
                    .ok_or(DeleteError::SourceStale)?
                    .as_bytes(),
            )
            .as_slice()
                != namespace
            {
                continue;
            }
            let path = normalize(&key, root.insensitive).map_err(|_| DeleteError::SourceStale)?;
            if super::super::root::source_id(&root, &path) != id {
                return Err(DeleteError::SourceStale);
            }
            return Ok(StoredPlan::from_pair(
                id,
                &prepare_pair(&self.sources, index, &path.spelling, cancelled).map_err(prep)?,
            ));
        }
        Err(DeleteError::SourceStale)
    }
    fn public(
        &self,
        op: Record,
        plan: &StoredPlan,
        members: &[MemberRecord],
    ) -> Result<DeleteResult, DeleteError> {
        let state = match op.state {
            OperationState::Running => DeleteState::Running,
            OperationState::Paused => DeleteState::NeedsConfirmation,
            OperationState::Uncertain => DeleteState::Uncertain,
            OperationState::Succeeded => DeleteState::Succeeded,
            _ => return Err(DeleteError::Storage),
        };
        let eligible = if state == DeleteState::NeedsConfirmation {
            continuation_ordinals(members).unwrap_or_default()
        } else {
            vec![]
        };
        let members = members
            .iter()
            .map(|m| DeleteMember {
                ordinal: m.ordinal,
                relative_label: if m.ordinal == 0 {
                    plan.markdown_rel.clone()
                } else {
                    plan.audio_rel.clone()
                },
                state: match m.status {
                    MemberStatus::Pending | MemberStatus::Intended => DeleteMemberState::Pending,
                    MemberStatus::Moved => DeleteMemberState::Moved,
                    MemberStatus::Missing => DeleteMemberState::Missing,
                    MemberStatus::Failed => DeleteMemberState::Failed,
                    MemberStatus::Uncertain => DeleteMemberState::Uncertain,
                },
                reason: m.reason.as_deref().and_then(|r| {
                    [
                        "unavailable",
                        "changed",
                        "unsafe_path",
                        "ownership_ambiguous",
                        "root_changed",
                        "uncertain",
                    ]
                    .into_iter()
                    .find(|s| *s == r)
                }),
                retry_eligible: eligible.contains(&m.ordinal),
            })
            .collect();
        Ok(DeleteResult {
            operation_id: op.id,
            source_id: plan.source_id.clone(),
            plan_hash: plan.hash().map_err(stored)?,
            checkpoint: op.checkpoint,
            state,
            members,
        })
    }
    fn execute(
        &self,
        id: &OperationId,
        plan: &StoredPlan,
        ordinals: &[u8],
        checkpoint: u64,
    ) -> Result<(), DeleteError> {
        for ordinal in ordinals {
            let (_, _, members) = self.load(id)?;
            if matches!(
                members[*ordinal as usize].status,
                MemberStatus::Moved | MemberStatus::Missing
            ) {
                continue;
            }
            let moved = members[0].status == MemberStatus::Moved;
            let remaining: Vec<_> = ordinals
                .iter()
                .copied()
                .filter(|o| {
                    !matches!(
                        members[*o as usize].status,
                        MemberStatus::Moved | MemberStatus::Missing
                    )
                })
                .collect();
            // Admission survives a client disconnect. Do not use session cancellation to
            // abandon an admitted operation; all further checks bind its original plan.
            let started = std::time::Instant::now();
            let stop = || started.elapsed() >= Duration::from_secs(120);
            if let Err(e) = validate_plan(&self.sources, plan, &remaining, moved, &stop) {
                self.db
                    .record_trash_member(
                        id,
                        *ordinal,
                        checkpoint,
                        MemberStatus::Failed,
                        None,
                        Some(reason(e)),
                    )
                    .map_err(stored)?;
                return Ok(());
            }
            super::fault::hit("before_intent", Some(*ordinal))?;
            self.db
                .intend_trash_member(id, *ordinal, checkpoint)
                .map_err(stored)?;
            super::fault::hit("after_intent", Some(*ordinal))?;
            let root = plan.resolved_root();
            let rel = normalize(
                if *ordinal == 0 {
                    &plan.markdown_rel
                } else {
                    &plan.audio_rel
                },
                root.insensitive,
            )
            .map_err(|_| DeleteError::Storage)?;
            let expected = if *ordinal == 0 {
                &plan.markdown
            } else {
                plan.audio.as_ref().ok_or(DeleteError::Storage)?
            };
            let check = || validate_plan(&self.sources, plan, &remaining, moved, &stop);
            super::fault::hit("before_native", Some(*ordinal))?;
            #[cfg(test)]
            let result = if super::fault::native_failure(*ordinal) {
                MoveResult::Failed(PreparationError::Unavailable)
            } else if let Some(mover) = &self.mover {
                match check() {
                    Ok(()) => mover(&root, &rel, expected),
                    Err(e) => MoveResult::Failed(e),
                }
            } else {
                native::move_member(&root, &rel, expected, check)
            };
            #[cfg(not(test))]
            let result = if super::fault::native_failure(*ordinal) {
                MoveResult::Failed(PreparationError::Unavailable)
            } else {
                native::move_member(&root, &rel, expected, check)
            };
            #[cfg(all(test, feature = "fault-injection"))]
            native_harness::retain_receipt(&result, expected, &root, &rel)?;
            super::fault::hit("after_native", Some(*ordinal))?;
            let (status, receipt, why) = match result {
                MoveResult::Moved(r) => (MemberStatus::Moved, Some(r), None),
                MoveResult::Failed(e) => (MemberStatus::Failed, None, Some(reason(e))),
                MoveResult::Uncertain { receipt } => {
                    (MemberStatus::Uncertain, receipt, Some("uncertain"))
                }
            };
            super::fault::hit("before_receipt", Some(*ordinal))?;
            self.db
                .record_trash_member(id, *ordinal, checkpoint, status, receipt.as_ref(), why)
                .map_err(stored)?;
            super::fault::hit("after_receipt", Some(*ordinal))?;
            if status != MemberStatus::Moved {
                break;
            }
        }
        Ok(())
    }
    /// Reconcile before exposing mutation readiness, independently of current source
    /// config. No native call is repeated during startup.
    pub fn recover(&self) -> Result<usize, DeleteError> {
        self.ready.store(false, Ordering::SeqCst);
        let _guard = self
            .db
            .source_mutations
            .lock()
            .map_err(|_| DeleteError::Storage)?;
        let mut count = 0;
        for op in self.db.unfinished().map_err(|_| DeleteError::Storage)? {
            if op.kind != "source_trash" {
                continue;
            }
            let (_, plan, members) = self.load(&op.id)?;
            // Retain the persisted root identity; never reinterpret its relative paths
            // through a newly configured root. Recovery does not require enablement.
            let root_available = op.identity == self.identity
                && super::super::read::validated_root(&plan.resolved_root()).is_ok();
            let proven: Vec<_> = members
                .iter()
                .filter(|m| matches!(m.status, MemberStatus::Intended | MemberStatus::Uncertain))
                .filter_map(|m| {
                    let original = if m.ordinal == 0 {
                        Some(&plan.markdown)
                    } else {
                        plan.audio.as_ref()
                    };
                    m.receipt
                        .as_ref()
                        .zip(original)
                        .filter(|(receipt, original)| native::verify_receipt(receipt, original))
                        .map(|_| m.ordinal)
                })
                .collect();
            self.db
                .recover_trash_with_proof(&op.id, &proven, !root_available)
                .map_err(stored)?;
            count += 1;
        }
        self.ready.store(true, Ordering::SeqCst);
        Ok(count)
    }
}

impl SourceTrash for Service {
    fn available(&self) -> bool {
        self.enabled && self.ready.load(Ordering::SeqCst)
    }
    fn preview(
        &self,
        session: &str,
        generation: u64,
        request: DeletePreviewRequest,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<DeletePreview, DeleteError> {
        let started = std::time::Instant::now();
        let cancelled = || cancelled() || started.elapsed() >= Duration::from_secs(120);
        if !self.ready.load(Ordering::SeqCst) {
            return Err(DeleteError::Storage);
        }
        if !self.enabled {
            return Err(DeleteError::Unsupported);
        }
        if session.is_empty() || session.len() > 128 {
            return Err(DeleteError::Invalid);
        }
        let (plan, ordinals, members) = match request.action {
            DeleteAction::Initial => {
                if request.operation_id.is_some() || request.expected_checkpoint != 0 {
                    return Err(DeleteError::Invalid);
                }
                let plan = self.initial(&request.source_id, &cancelled).map_err(|e| {
                    if cancelled() {
                        DeleteError::PreviewStale
                    } else {
                        e
                    }
                })?;
                let members = vec![
                    MemberRecord {
                        ordinal: 0,
                        status: MemberStatus::Pending,
                        receipt: None,
                        reason: None,
                        checkpoint: 1,
                    },
                    MemberRecord {
                        ordinal: 1,
                        status: if plan.audio.is_some() {
                            MemberStatus::Pending
                        } else {
                            MemberStatus::Missing
                        },
                        receipt: None,
                        reason: None,
                        checkpoint: 1,
                    },
                ];
                (plan, vec![0, 1], members)
            }
            DeleteAction::Continuation => {
                let id = request.operation_id.as_ref().ok_or(DeleteError::Invalid)?;
                let (op, plan, members) = self.load(id)?;
                if op.state == OperationState::Uncertain {
                    return Err(DeleteError::Uncertain);
                }
                if op.state != OperationState::Paused
                    || op.checkpoint != request.expected_checkpoint
                {
                    return Err(DeleteError::CheckpointMismatch);
                }
                if plan.source_id != request.source_id {
                    return Err(DeleteError::Mismatch);
                }
                let ordinals = continuation_ordinals(&members).map_err(stored)?;
                validate_plan(
                    &self.sources,
                    &plan,
                    &ordinals,
                    members[0].status == MemberStatus::Moved,
                    &cancelled,
                )
                .map_err(prep)?;
                (plan, ordinals, members)
            }
        };
        if cancelled() {
            return Err(DeleteError::PreviewStale);
        }
        let plan_hash = plan.hash().map_err(stored)?;
        let token = format!(
            "{}{}",
            random_uuid().map_err(|_| DeleteError::Storage)?,
            random_uuid().map_err(|_| DeleteError::Storage)?
        );
        let now = (self.clock)();
        let public_members = members
            .iter()
            .map(|m| DeleteMember {
                ordinal: m.ordinal,
                relative_label: if m.ordinal == 0 {
                    plan.markdown_rel.clone()
                } else {
                    plan.audio_rel.clone()
                },
                state: match m.status {
                    MemberStatus::Missing => DeleteMemberState::Missing,
                    MemberStatus::Moved => DeleteMemberState::Moved,
                    MemberStatus::Failed => DeleteMemberState::Failed,
                    _ => DeleteMemberState::Pending,
                },
                reason: None,
                retry_eligible: ordinals.contains(&m.ordinal)
                    && !matches!(
                        m.status,
                        MemberStatus::Moved | MemberStatus::Missing | MemberStatus::Uncertain
                    ),
            })
            .collect();
        let mut previews = self.previews.lock().map_err(|_| DeleteError::Storage)?;
        previews
            .retain(|_, p| p.session == session && p.generation == generation && p.expires > now);
        if previews.len() >= MAX_PREVIEWS {
            return Err(DeleteError::Limit);
        }
        previews.insert(
            token.clone(),
            Prepared {
                plan,
                session: session.into(),
                generation,
                action: request.action,
                operation_id: request.operation_id,
                checkpoint: request.expected_checkpoint,
                ordinals: ordinals.clone(),
                expires: now.saturating_add(PREVIEW_MS),
            },
        );
        Ok(DeletePreview {
            token,
            plan_hash,
            expected_checkpoint: request.expected_checkpoint,
            expires_in_ms: PREVIEW_MS,
            ordinals,
            members: public_members,
        })
    }
    fn confirm(
        &self,
        session: &str,
        generation: u64,
        request: DeleteConfirmRequest,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<DeleteAcknowledged, DeleteError> {
        let input_hash = hash(&request)?;
        let _guard = self
            .db
            .source_mutations
            .lock()
            .map_err(|_| DeleteError::Storage)?;
        if let Some(op) = self
            .db
            .get(&request.operation_id)
            .map_err(|_| DeleteError::Storage)?
        {
            if op.kind != "source_trash" || op.identity.workspace_id != self.identity.workspace_id {
                return Err(DeleteError::Mismatch);
            }
            if let Some(checkpoint) = self
                .db
                .trash_step(
                    &request.operation_id,
                    &request.step_id,
                    &input_hash,
                    request.expected_checkpoint,
                )
                .map_err(stored)?
            {
                return Ok(DeleteAcknowledged {
                    operation_id: request.operation_id,
                    step_id: request.step_id,
                    admitted_checkpoint: checkpoint,
                });
            }
            if request.action == DeleteAction::Initial {
                return Err(DeleteError::Mismatch);
            }
        } else if request.action == DeleteAction::Continuation {
            return Err(DeleteError::NotFound);
        }
        if !self.ready.load(Ordering::SeqCst) {
            return Err(DeleteError::Storage);
        }
        if !self.enabled {
            return Err(DeleteError::Unsupported);
        }
        let mut previews = self.previews.lock().map_err(|_| DeleteError::Storage)?;
        let prepared = previews.get(&request.token).ok_or(DeleteError::Expired)?;
        if prepared.expires <= (self.clock)() {
            return Err(DeleteError::Expired);
        }
        if prepared.session != session
            || prepared.generation != generation
            || prepared.action != request.action
            || prepared.checkpoint != request.expected_checkpoint
            || prepared.plan.source_id != request.source_id
            || prepared.plan.hash().map_err(stored)? != request.plan_hash
            || prepared.ordinals != request.ordinals
            || (request.action == DeleteAction::Continuation
                && prepared.operation_id.as_ref() != Some(&request.operation_id))
        {
            return Err(DeleteError::PreviewStale);
        }
        let markdown_moved = if request.action == DeleteAction::Continuation {
            let (op, _, members) = self.load(&request.operation_id)?;
            if op.state == OperationState::Uncertain {
                return Err(DeleteError::Uncertain);
            }
            if op.identity != self.identity {
                return Err(DeleteError::SourceStale);
            }
            if op.checkpoint != request.expected_checkpoint {
                return Err(DeleteError::CheckpointMismatch);
            }
            members[0].status == MemberStatus::Moved
        } else {
            false
        };
        validate_plan(
            &self.sources,
            &prepared.plan,
            &request.ordinals,
            markdown_moved,
            cancelled,
        )
        .map_err(prep)?;
        if cancelled() {
            return Err(DeleteError::PreviewStale);
        }
        // Preparation time cannot stretch the confirmation window.
        if prepared.expires <= (self.clock)() {
            return Err(DeleteError::Expired);
        }
        let plan = prepared.plan.clone();
        let step = NewStep {
            step_id: request.step_id.clone(),
            input_hash: input_hash.clone(),
            expected_checkpoint: request.expected_checkpoint,
            outcome: format!("admitted:{}", request.expected_checkpoint + 1),
            state: if request.action == DeleteAction::Continuation {
                Some(OperationState::Running)
            } else {
                None
            },
        };
        if request.action == DeleteAction::Initial {
            let admission = self
                .db
                .admit_trash(
                    &NewOperation {
                        id: request.operation_id.clone(),
                        kind: "source_trash".into(),
                        hash_version: 1,
                        input_hash,
                        resources: plan.resources().map_err(stored)?,
                        identity: self.identity.clone(),
                        first_step: Some(step),
                    },
                    &plan,
                    &fingerprint(&request.token),
                )
                .map_err(stored)?;
            if !matches!(admission, Admission::New(_)) {
                return Err(DeleteError::Mismatch);
            }
        } else {
            self.db
                .continue_trash(
                    &request.operation_id,
                    &step,
                    &plan,
                    &request.ordinals,
                    &fingerprint(&request.token),
                )
                .map_err(stored)?;
        }
        super::fault::hit("after_admission", None)?;
        previews.remove(&request.token);
        drop(previews);
        self.execute(
            &request.operation_id,
            &plan,
            &request.ordinals,
            request.expected_checkpoint + 1,
        )?;
        Ok(DeleteAcknowledged {
            operation_id: request.operation_id,
            step_id: request.step_id,
            admitted_checkpoint: request.expected_checkpoint + 1,
        })
    }
    fn operation(&self, id: &OperationId) -> Result<Option<DeleteResult>, DeleteError> {
        match self.load(id) {
            Ok((op, plan, members)) => self.public(op, &plan, &members).map(Some),
            Err(DeleteError::NotFound) => Ok(None),
            Err(e) => Err(e),
        }
    }
    fn unfinished(&self, cursor: Option<&str>, limit: usize) -> Result<DeletePage, DeleteError> {
        if limit == 0 || limit > 100 {
            return Err(DeleteError::Invalid);
        }
        let prefix = hash_input(
            "trash_cursor",
            1,
            &[
                ("workspace", &self.identity.workspace_id),
                ("filter", "source_trash:unfinished"),
            ],
        );
        let after = match cursor {
            None => String::new(),
            Some(s) => {
                let (scope, id) = s.split_once('.').ok_or(DeleteError::Invalid)?;
                if scope != prefix || OperationId::parse(id).is_none() {
                    return Err(DeleteError::Invalid);
                }
                id.into()
            }
        };
        let ids:Vec<String>=self.db.with(|c|{
            let mut stmt=c.prepare("SELECT operation_id FROM operations WHERE kind='source_trash' AND state IN ('running','paused','uncertain') AND workspace_id=?1 AND operation_id>?2 ORDER BY operation_id LIMIT ?3").map_err(|_|DbError::Storage)?;
            stmt.query_map((&self.identity.workspace_id, &after, (limit+1) as i64),|r|r.get(0)).map_err(|_|DbError::Storage)?.collect::<Result<_,_>>().map_err(|_|DbError::Storage)
        }).map_err(|_|DeleteError::Storage)?;
        let next_cursor = if ids.len() > limit {
            Some(format!("{prefix}.{}", ids[limit - 1]))
        } else {
            None
        };
        let mut operations = Vec::new();
        for id in ids.into_iter().take(limit) {
            let id = OperationId::parse(&id).ok_or(DeleteError::Storage)?;
            if let Some(result) = self.operation(&id)?
                && result.state != DeleteState::Succeeded
            {
                operations.push(result);
            }
        }
        Ok(DeletePage {
            operations,
            next_cursor,
        })
    }
}

#[cfg(all(test, feature = "fault-injection"))]
mod native_harness;
#[cfg(test)]
mod tests;

//! Checkpoint/authorization and per-member progress transactions. Native calls and
//! hashing happen outside these transactions under the shared source coordinator.
use super::*;
use crate::ledger::{advance_tx, fetch, finish_tx};
use diary_application::operations::{NewStep, OperationId, OperationState, StepId, StepOutcome};

fn sql(_: rusqlite::Error) -> DbError {
    DbError::Storage
}
fn used(tx: &rusqlite::Connection, fingerprint: &str) -> Result<bool, DbError> {
    Ok(tx
        .query_row(
            "SELECT 1 FROM trash_authorizations WHERE fingerprint=?1",
            [fingerprint],
            |_| Ok(()),
        )
        .optional()
        .map_err(sql)?
        .is_some())
}
fn duplicate(
    tx: &rusqlite::Connection,
    id: &OperationId,
    step: &StepId,
    hash: &str,
    checkpoint: u64,
) -> Result<Result<Option<u64>, StoreError>, DbError> {
    let prior:Option<(String,i64,i64)>=tx.query_row("SELECT input_hash,expected_checkpoint,new_checkpoint FROM operation_steps WHERE operation_id=?1 AND step_id=?2",[id.as_str(),step.as_str()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(sql)?;
    Ok(match prior {
        None => Ok(None),
        Some((h, expected, new))
            if h == hash
                && expected >= 0
                && expected as u64 == checkpoint
                && new == expected + 1 =>
        {
            Ok(Some(new as u64))
        }
        _ => Err(StoreError::Ledger(LedgerError::StepMismatch)),
    })
}

/// Only unchanged failed members may be selected by continuation. The sole pending
/// exception is original audio waiting behind a failed Markdown member.
pub fn continuation_ordinals(members: &[MemberRecord]) -> Result<Vec<u8>, StoreError> {
    if members.len() != 2
        || members
            .iter()
            .any(|m| matches!(m.status, MemberStatus::Intended | MemberStatus::Uncertain))
    {
        return Err(StoreError::Invalid);
    }
    let mut ordinals: Vec<_> = members
        .iter()
        .filter(|m| m.status == MemberStatus::Failed)
        .map(|m| m.ordinal)
        .collect();
    if members[0].status == MemberStatus::Failed && members[1].status == MemberStatus::Pending {
        ordinals.push(1);
    }
    if ordinals.is_empty() {
        return Err(StoreError::Invalid);
    }
    Ok(ordinals)
}

impl StoredPlan {
    pub fn confirmation_hash(
        &self,
        initial: bool,
        checkpoint: u64,
        ordinals: &[u8],
    ) -> Result<String, StoreError> {
        let ordinals = ordinals
            .iter()
            .map(u8::to_string)
            .collect::<Vec<_>>()
            .join(",");
        Ok(hash_input(
            "source_trash",
            1,
            &[
                ("action", if initial { "initial" } else { "continuation" }),
                ("source_id", &self.source_id),
                ("plan_hash", &self.hash()?),
                ("checkpoint", &checkpoint.to_string()),
                ("ordinals", &ordinals),
            ],
        ))
    }
}

impl Db {
    /// Durable duplicate lookup precedes all transient preview/token checks. The caller
    /// has authenticated the current client; no old-session credential is needed.
    pub fn trash_step(
        &self,
        id: &OperationId,
        step: &StepId,
        hash: &str,
        checkpoint: u64,
    ) -> Result<Option<u64>, StoreError> {
        self.with(|c| duplicate(c, id, step, hash, checkpoint))?
    }

    pub fn continue_trash(
        &self,
        id: &OperationId,
        step: &NewStep,
        plan: &StoredPlan,
        ordinals: &[u8],
        fingerprint: &str,
    ) -> Result<StepOutcome, StoreError> {
        if !fingerprint_ok(fingerprint)
            || step.expected_checkpoint > i64::MAX as u64 - 1
            || step.state != Some(OperationState::Running)
            || step.input_hash
                != plan.confirmation_hash(false, step.expected_checkpoint, ordinals)?
        {
            return Err(StoreError::Invalid);
        }
        self.transact(|tx| {
            match duplicate(tx,id,&step.step_id,&step.input_hash,step.expected_checkpoint)? {
                Err(e)=>return Ok(Err(e)),
                Ok(Some(checkpoint))=>return Ok(Ok(StepOutcome::Recorded{outcome:format!("admitted:{checkpoint}"),checkpoint})),
                Ok(None)=>{},
            }
            if used(tx,fingerprint)? { return Ok(Err(StoreError::TokenUsed)); }
            let Some(op)=fetch(tx,id)? else { return Ok(Err(StoreError::Ledger(LedgerError::NotFound))); };
            if op.kind!="source_trash" || op.state!=OperationState::Paused {return Ok(Err(StoreError::Invalid));}
            if op.checkpoint!=step.expected_checkpoint {return Ok(Err(StoreError::Ledger(LedgerError::CheckpointMismatch{current:op.checkpoint})));}
            let (saved,members)=match load_plan(tx,id)? {Ok(Some(p))=>p,_=>return Ok(Err(StoreError::Corrupt))};
            if saved.hash()!=plan.hash() || continuation_ordinals(&members).as_ref()!=Ok(&ordinals.to_vec()) { return Ok(Err(StoreError::Invalid)); }
            let outcome=match advance_tx(tx,id,step)? {Ok(o)=>o,Err(e)=>return Ok(Err(StoreError::Ledger(e)))};
            for ordinal in ordinals {
                tx.execute("UPDATE trash_members SET status='pending',receipt=NULL,reason=NULL,checkpoint=?3 WHERE operation_id=?1 AND ordinal=?2",params![id.as_str(),ordinal,step.expected_checkpoint as i64+1]).map_err(sql)?;
            }
            tx.execute("INSERT INTO trash_authorizations VALUES (?1,?2,?3,?4)",params![fingerprint,id.as_str(),step.step_id.as_str(),plan.hash().map_err(|_|DbError::Damaged)?]).map_err(sql)?;
            Ok(Ok(outcome))
        })?
    }

    /// Must commit before a native call. Audio can never overtake Markdown.
    pub fn intend_trash_member(
        &self,
        id: &OperationId,
        ordinal: u8,
        checkpoint: u64,
    ) -> Result<(), StoreError> {
        self.transact(|tx| {
            let Some(op)=fetch(tx,id)? else {return Ok(Err(StoreError::Invalid));};
            let (_,members)=match load_plan(tx,id)? {Ok(Some(p))=>p,_=>return Ok(Err(StoreError::Corrupt))};
            if op.kind!="source_trash" || op.state!=OperationState::Running || op.checkpoint!=checkpoint || ordinal>1
                || members[ordinal as usize].status!=MemberStatus::Pending
                || (ordinal==1 && members[0].status!=MemberStatus::Moved)
            {return Ok(Err(StoreError::Invalid));}
            tx.execute("UPDATE trash_members SET status='intended',checkpoint=?3 WHERE operation_id=?1 AND ordinal=?2",params![id.as_str(),ordinal,checkpoint as i64]).map_err(sql)?;
            Ok(Ok(()))
        })?
    }

    /// Receipt/member progress and the operation transition commit together. A failed
    /// transaction leaves intent outstanding; callers stop before another native call.
    pub fn record_trash_member(
        &self,
        id: &OperationId,
        ordinal: u8,
        checkpoint: u64,
        status: MemberStatus,
        receipt: Option<&Receipt>,
        reason: Option<&str>,
    ) -> Result<(), StoreError> {
        if !matches!(
            status,
            MemberStatus::Moved | MemberStatus::Failed | MemberStatus::Uncertain
        ) || (status == MemberStatus::Moved && receipt.is_none())
            || reason.is_some_and(|s| {
                ![
                    "unavailable",
                    "changed",
                    "unsafe_path",
                    "ownership_ambiguous",
                    "root_changed",
                    "uncertain",
                ]
                .contains(&s)
            })
        {
            return Err(StoreError::Invalid);
        }
        let encoded = receipt
            .map(serde_json::to_string)
            .transpose()
            .map_err(|_| StoreError::Invalid)?;
        if encoded.as_ref().is_some_and(|s| s.len() > 8192) {
            return Err(StoreError::Invalid);
        }
        self.transact(|tx| {
            let Some(op)=fetch(tx,id)? else {return Ok(Err(StoreError::Invalid));};
            let (plan,members)=match load_plan(tx,id)? {Ok(Some(p))=>p,_=>return Ok(Err(StoreError::Corrupt))};
            if op.kind!="source_trash" || op.state!=OperationState::Running || op.checkpoint!=checkpoint || ordinal>1
                || !matches!(members[ordinal as usize].status,MemberStatus::Intended|MemberStatus::Pending)
                || (status==MemberStatus::Moved && members[ordinal as usize].status!=MemberStatus::Intended)
            {return Ok(Err(StoreError::Invalid));}
            let original=if ordinal==0 {Some(&plan.markdown)} else {plan.audio.as_ref()};
            if receipt.is_some_and(|r| !r.path.is_absolute() || r.path.to_str().is_none_or(|s|s.len()>4096)
                || original.is_none_or(|o|r.dev!=o.dev || r.ino!=o.ino)
                || r.path==plan.root.join(if ordinal==0 {&plan.markdown_rel} else {&plan.audio_rel}))
            {return Ok(Err(StoreError::Invalid));}
            tx.execute("UPDATE trash_members SET status=?3,receipt=?4,reason=?5,checkpoint=?6 WHERE operation_id=?1 AND ordinal=?2",params![id.as_str(),ordinal,status.as_str(),encoded,reason,checkpoint as i64]).map_err(sql)?;
            if status==MemberStatus::Failed || status==MemberStatus::Uncertain {
                tx.execute("UPDATE operations SET state=?2 WHERE operation_id=?1",params![id.as_str(),if status==MemberStatus::Uncertain {"uncertain"} else {"paused"}]).map_err(sql)?;
            } else if members.iter().all(|m|m.ordinal==ordinal || matches!(m.status,MemberStatus::Moved|MemberStatus::Missing))
                && let Err(e)=finish_tx(tx,id,OperationState::Succeeded,"source_trash")? {return Ok(Err(StoreError::Ledger(e)));}

            Ok(Ok(()))
        })?
    }

    /// Startup never replays a native call. Pending members require renewed consent;
    /// an unreceipted intent stays uncertain even when the original is unchanged.
    pub fn recover_trash(&self, id: &OperationId) -> Result<(), StoreError> {
        self.recover_trash_with_proof(id, &[], false)
    }

    /// `proven` comes only from receipt/resource verification outside the transaction.
    pub fn recover_trash_with_proof(
        &self,
        id: &OperationId,
        proven: &[u8],
        root_changed: bool,
    ) -> Result<(), StoreError> {
        self.transact(|tx| {
            let Some(op) = fetch(tx, id)? else { return Ok(Err(StoreError::Invalid)); };
            let (_, mut members) = match load_plan(tx, id)? { Ok(Some(p)) => p, _ => return Ok(Err(StoreError::Corrupt)) };
            if op.kind != "source_trash" || op.state.is_terminal() { return Ok(Err(StoreError::Invalid)); }
            for ordinal in proven {
                let Some(member) = members.get_mut(*ordinal as usize) else { return Ok(Err(StoreError::Invalid)); };
                if !matches!(member.status, MemberStatus::Intended | MemberStatus::Uncertain) || member.receipt.is_none() { return Ok(Err(StoreError::Invalid)); }
                member.status = MemberStatus::Moved;
            }
            for ordinal in proven {
                tx.execute("UPDATE trash_members SET status='moved',reason=NULL WHERE operation_id=?1 AND ordinal=?2", params![id.as_str(), ordinal]).map_err(sql)?;
            }
            if members.iter().all(|m| matches!(m.status, MemberStatus::Moved | MemberStatus::Missing)) {
                return Ok(finish_tx(tx, id, OperationState::Succeeded, "source_trash")?.map_err(StoreError::Ledger));
            }
            let uncertain = members.iter().any(|m| matches!(m.status, MemberStatus::Intended | MemberStatus::Uncertain));
            tx.execute("UPDATE trash_members SET status='uncertain',reason='uncertain' WHERE operation_id=?1 AND status='intended'", [id.as_str()]).map_err(sql)?;
            let pending = if members[0].status == MemberStatus::Pending { Some(0) }
                else if members[0].status == MemberStatus::Moved && members[1].status == MemberStatus::Pending { Some(1) } else { None };
            if let Some(ordinal) = pending && !uncertain {
                tx.execute("UPDATE trash_members SET status='failed',reason=?3 WHERE operation_id=?1 AND ordinal=?2", params![id.as_str(),ordinal,if root_changed {"root_changed"} else {"unavailable"}]).map_err(sql)?;
            }
            tx.execute("UPDATE operations SET state=?2 WHERE operation_id=?1", params![id.as_str(), if uncertain {"uncertain"} else {"paused"}]).map_err(sql)?;
            Ok(Ok(()))
        })?
    }
}

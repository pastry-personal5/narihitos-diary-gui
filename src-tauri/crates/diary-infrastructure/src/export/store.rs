//! Export persistence: intent, per-chunk checkpoints, receipts and consumed authorizations.
//! Each method is one immediate transaction, so admission, token consumption and terminal
//! transitions commit atomically with the ledger.
use crate::{
    ledger::{admit_tx, advance_tx, finish_tx},
    sqlite::{Db, DbError},
};
use diary_application::{
    export::{ContentKind, Receipt, Target},
    operations::{
        Admission, LedgerError, NewOperation, NewStep, OperationId, OperationState, StepOutcome,
    },
};
use rusqlite::{OptionalExtension, params};

#[derive(Debug, PartialEq, Eq)]
pub enum StoreError {
    Invalid,
    TokenUsed,
    Ledger(LedgerError),
    Storage,
}
impl From<DbError> for StoreError {
    fn from(_: DbError) -> Self {
        Self::Storage
    }
}
fn sql(_: rusqlite::Error) -> DbError {
    DbError::Storage
}

/// What is persisted before any destination call.
pub struct Intent {
    pub content_kind: ContentKind,
    pub illustrated_version: Option<String>,
    pub photos: Vec<Vec<u8>>,
    pub target: Target,
    pub destination_key: String,
    pub authentic_id: String,
    pub date: String,
    pub revision_sequence: u64,
    pub revision_sha: String,
    pub snapshot: Vec<u8>,
    pub reason: Option<&'static str>,
}
pub const RENDERER_VERSION: u32 = 1;
pub const API_VERSION: u32 = 1;

#[derive(Debug, Clone)]
pub struct Row {
    pub content_kind: ContentKind,
    pub illustrated_version: Option<String>,
    pub target: Target,
    pub destination_key: String,
    pub authentic_id: String,
    pub date: String,
    pub revision_sequence: u64,
    pub revision_sha: String,
    pub snapshot: Option<Vec<u8>>,
    pub reason: Option<String>,
}

pub struct Publication<'a> {
    pub destination_key: &'a str,
    pub authentic_id: &'a str,
    pub date: &'a str,
    pub receipt: &'a Receipt,
}

impl Db {
    /// Ledger intent, reservations, first step, export row and chunk 0 in one transaction.
    pub fn admit_export(
        &self,
        op: &NewOperation,
        intent: &Intent,
    ) -> Result<Admission, StoreError> {
        if intent.snapshot.len() > diary_application::export::MAX_SNAPSHOT_BYTES
            || intent.photos.len() > 99
            || intent.photos.iter().any(|p| p.len() > 5 * 1024 * 1024)
            || intent.photos.iter().map(Vec::len).sum::<usize>() > 64 * 1024 * 1024
        {
            return Err(StoreError::Invalid);
        }
        self.transact(|tx| {
            let admitted = match admit_tx(tx, op)? {
                Ok(a) => a,
                Err(e) => return Ok(Err(StoreError::Ledger(e))),
            };
            if matches!(admitted, Admission::New(_)) {
                tx.execute(
                    "INSERT INTO export_operations VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
                    params![
                        op.id.as_str(),
                        intent.target.as_str(),
                        intent.destination_key,
                        intent.authentic_id,
                        intent.date,
                        intent.revision_sequence as i64,
                        intent.revision_sha,
                        intent.revision_sha,
                        intent.snapshot,
                        RENDERER_VERSION,
                        API_VERSION,
                        intent.reason,
                        intent.content_kind.as_str(),
                        intent.illustrated_version,
                    ],
                )
                .map_err(sql)?;
                for (n, jpeg) in intent.photos.iter().enumerate() {
                    tx.execute("INSERT INTO export_photos VALUES (?1,?2,?3)", params![op.id.as_str(), n as i64, jpeg]).map_err(sql)?;
                }
                tx.execute(
                    "INSERT INTO export_checkpoints VALUES (?1, 0, ?2, 'pending', NULL, 1)",
                    params![op.id.as_str(), intent.revision_sha],
                )
                .map_err(sql)?;
            }
            Ok(Ok(admitted))
        })?
    }

    pub fn export_row(&self, id: &OperationId) -> Result<Option<Row>, StoreError> {
        Ok(self.with(|c| {
            c.query_row(
                "SELECT target,destination_key,authentic_id,date,revision_sequence,revision_sha,snapshot,reason,content_kind,illustrated_version \
                 FROM export_operations WHERE operation_id=?1",
                [id.as_str()],
                |r| {
                    let target: String = r.get(0)?;
                    let kind: String = r.get(8)?;
                    Ok(Row {
                        content_kind: ContentKind::parse(&kind).ok_or(rusqlite::Error::InvalidQuery)?,
                        illustrated_version: r.get(9)?,
                        target: Target::parse(&target).ok_or(rusqlite::Error::InvalidQuery)?,
                        destination_key: r.get(1)?,
                        authentic_id: r.get(2)?,
                        date: r.get(3)?,
                        revision_sequence: r.get::<_, i64>(4)? as u64,
                        revision_sha: r.get(5)?,
                        snapshot: r.get(6)?,
                        reason: r.get(7)?,
                    })
                },
            )
            .optional()
            .map_err(sql)
        })?)
    }

    /// The recorded checkpoint of a step, ahead of any token or checkpoint check. A reused step
    /// ID with a different input is a mismatch.
    pub fn export_step(
        &self,
        id: &OperationId,
        step: &str,
        input_hash: &str,
    ) -> Result<Option<u64>, StoreError> {
        let prior: Option<(String, i64)> = self.with(|c| {
            c.query_row(
                "SELECT input_hash,new_checkpoint FROM operation_steps WHERE operation_id=?1 AND step_id=?2",
                [id.as_str(), step],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(sql)
        })?;
        match prior {
            None => Ok(None),
            Some((hash, cp)) if hash == input_hash => Ok(Some(cp as u64)),
            Some(_) => Err(StoreError::Ledger(LedgerError::StepMismatch)),
        }
    }

    /// Consumes the token fingerprint and checkpoints the authorized step before its side
    /// effect, atomically.
    pub fn confirm_export(
        &self,
        id: &OperationId,
        step: &NewStep,
        fingerprint: &str,
    ) -> Result<StepOutcome, StoreError> {
        self.transact(|tx| {
            // Return a recorded step before token consumption or checkpoint checks.
            let prior = tx.query_row(
                "SELECT input_hash,outcome,new_checkpoint FROM operation_steps WHERE operation_id=?1 AND step_id=?2",
                [id.as_str(), step.step_id.as_str()],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?)),
            ).optional().map_err(sql)?;
            if let Some((hash, outcome, checkpoint)) = prior {
                return Ok(if hash == step.input_hash {
                    Ok(StepOutcome::Recorded { outcome, checkpoint: checkpoint as u64 })
                } else {
                    Err(StoreError::Ledger(LedgerError::StepMismatch))
                });
            }
            let used = tx
                .query_row(
                    "SELECT 1 FROM export_authorizations WHERE fingerprint=?1",
                    [fingerprint],
                    |_| Ok(()),
                )
                .optional()
                .map_err(sql)?
                .is_some();
            if used {
                return Ok(Err(StoreError::TokenUsed));
            }
            let outcome = match advance_tx(tx, id, step)? {
                Ok(o) => o,
                Err(e) => return Ok(Err(StoreError::Ledger(e))),
            };
            tx.execute(
                "INSERT INTO export_authorizations VALUES (?1,?2,?3)",
                params![fingerprint, id.as_str(), step.step_id.as_str()],
            )
            .map_err(sql)?;
            tx.execute(
                "UPDATE export_operations SET reason=NULL WHERE operation_id=?1",
                [id.as_str()],
            )
            .map_err(sql)?;
            Ok(Ok(outcome))
        })?
    }

    /// Moves the operation to `paused` or `uncertain` with a new step; the reservation stays.
    pub fn park_export(
        &self,
        id: &OperationId,
        step: &NewStep,
        reason: Option<&str>,
    ) -> Result<StepOutcome, StoreError> {
        self.transact(|tx| {
            let outcome = match advance_tx(tx, id, step)? {
                Ok(o) => o,
                Err(e) => return Ok(Err(StoreError::Ledger(e))),
            };
            tx.execute(
                "UPDATE export_operations SET reason=?2 WHERE operation_id=?1",
                params![id.as_str(), reason],
            )
            .map_err(sql)?;
            Ok(Ok(outcome))
        })?
    }

    pub fn export_chunk(
        &self,
        id: &OperationId,
        status: &str,
        remote_id: Option<&str>,
    ) -> Result<(), StoreError> {
        Ok(self.with(|c| {
            c.execute(
                "UPDATE export_checkpoints SET status=?2, remote_id=COALESCE(?3, remote_id) WHERE operation_id=?1 AND chunk=0",
                params![id.as_str(), status, remote_id],
            )
            .map(|_| ())
            .map_err(sql)
        })?)
    }
    /// A bounded tag shown with a succeeded export (for example `projection_warnings`).
    pub fn set_export_reason(&self, id: &OperationId, reason: &str) -> Result<(), StoreError> {
        Ok(self.with(|c| {
            c.execute(
                "UPDATE export_operations SET reason=?2 WHERE operation_id=?1",
                params![id.as_str(), reason],
            )
            .map(|_| ())
            .map_err(sql)
        })?)
    }
    pub fn export_chunk_status(&self, id: &OperationId) -> Result<Option<String>, StoreError> {
        Ok(self.with(|c| {
            c.query_row(
                "SELECT status FROM export_checkpoints WHERE operation_id=?1 AND chunk=0",
                [id.as_str()],
                |r| r.get(0),
            )
            .optional()
            .map_err(sql)
        })?)
    }

    /// The only terminal path: ledger state, receipt (on success) and snapshot deletion commit
    /// together (owner O5).
    pub fn finish_export(
        &self,
        id: &OperationId,
        state: OperationState,
        result: &str,
        publication: Option<Publication>,
    ) -> Result<(), StoreError> {
        if !state.is_terminal() || (state != OperationState::Succeeded && publication.is_some()) {
            return Err(StoreError::Invalid);
        }
        self.transact(|tx| {
            if let Err(e) = finish_tx(tx, id, state, result)? {
                return Ok(Err(StoreError::Ledger(e)));
            }
            tx.execute(
                // Only the success-time warning tag outlives the operation's parked reason.
                "UPDATE export_operations SET snapshot=NULL, \
                 reason=CASE WHEN ?2 AND reason='projection_warnings' THEN reason ELSE NULL END \
                 WHERE operation_id=?1",
                params![id.as_str(), state == OperationState::Succeeded],
            )
            .map_err(sql)?;
            tx.execute(
                "UPDATE export_checkpoints SET status=?2 WHERE operation_id=?1 AND status != 'done'",
                params![
                    id.as_str(),
                    if state == OperationState::Succeeded { "done" } else { "failed" }
                ],
            )
            .map_err(sql)?;
            tx.execute("DELETE FROM export_photos WHERE operation_id=?1", [id.as_str()]).map_err(sql)?;
            if let Some(p) = publication {
                tx.execute(
                    "INSERT INTO export_receipts VALUES (?1,?2,?3,?4,?5,?6,?7) \
                     ON CONFLICT (destination_key, authentic_id) DO UPDATE SET date=excluded.date, \
                     revision_sha=excluded.revision_sha, projection_digest=excluded.projection_digest, \
                     remote_ids=excluded.remote_ids, remote_fingerprint=excluded.remote_fingerprint",
                    params![
                        p.destination_key,
                        p.authentic_id,
                        p.date,
                        p.receipt.revision_sha,
                        p.receipt.projection_digest,
                        p.receipt.remote_ids,
                        p.receipt.remote_fingerprint
                    ],
                )
                .map_err(sql)?;
            }
            Ok(Ok(()))
        })?
    }

    pub fn export_receipt(
        &self,
        key: &str,
        authentic_id: &str,
    ) -> Result<Option<Receipt>, StoreError> {
        Ok(self.with(|c| {
            c.query_row(
                "SELECT revision_sha,projection_digest,remote_ids,remote_fingerprint FROM export_receipts \
                 WHERE destination_key=?1 AND authentic_id=?2",
                [key, authentic_id],
                |r| {
                    Ok(Receipt {
                        revision_sha: r.get(0)?,
                        projection_digest: r.get(1)?,
                        remote_ids: r.get(2)?,
                        remote_fingerprint: r.get(3)?,
                    })
                },
            )
            .optional()
            .map_err(sql)
        })?)
    }

    /// The unfinished export holding a destination, if any.
    pub fn export_holder(
        &self,
        key: &str,
    ) -> Result<Option<(OperationId, OperationState)>, StoreError> {
        let found: Option<(String, String)> = self.with(|c| {
            c.query_row(
                "SELECT o.operation_id, o.state FROM operation_resources r \
                 JOIN operations o USING (operation_id) WHERE r.resource_key=?1 LIMIT 1",
                [format!("export:{key}")],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(sql)
        })?;
        Ok(found.and_then(|(id, state)| {
            Some((OperationId::parse(&id)?, OperationState::parse(&state)?))
        }))
    }

    pub fn export_any_unfinished(&self) -> Result<bool, StoreError> {
        Ok(self.with(|c| {
            c.query_row(
                "SELECT EXISTS (SELECT 1 FROM operations WHERE kind='export' AND state IN ('running','paused','uncertain'))",
                [],
                |r| r.get(0),
            )
            .map_err(sql)
        })?)
    }

    pub fn export_unfinished_ids(
        &self,
        workspace: &str,
        after: &str,
        limit: usize,
    ) -> Result<Vec<String>, StoreError> {
        Ok(self.with(|c| {
            let mut stmt = c
                .prepare(
                    "SELECT operation_id FROM operations WHERE kind='export' AND state IN ('running','paused','uncertain') \
                     AND workspace_id=?1 AND operation_id>?2 ORDER BY operation_id LIMIT ?3",
                )
                .map_err(sql)?;
            stmt.query_map(params![workspace, after, limit as i64], |r| r.get(0))
                .map_err(sql)?
                .collect::<Result<_, _>>()
                .map_err(sql)
        })?)
    }
}

impl Db {
    pub fn export_photos(&self, id: &OperationId) -> Result<Vec<Vec<u8>>, StoreError> {
        Ok(self.with(|c| {
            let mut query = c
                .prepare("SELECT jpeg FROM export_photos WHERE operation_id=?1 ORDER BY n")
                .map_err(sql)?;
            query
                .query_map([id.as_str()], |r| r.get(0))
                .map_err(sql)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(sql)
        })?)
    }
}

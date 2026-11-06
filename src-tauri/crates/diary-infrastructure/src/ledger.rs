//! `OperationLedger` over the workspace database. Every method is one immediate transaction,
//! so admission, step recording and reservation checks are atomic with each other.
use crate::sqlite::{Db, DbError};
use diary_application::operations::{
    Admission, Identity, LedgerError, MAX_OUTCOME_BYTES, MAX_RESOURCES, NewOperation, NewStep,
    OperationId, OperationLedger, OperationState, Record, ResourceKey, StepId, StepOutcome,
};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};
use sha2::{Digest, Sha256};

const MAX_RESULT_BYTES: usize = 4096;

/// Versioned, field-order-independent business-input hash. Callers must leave out session
/// credentials, expiring token bytes and client correlation fields.
pub fn hash_input(kind: &str, version: u32, fields: &[(&str, &str)]) -> String {
    let mut sorted = fields.to_vec();
    sorted.sort_unstable();
    let mut sha = Sha256::new();
    sha.update(version.to_be_bytes());
    for part in std::iter::once(kind).chain(sorted.iter().flat_map(|(k, v)| [*k, *v])) {
        sha.update((part.len() as u64).to_be_bytes());
        sha.update(part.as_bytes());
    }
    sha.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

fn lower_hex64(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}
fn valid_step(step: &NewStep) -> bool {
    lower_hex64(&step.input_hash)
        && step.outcome.len() <= MAX_OUTCOME_BYTES
        && !step.state.is_some_and(OperationState::is_terminal) // terminal states go through finish
}

fn storage(_: rusqlite::Error) -> DbError {
    DbError::Storage
}
impl From<DbError> for LedgerError {
    fn from(e: DbError) -> Self {
        match e {
            DbError::Storage | DbError::Damaged | DbError::Newer | DbError::Missing => {
                LedgerError::Storage
            }
        }
    }
}

const COLUMNS: &str = "operation_id, kind, hash_version, input_hash, state, checkpoint, result, \
                       workspace_id, root, native, adapter_version";
fn record(row: &Row) -> rusqlite::Result<Record> {
    let bad =
        |i| rusqlite::Error::InvalidColumnType(i, "value".into(), rusqlite::types::Type::Text);
    let id: String = row.get(0)?;
    let state: String = row.get(4)?;
    Ok(Record {
        id: OperationId::parse(&id).ok_or_else(|| bad(0))?,
        kind: row.get(1)?,
        hash_version: row.get(2)?,
        input_hash: row.get(3)?,
        state: OperationState::parse(&state).ok_or_else(|| bad(4))?,
        checkpoint: row.get::<_, i64>(5)? as u64,
        result: row.get(6)?,
        identity: Identity {
            workspace_id: row.get(7)?,
            root: row.get(8)?,
            native: row.get(9)?,
            adapter_version: row.get(10)?,
        },
    })
}
pub(crate) fn fetch(tx: &Connection, id: &OperationId) -> Result<Option<Record>, DbError> {
    tx.query_row(
        &format!("SELECT {COLUMNS} FROM operations WHERE operation_id = ?1"),
        [id.as_str()],
        record,
    )
    .optional()
    .map_err(storage)
}

/// Inserts the step and moves the checkpoint by one inside the caller's transaction.
fn apply_step(tx: &Transaction, id: &OperationId, step: &NewStep) -> Result<(), DbError> {
    let next = step.expected_checkpoint + 1;
    tx.execute(
        "INSERT INTO operation_steps VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            id.as_str(),
            step.step_id.as_str(),
            step.input_hash,
            step.expected_checkpoint as i64,
            next as i64,
            step.outcome
        ],
    )
    .map_err(storage)?;
    tx.execute(
        "UPDATE operations SET checkpoint = ?2, state = COALESCE(?3, state) WHERE operation_id = ?1",
        params![id.as_str(), next as i64, step.state.map(OperationState::as_str)],
    )
    .map_err(storage)?;
    Ok(())
}

/// A transaction body reports either a domain error or a database failure.
pub(crate) type Tx<T> = Result<Result<T, LedgerError>, DbError>;

/// Terminal transition and reservation release inside the caller's transaction, so a caller can
/// commit its own receipt in the same transaction.
pub(crate) fn finish_tx(
    tx: &Transaction,
    id: &OperationId,
    state: OperationState,
    result: &str,
) -> Tx<()> {
    let Some(op) = fetch(tx, id)? else {
        return Ok(Err(LedgerError::NotFound));
    };
    if op.state.is_terminal() {
        let same = op.state == state && op.result.as_deref() == Some(result);
        return Ok(if same {
            Ok(())
        } else {
            Err(LedgerError::Terminal)
        });
    }
    tx.execute(
        "UPDATE operations SET state = ?2, result = ?3 WHERE operation_id = ?1",
        params![id.as_str(), state.as_str(), result],
    )
    .map_err(storage)?;
    tx.execute(
        "DELETE FROM operation_resources WHERE operation_id = ?1",
        [id.as_str()],
    )
    .map_err(storage)?;
    Ok(Ok(()))
}

/// Admission inside a caller-owned transaction, allowing a member plan and consumed
/// authorization to commit atomically with the ledger intent and reservations.
pub(crate) fn admit_tx(tx: &Transaction, op: &NewOperation) -> Tx<Admission> {
    let mut keys: Vec<&ResourceKey> = op.resources.iter().collect();
    keys.sort();
    keys.dedup();
    let kind_ok = !op.kind.is_empty()
        && op.kind.len() <= 64
        && op.kind.starts_with(|c: char| c.is_ascii_lowercase())
        && op
            .kind
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
    if !kind_ok
        || !lower_hex64(&op.input_hash)
        || keys.len() > MAX_RESOURCES
        || op
            .first_step
            .as_ref()
            .is_some_and(|s| !valid_step(s) || s.expected_checkpoint != 0)
    {
        return Ok(Err(LedgerError::Invalid));
    }

    // The recorded outcome of a retry wins over every new-action precondition.
    if let Some(existing) = fetch(tx, &op.id)? {
        let same = existing.hash_version == op.hash_version && existing.input_hash == op.input_hash;
        return Ok(Ok(if same {
            Admission::Existing(existing)
        } else {
            Admission::Mismatch
        }));
    }
    for key in &keys {
        let holder: Option<String> = tx
            .query_row(
                "SELECT operation_id FROM operation_resources WHERE resource_key = ?1 LIMIT 1",
                [key.as_str()],
                |r| r.get(0),
            )
            .optional()
            .map_err(storage)?;
        if let Some(holder) = holder.and_then(|h| OperationId::parse(&h)) {
            return Ok(Err(LedgerError::Reserved(holder)));
        }
    }
    tx.execute(
                &format!("INSERT INTO operations ({COLUMNS}) VALUES (?1, ?2, ?3, ?4, 'running', 0, NULL, ?5, ?6, ?7, ?8)"),
                params![
                    op.id.as_str(), op.kind, op.hash_version, op.input_hash,
                    op.identity.workspace_id, op.identity.root, op.identity.native,
                    op.identity.adapter_version
                ],
            )
            .map_err(storage)?;
    for key in &keys {
        tx.execute(
            "INSERT INTO operation_resources VALUES (?1, ?2)",
            [op.id.as_str(), key.as_str()],
        )
        .map_err(storage)?;
    }
    if let Some(step) = &op.first_step {
        apply_step(tx, &op.id, step)?;
    }
    Ok(Ok(Admission::New(
        fetch(tx, &op.id)?.ok_or(DbError::Storage)?,
    )))
}

/// Step/checkpoint transition inside a caller-owned authorization transaction.
pub(crate) fn advance_tx(tx: &Transaction, id: &OperationId, step: &NewStep) -> Tx<StepOutcome> {
    if !valid_step(step) {
        return Ok(Err(LedgerError::Invalid));
    }

    let Some(op) = fetch(tx, id)? else {
        return Ok(Err(LedgerError::NotFound));
    };
    let prior: Option<(String, String, i64)> = tx
        .query_row(
            "SELECT input_hash, outcome, new_checkpoint FROM operation_steps \
                     WHERE operation_id = ?1 AND step_id = ?2",
            [id.as_str(), step.step_id.as_str()],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(storage)?;
    // A duplicate answers before any token, terminal or checkpoint check.
    if let Some((hash, outcome, checkpoint)) = prior {
        return Ok(if hash == step.input_hash {
            Ok(StepOutcome::Recorded {
                outcome,
                checkpoint: checkpoint as u64,
            })
        } else {
            Err(LedgerError::StepMismatch)
        });
    }
    if op.state.is_terminal() {
        return Ok(Err(LedgerError::Terminal));
    }
    if op.checkpoint != step.expected_checkpoint {
        return Ok(Err(LedgerError::CheckpointMismatch {
            current: op.checkpoint,
        }));
    }
    apply_step(tx, id, step)?;
    Ok(Ok(StepOutcome::Applied(
        fetch(tx, id)?.ok_or(DbError::Storage)?,
    )))
}

impl OperationLedger for Db {
    fn admit(&self, op: NewOperation) -> Result<Admission, LedgerError> {
        self.transact(|tx| admit_tx(tx, &op))?
    }

    fn get(&self, id: &OperationId) -> Result<Option<Record>, LedgerError> {
        Ok(self.with(|c| fetch(c, id))?)
    }

    fn advance(&self, id: &OperationId, step: NewStep) -> Result<StepOutcome, LedgerError> {
        self.transact(|tx| advance_tx(tx, id, &step))?
    }

    fn finish(
        &self,
        id: &OperationId,
        state: OperationState,
        result: &str,
    ) -> Result<(), LedgerError> {
        if !state.is_terminal() || result.len() > MAX_RESULT_BYTES {
            return Err(LedgerError::Invalid);
        }
        self.transact(|tx| finish_tx(tx, id, state, result))?
    }

    fn step_outcome(&self, id: &OperationId, step: &StepId) -> Result<Option<String>, LedgerError> {
        Ok(self.with(|c| {
            c.query_row(
                "SELECT outcome FROM operation_steps WHERE operation_id = ?1 AND step_id = ?2",
                [id.as_str(), step.as_str()],
                |r| r.get(0),
            )
            .optional()
            .map_err(storage)
        })?)
    }

    fn unfinished(&self) -> Result<Vec<Record>, LedgerError> {
        Ok(self.with(|c| {
            let mut stmt = c
                .prepare(&format!(
                    "SELECT {COLUMNS} FROM operations \
                     WHERE state IN ('running', 'paused', 'uncertain') ORDER BY rowid"
                ))
                .map_err(storage)?;
            stmt.query_map([], record)
                .map_err(storage)?
                .collect::<Result<_, _>>()
                .map_err(storage)
        })?)
    }
}

#[cfg(test)]
mod tests;

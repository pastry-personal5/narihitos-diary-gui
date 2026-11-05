//! Workspace initialization and opening under the OS lock. The marker `workspace.json` is
//! published last; authoritative state is never created over existing state.
use crate::{
    lifecycle::{WorkspaceLock, random_uuid},
    sqlite::{DB_FILE, Db, DbError},
};
use diary_application::{
    diagnostics::valid_uuid,
    operations::{Classified, Identity, LedgerError, OperationLedger, Readiness, classify},
};
use diary_logging::private_fs::PrivateDir;
use serde::{Deserialize, Serialize};
use std::{io::Write, os::unix::fs::MetadataExt, time::SystemTime};

/// Bump when the meaning of a stored operation identity changes.
pub const ADAPTER_VERSION: u32 = 1;
const MARKER: &str = "workspace.json";
const MARKER_TMP: &str = "workspace.json.tmp";
const ROOT_ALLOWED: &[&str] = &[".diary", ".DS_Store"];
const UNMARKED_ALLOWED: &[&str] = &[
    "workspace.lock",
    DB_FILE,
    "state.sqlite3-wal",
    "state.sqlite3-shm",
    MARKER_TMP,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceError {
    Unsafe,
    Io,
}

/// Why a workspace is not `Ready`; for tests and the log's error tag, never user text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    UnexpectedFiles,
    MarkerInvalid,
    DatabaseMissing,
    DatabaseDamaged,
    SchemaNewer,
    WorkspaceMismatch,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Marker {
    schema_version: u32,
    workspace_id: String,
    created_at: String,
}

pub struct Workspace {
    pub readiness: Readiness,
    pub reason: Option<Reason>,
    /// What new operations are admitted under. `workspace_id` is empty when unknown.
    pub identity: Identity,
    db: Option<Db>,
}

/// Test-only interruption points of initialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Fault {
    AfterDatabase,
    AfterMarkerTemp,
}

impl Workspace {
    /// The lock proves exclusivity; recovery and inspection modes expose no ledger.
    pub fn open(lock: &WorkspaceLock) -> Result<Self, WorkspaceError> {
        open_with(lock, None)
    }

    pub fn ledger(&self) -> Result<&dyn OperationLedger, LedgerError> {
        match (&self.db, self.readiness) {
            (Some(db), Readiness::Ready) => Ok(db),
            _ => Err(LedgerError::Unavailable),
        }
    }

    /// Unfinished operations against this root. No reconciler exists yet, so valid ones are
    /// quarantined (`NoReconciler`) and keep their reservations until their milestone lands.
    pub fn classify(&self) -> Result<Vec<Classified>, LedgerError> {
        Ok(classify(&self.ledger()?.unfinished()?, &self.identity, &[]))
    }
}

fn open_with(lock: &WorkspaceLock, fault: Option<Fault>) -> Result<Workspace, WorkspaceError> {
    let root = &lock.root;
    let native = std::fs::metadata(root)
        .map(|m| format!("{}:{}", m.dev(), m.ino()))
        .map_err(|_| WorkspaceError::Io)?;
    let diary_path = root.join(".diary");
    let diary = PrivateDir::open(&diary_path).map_err(|_| WorkspaceError::Unsafe)?;
    let identity = |workspace_id: &str| Identity {
        workspace_id: workspace_id.into(),
        root: root.to_string_lossy().into_owned(),
        native: native.clone(),
        adapter_version: ADAPTER_VERSION,
    };
    let blocked = |readiness, reason, id: &str| Workspace {
        readiness,
        reason: Some(reason),
        identity: identity(id),
        db: None,
    };
    let names = diary.list().map_err(|_| WorkspaceError::Io)?;
    let db_path = diary_path.join(DB_FILE);

    if !names.iter().any(|n| n == MARKER) {
        let root_ok = std::fs::read_dir(root)
            .map_err(|_| WorkspaceError::Io)?
            .all(|e| {
                e.is_ok_and(|e| ROOT_ALLOWED.contains(&e.file_name().to_string_lossy().as_ref()))
            });
        // Sidecars of a database that is gone belong to someone else's history: inspect.
        let orphan_sidecar = !names.iter().any(|n| n == DB_FILE)
            && names.iter().any(|n| n.starts_with("state.sqlite3-"));
        let tree_ok = !orphan_sidecar
            && names.iter().all(|n| {
                UNMARKED_ALLOWED.contains(&n.as_str()) && diary.is_regular(n).unwrap_or(false)
            });
        let inspect = Readiness::InspectionRequired;
        if !root_ok || !tree_ok {
            return Ok(blocked(inspect, Reason::UnexpectedFiles, ""));
        }
        // No marker means no operation was ever admitted, so an empty database left by an
        // interrupted attempt is reused; one with operations is never touched.
        let db = if names.iter().any(|n| n == DB_FILE) {
            match Db::open_existing(&diary, &db_path, true) {
                Ok(db) if operation_count(&db)? == 0 => db,
                Ok(_) | Err(DbError::Damaged | DbError::Newer | DbError::Missing) => {
                    return Ok(blocked(inspect, Reason::UnexpectedFiles, ""));
                }
                Err(DbError::Storage) => return Err(WorkspaceError::Io),
            }
        } else {
            Db::create(&diary, &db_path).map_err(|_| WorkspaceError::Io)?
        };
        let id = ensure_workspace_id(&db)?;
        if fault == Some(Fault::AfterDatabase) {
            return Err(WorkspaceError::Io);
        }
        diary.sync().map_err(|_| WorkspaceError::Io)?;
        publish_marker(&diary, &id, fault)?;
        return Ok(Workspace {
            readiness: Readiness::Ready,
            reason: None,
            identity: identity(&id),
            db: Some(db),
        });
    }

    let marker = diary
        .read_bounded(MARKER, 4096)
        .ok()
        .and_then(|b| serde_json::from_slice::<Marker>(&b).ok())
        .filter(|m| {
            m.schema_version == 1
                && valid_uuid(&m.workspace_id)
                && !m.created_at.is_empty()
                && m.created_at.len() <= 32
        });
    let Some(marker) = marker else {
        return Ok(blocked(
            Readiness::InspectionRequired,
            Reason::MarkerInvalid,
            "",
        ));
    };
    let recovery = |reason| blocked(Readiness::Recovery, reason, &marker.workspace_id);
    let db = match Db::open_existing(&diary, &db_path, false) {
        Ok(db) => db,
        Err(DbError::Missing) => return Ok(recovery(Reason::DatabaseMissing)),
        Err(DbError::Damaged) => return Ok(recovery(Reason::DatabaseDamaged)),
        Err(DbError::Newer) => return Ok(recovery(Reason::SchemaNewer)),
        Err(DbError::Storage) => return Err(WorkspaceError::Io),
    };
    if ensure_matching_id(&db, &marker.workspace_id) != Some(true) {
        return Ok(recovery(Reason::WorkspaceMismatch));
    }
    // Ready means the ledger is readable: one unreadable row must not hide held reservations.
    if db.unfinished().is_err() {
        return Ok(recovery(Reason::DatabaseDamaged));
    }
    Ok(Workspace {
        readiness: Readiness::Ready,
        reason: None,
        identity: identity(&marker.workspace_id),
        db: Some(db),
    })
}

fn operation_count(db: &Db) -> Result<i64, WorkspaceError> {
    db.with(|c| {
        c.query_row("SELECT count(*) FROM operations", [], |r| r.get(0))
            .map_err(|_| DbError::Storage)
    })
    .map_err(|_| WorkspaceError::Io)
}

/// The id recorded in the database, creating it once.
fn ensure_workspace_id(db: &Db) -> Result<String, WorkspaceError> {
    let fresh = random_uuid().map_err(|_| WorkspaceError::Io)?;
    db.transact(|tx| {
        tx.execute(
            "INSERT OR IGNORE INTO workspace_meta VALUES (1, ?1)",
            [&fresh],
        )
        .map_err(|_| DbError::Storage)?;
        tx.query_row(
            "SELECT workspace_id FROM workspace_meta WHERE id = 1",
            [],
            |r| r.get(0),
        )
        .map_err(|_| DbError::Storage)
    })
    .map_err(|_| WorkspaceError::Io)
}

fn ensure_matching_id(db: &Db, expected: &str) -> Option<bool> {
    let found: Option<String> = db
        .with(|c| {
            c.query_row(
                "SELECT workspace_id FROM workspace_meta WHERE id = 1",
                [],
                |r| r.get(0),
            )
            .map_err(|_| DbError::Damaged)
        })
        .ok();
    found.map(|f| f == expected)
}

/// Temp file, sync, exclusive rename, directory sync: the marker appears whole or not at all.
fn publish_marker(
    diary: &PrivateDir,
    id: &str,
    fault: Option<Fault>,
) -> Result<(), WorkspaceError> {
    let _ = diary.remove(MARKER_TMP); // a stale temp from an interrupted attempt
    let bytes = serde_json::to_vec(&Marker {
        schema_version: 1,
        workspace_id: id.into(),
        created_at: utc_now(),
    })
    .map_err(|_| WorkspaceError::Io)?;
    let mut file = diary.create(MARKER_TMP).map_err(|_| WorkspaceError::Io)?;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| WorkspaceError::Io)?;
    if fault == Some(Fault::AfterMarkerTemp) {
        return Err(WorkspaceError::Io);
    }
    diary
        .rename(MARKER_TMP, MARKER, true)
        .map_err(|_| WorkspaceError::Io)?;
    diary.sync().map_err(|_| WorkspaceError::Io)
}

fn utc_now() -> String {
    let secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    utc(secs)
}
/// `YYYY-MM-DDTHH:MM:SSZ` (Hinnant's civil-from-days).
fn utc(secs: u64) -> String {
    let (days, rem) = ((secs / 86_400) as i64, secs % 86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests;

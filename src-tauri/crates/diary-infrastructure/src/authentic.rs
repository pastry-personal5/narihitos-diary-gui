//! Authentic persistence over the workspace: canonical files, the rebuildable catalog, guarded
//! durable saves and recovery. One process-wide write lock serializes mutations; the OS
//! workspace lock keeps other processes out, and `NSFileCoordinator` serializes cooperating
//! external writers around every read-before-write and publication.
mod catalog;
mod save;

use crate::{
    authentic_fs::{self as fs, Observed},
    coordination,
    sqlite::Db,
};
use diary_application::{
    authentic::{
        AuthenticStore, CreateRequest, Loaded, PAGE_LIMIT, Page, RebuildReport, ReviewSave,
        SaveOutcome, SaveRequest, StoreError,
    },
    operations::{Identity, LedgerError, OperationId, OperationLedger, Record},
};
use diary_domain::authentic::{
    AuthenticHeader, AuthenticRevision, DiaryDate, MAX_BODY_BYTES, parse, valid_label,
};
use sha2::{Digest, Sha256};
use std::{
    io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

/// Largest file we will read: the body cap plus generous header room.
pub(crate) const MAX_FILE: u64 = (MAX_BODY_BYTES + 32 * 1024) as u64;
pub const KIND_SAVE: &str = "authentic_save";
pub const KIND_CREATE: &str = "authentic_create";
pub use diary_application::review::KIND_REVIEW;

/// Durable boundaries of one save, in order. Tests inject a fault before or after any of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    StageWrite,
    StageSync,
    Intent,
    Publish,
    FileSync,
    DirSync,
    Verify,
    Receipt,
    Ack,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum When {
    Before,
    After,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultKind {
    /// Stop at once as a crash would: no cleanup, no ledger update.
    Crash,
    /// A failing system call (sync error, disk full): normal error handling runs.
    Io,
    /// SIGKILL this process (smoke test only).
    Kill,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fault {
    pub step: Step,
    pub when: When,
    pub kind: FaultKind,
}

/// Internal failure: a simulated crash must unwind without any cleanup.
#[derive(Debug)]
pub(crate) enum Fail {
    #[cfg_attr(not(any(test, feature = "fault-injection")), allow(dead_code))]
    Crash,
    Err(StoreError),
}
impl From<StoreError> for Fail {
    fn from(e: StoreError) -> Self {
        Self::Err(e)
    }
}
impl From<Fail> for StoreError {
    fn from(f: Fail) -> Self {
        match f {
            Fail::Crash => Self::Storage,
            Fail::Err(e) => e,
        }
    }
}
pub(crate) fn io_err(_: io::Error) -> Fail {
    Fail::Err(StoreError::Storage)
}
pub(crate) fn ledger_err(e: LedgerError) -> StoreError {
    match e {
        LedgerError::Reserved(_) => StoreError::Reserved,
        LedgerError::Unavailable => StoreError::Unavailable,
        _ => StoreError::Storage,
    }
}

/// One coordinated, bounded, symlink-refusing read of a managed file.
pub(crate) fn observe(path: &Path) -> Result<Option<Observed>, StoreError> {
    match coordination::read(path, || fs::read_bounded(path, MAX_FILE))
        .map_err(|_| StoreError::Coordination)?
    {
        Ok(seen) => Ok(seen),
        Err(e) if e.kind() == io::ErrorKind::InvalidData => Err(StoreError::Blocked(
            "file is too large or not a regular file".into(),
        )),
        Err(_) => Err(StoreError::Storage),
    }
}

fn load_from(path: &Path, date: &DiaryDate) -> Result<Loaded, StoreError> {
    let Some(seen) = observe(path)? else {
        return Ok(Loaded::Virtual(*date));
    };
    let (header, body) =
        parse(&seen.bytes, date).map_err(|e| StoreError::Blocked(e.to_string()))?;
    Ok(Loaded::File {
        revision: revision_of(&seen.bytes, &header),
        header: Box::new(header),
        body,
    })
}

fn dated_path(root: &Path, date: &DiaryDate) -> PathBuf {
    root.join("assets/diary/authentic")
        .join(format!("{:04}", date.year()))
        .join(format!("{:02}", date.month()))
        .join(format!("{date}.md"))
}

/// Recovery mode: files stay readable, every mutation and the ledger are unavailable.
pub struct ReadOnlyStore {
    pub(crate) root: PathBuf,
}

impl AuthenticStore for ReadOnlyStore {
    fn load(&self, date: &DiaryDate) -> Result<Loaded, StoreError> {
        load_from(&dated_path(&self.root, date), date)
    }
    fn list(&self, after: Option<DiaryDate>, limit: usize) -> Result<Page, StoreError> {
        let limit = limit.clamp(1, PAGE_LIMIT);
        let mut rows: Vec<_> = catalog::scan(&self.root.join("assets/diary/authentic"))?
            .into_iter()
            .filter(|r| after.is_none_or(|a| r.date > a))
            .map(catalog::Row::into_public)
            .collect();
        let next = (rows.len() > limit).then(|| rows[limit - 1].date);
        rows.truncate(limit);
        Ok(Page { rows, next })
    }
    fn save(&self, _: SaveRequest) -> Result<SaveOutcome, StoreError> {
        Err(StoreError::Unavailable)
    }
    fn create_empty(&self, _: CreateRequest) -> Result<SaveOutcome, StoreError> {
        Err(StoreError::Unavailable)
    }
    fn refresh(&self) -> Result<RebuildReport, StoreError> {
        Err(StoreError::Unavailable)
    }
    fn operation(&self, _: &OperationId) -> Result<Option<Record>, StoreError> {
        Err(StoreError::Unavailable)
    }
}

pub fn sha_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// SHA-256 for paragraph review digests, injected into the domain and application (O2).
pub struct Sha256Hasher;
impl diary_domain::review::Hasher for Sha256Hasher {
    fn sha256_hex(&self, bytes: &[u8]) -> String {
        sha_hex(bytes)
    }
}

pub struct RecoverReport {
    pub reconciled: usize,
    pub orphans_removed: usize,
    pub catalog: RebuildReport,
}

pub struct Store {
    pub(crate) db: Arc<Db>,
    pub(crate) root: PathBuf,
    pub(crate) identity: Identity,
    pub(crate) write_lock: Mutex<()>,
    pub(crate) now: Box<dyn Fn() -> u64 + Send + Sync>,
    /// `(IANA zone, timezone database version)` captured when a file is first created.
    pub(crate) zone: Box<dyn Fn() -> (String, String) + Send + Sync>,
    #[cfg(any(test, feature = "fault-injection"))]
    fault: Mutex<Option<Fault>>,
    /// Test override of the clock in Unix seconds; zero means the real clock.
    #[cfg(test)]
    pub(crate) clock: std::sync::atomic::AtomicU64,
}

impl Store {
    pub fn new(db: Arc<Db>, root: PathBuf, identity: Identity) -> Self {
        let store = Self {
            db,
            root,
            identity,
            write_lock: Mutex::new(()),
            now: Box::new(|| {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_secs())
            }),
            zone: Box::new(system_zone),
            #[cfg(any(test, feature = "fault-injection"))]
            fault: Mutex::new(None),
            #[cfg(test)]
            clock: std::sync::atomic::AtomicU64::new(0),
        };
        #[cfg(feature = "fault-injection")]
        if let Some(fault) = std::env::var("DIARY_FAULT")
            .ok()
            .and_then(|v| parse_fault(&v))
        {
            store.inject(fault);
        }
        store
    }

    pub(crate) fn clock_secs(&self) -> u64 {
        #[cfg(test)]
        {
            let fixed = self.clock.load(std::sync::atomic::Ordering::SeqCst);
            if fixed != 0 {
                return fixed;
            }
        }
        (self.now)()
    }

    #[cfg(any(test, feature = "fault-injection"))]
    pub fn inject(&self, fault: Fault) {
        *self.fault.lock().expect("fault lock") = Some(fault);
    }

    #[cfg(any(test, feature = "fault-injection"))]
    pub(crate) fn hit(&self, step: Step, when: When) -> Result<(), Fail> {
        let mut slot = self.fault.lock().expect("fault lock");
        match *slot {
            Some(f) if f.step == step && f.when == when => {
                *slot = None;
                match f.kind {
                    FaultKind::Crash => Err(Fail::Crash),
                    FaultKind::Io => Err(Fail::Err(StoreError::Storage)),
                    FaultKind::Kill => {
                        // SAFETY: killing our own process is always permitted.
                        unsafe { libc::kill(libc::getpid(), libc::SIGKILL) };
                        Err(Fail::Crash)
                    }
                }
            }
            _ => Ok(()),
        }
    }

    #[cfg(not(any(test, feature = "fault-injection")))]
    #[inline]
    pub(crate) fn hit(&self, _: Step, _: When) -> Result<(), Fail> {
        Ok(())
    }

    pub(crate) fn tree(&self) -> PathBuf {
        self.root.join("assets/diary/authentic")
    }

    pub(crate) fn dir_parts(date: &DiaryDate) -> [String; 5] {
        [
            "assets".into(),
            "diary".into(),
            "authentic".into(),
            format!("{:04}", date.year()),
            format!("{:02}", date.month()),
        ]
    }

    pub(crate) fn path_of(&self, date: &DiaryDate) -> PathBuf {
        dated_path(&self.root, date)
    }

    pub(crate) fn pending(&self) -> PathBuf {
        self.root.join(".diary/pending")
    }

    pub(crate) fn observe(&self, path: &Path) -> Result<Option<Observed>, StoreError> {
        observe(path)
    }

    pub fn recover(&self, resume: &[OperationId]) -> Result<RecoverReport, StoreError> {
        let _guard = self.write_lock.lock().map_err(|_| StoreError::Storage)?;
        let mut reconciled = 0;
        for record in self.db.unfinished().map_err(ledger_err)? {
            if resume.contains(&record.id) {
                self.reconcile(&record)?;
                reconciled += 1;
            }
        }
        let orphans_removed = self.remove_orphans()?;
        Ok(RecoverReport {
            reconciled,
            orphans_removed,
            catalog: catalog::rebuild(&self.db, &self.tree())?,
        })
    }
}

#[cfg(feature = "fault-injection")]
fn parse_fault(text: &str) -> Option<Fault> {
    let mut parts = text.split(':');
    let step = match parts.next()? {
        "stage_write" => Step::StageWrite,
        "stage_sync" => Step::StageSync,
        "intent" => Step::Intent,
        "publish" => Step::Publish,
        "file_sync" => Step::FileSync,
        "dir_sync" => Step::DirSync,
        "verify" => Step::Verify,
        "receipt" => Step::Receipt,
        "ack" => Step::Ack,
        _ => return None,
    };
    let when = match parts.next()? {
        "before" => When::Before,
        "after" => When::After,
        _ => return None,
    };
    let kind = match parts.next()? {
        "crash" => FaultKind::Crash,
        "io" => FaultKind::Io,
        "kill" => FaultKind::Kill,
        _ => return None,
    };
    Some(Fault { step, when, kind })
}

/// The Mac's IANA zone (target of `/etc/localtime`) and tzdata release (`+VERSION`). A missing
/// link means the system is on UTC; an unreadable release is recorded as `unknown`.
fn system_zone() -> (String, String) {
    let zone = std::fs::read_link("/etc/localtime")
        .ok()
        .and_then(|p| {
            let text = p.to_string_lossy().into_owned();
            text.split_once("zoneinfo/").map(|(_, z)| z.to_owned())
        })
        .filter(|z| valid_label(z, 64))
        .unwrap_or_else(|| "UTC".into());
    let release = std::fs::read_to_string("/var/db/timezone/zoneinfo/+VERSION")
        .ok()
        .map(|v| v.trim().to_owned())
        .filter(|v| valid_label(v, 32))
        .unwrap_or_else(|| "unknown".into());
    (zone, release)
}

pub(crate) fn revision_of(bytes: &[u8], header: &AuthenticHeader) -> AuthenticRevision {
    AuthenticRevision {
        sequence: header.revision,
        sha256: sha_hex(bytes),
    }
}

impl AuthenticStore for Store {
    fn load(&self, date: &DiaryDate) -> Result<Loaded, StoreError> {
        load_from(&self.path_of(date), date)
    }

    fn list(&self, after: Option<DiaryDate>, limit: usize) -> Result<Page, StoreError> {
        let limit = limit.clamp(1, PAGE_LIMIT);
        let mut rows = catalog::page(&self.db, after, limit + 1)?;
        let next = (rows.len() > limit).then(|| rows[limit - 1].date);
        rows.truncate(limit);
        Ok(Page { rows, next })
    }

    fn save(&self, request: SaveRequest) -> Result<SaveOutcome, StoreError> {
        let _guard = self.write_lock.lock().map_err(|_| StoreError::Storage)?;
        self.execute(
            KIND_SAVE,
            request.operation_id,
            request.date,
            request.expected,
            request.body,
            (request.buffer_generation, None),
        )
        .map_err(StoreError::from)
    }

    fn save_review(&self, r: ReviewSave) -> Result<SaveOutcome, StoreError> {
        let _guard = self.write_lock.lock().map_err(|_| StoreError::Storage)?;
        let extra = save::Extra {
            fields: vec![
                ("candidate", r.candidate_digest),
                ("hunk", r.hunk.clone()),
                ("before", r.before.clone()),
            ],
            receipt: format!("{},{},{}", r.hunk, r.before, r.after),
        };
        self.execute(
            KIND_REVIEW,
            r.operation_id,
            r.date,
            r.expected,
            r.body,
            (0, Some(extra)),
        )
        .map_err(StoreError::from)
    }

    fn create_empty(&self, request: CreateRequest) -> Result<SaveOutcome, StoreError> {
        let _guard = self.write_lock.lock().map_err(|_| StoreError::Storage)?;
        self.execute(
            KIND_CREATE,
            request.operation_id,
            request.date,
            None,
            String::new(),
            (0, None),
        )
        .map_err(StoreError::from)
    }

    fn refresh(&self) -> Result<RebuildReport, StoreError> {
        let _guard = self.write_lock.lock().map_err(|_| StoreError::Storage)?;
        catalog::rebuild(&self.db, &self.tree())
    }

    fn operation(&self, id: &OperationId) -> Result<Option<Record>, StoreError> {
        self.db.get(id).map_err(ledger_err)
    }
}

#[cfg(test)]
mod tests;

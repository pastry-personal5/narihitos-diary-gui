//! The guarded, durable producer-source body save.
//!
//! Same shape as the Authentic protocol: read fresh and check the client's digest, build and gate
//! the new bytes, stage them under `.diary/pending`, record the `prepared` intent under the
//! reservation `source:<id>`, publish a same-directory temp file with a coordinated rename,
//! sync, read back, then finish the operation in one transaction. Producer bytes are never
//! rolled back: reconciliation judges by the current bytes and keeps both versions on doubt.
use super::{
    narihito_write::{self as nw, Unwritable},
    read::{Observation, ReadError, read_fresh},
    root::{ResolvedRoot, resolve_roots, source_id},
};
use crate::{
    authentic::{Step, When, sha_hex},
    authentic_fs as fs,
    config::{Source, SourceKind},
    coordination,
    ledger::{finish_tx, hash_input},
    sqlite::Db,
};
use diary_application::{
    operations::{
        Admission, Identity, NewOperation, NewStep, OperationId, OperationLedger, OperationState,
        Record, ResourceKey, StepId,
    },
    source::{SaveError, SourceSaveRequest, SourceSaved, SourceWriter},
};
use diary_domain::source::path::{SourcePath, normalize};
use rusqlite::OptionalExtension;
use sha2::{Digest, Sha256};
use std::{
    io::{self, Write},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::Arc,
};

pub const KIND_SOURCE_SAVE: &str = "source_save";
/// Enabled after `scripts/smoke/producer.py` passed (2026-11-06); it proves file format only.
pub(super) const WRITE_BODY_VERIFIED: bool = true;

#[derive(Debug)]
enum Fail {
    #[cfg_attr(not(test), allow(dead_code))]
    Crash,
    Err(SaveError),
}
impl From<SaveError> for Fail {
    fn from(e: SaveError) -> Self {
        Self::Err(e)
    }
}

/// What an admitted save intends; stored as the outcome of its first step.
struct Prepared {
    source_id: String,
    namespace: String,
    key: String,
    old: String,
    new: String,
}
impl Prepared {
    fn encode(&self) -> String {
        format!(
            "prepared {} {} {} {} {}",
            self.source_id, self.namespace, self.old, self.new, self.key
        )
    }
    fn decode(text: &str) -> Option<Self> {
        let mut p = text.splitn(6, ' ');
        (p.next()? == "prepared").then_some(())?;
        Some(Self {
            source_id: p.next()?.into(),
            namespace: p.next()?.into(),
            old: p.next()?.into(),
            new: p.next()?.into(),
            key: p.next()?.into(),
        })
    }
}

fn step_id(op: &OperationId, label: &str) -> StepId {
    let d = sha_hex(format!("source:{label}:{}", op.as_str()).as_bytes());
    StepId::parse(&format!(
        "{}-{}-{}-{}-{}",
        &d[..8],
        &d[8..12],
        &d[12..16],
        &d[16..20],
        &d[20..32]
    ))
    .expect("formatted as a lowercase UUID")
}
fn step_hash(op: &OperationId, label: &str) -> String {
    hash_input(
        "source_step",
        1,
        &[("operation", op.as_str()), ("label", label)],
    )
}
fn ledger(e: diary_application::operations::LedgerError) -> SaveError {
    use diary_application::operations::LedgerError::*;
    match e {
        Reserved(_) => SaveError::Reserved,
        _ => SaveError::Storage,
    }
}
fn read_err(e: ReadError) -> SaveError {
    match e {
        ReadError::Missing | ReadError::InvalidPath => SaveError::SourceStale,
        ReadError::NotDownloaded => SaveError::CloudUnavailable,
        ReadError::SourceChanging => SaveError::SourceChanging,
        ReadError::NotRegular | ReadError::TooLarge | ReadError::Denied => {
            SaveError::UnsupportedFormat
        }
        ReadError::Io => SaveError::Storage,
    }
}
fn unwritable(e: Unwritable) -> SaveError {
    match e {
        Unwritable::Format => SaveError::UnsupportedFormat,
        Unwritable::Body => SaveError::InvalidBody,
    }
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn lower_hex64(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

struct Target {
    root: ResolvedRoot,
    path: SourcePath,
}

pub struct Writer {
    db: Arc<Db>,
    identity: Identity,
    sources: Vec<Source>,
    enabled: bool,
    now: Box<dyn Fn() -> u64 + Send + Sync>,
    #[cfg(test)]
    fault: std::sync::Mutex<Option<crate::authentic::Fault>>,
}

impl Writer {
    pub fn new(db: Arc<Db>, identity: Identity, sources: Vec<Source>) -> Self {
        Self {
            db,
            identity,
            sources,
            enabled: WRITE_BODY_VERIFIED,
            now: Box::new(|| {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_secs())
            }),
            #[cfg(test)]
            fault: std::sync::Mutex::new(None),
        }
    }

    #[cfg(test)]
    pub(crate) fn inject(&self, fault: crate::authentic::Fault) {
        *self.fault.lock().unwrap() = Some(fault);
    }
    #[cfg(test)]
    fn hit(&self, step: Step, when: When) -> Result<(), Fail> {
        use crate::authentic::FaultKind;
        let mut slot = self.fault.lock().unwrap();
        match *slot {
            Some(f) if f.step == step && f.when == when => {
                *slot = None;
                match f.kind {
                    FaultKind::Io => Err(Fail::Err(SaveError::Storage)),
                    _ => Err(Fail::Crash),
                }
            }
            _ => Ok(()),
        }
    }
    #[cfg(not(test))]
    fn hit(&self, _: Step, _: When) -> Result<(), Fail> {
        Ok(())
    }

    fn pending(&self) -> PathBuf {
        PathBuf::from(&self.identity.root).join(".diary/pending")
    }

    /// The configured Narihito root with this namespace, and the key normalized under it.
    fn locate(&self, namespace: &[u8], key: &str) -> Result<Target, SaveError> {
        for (source, root) in self.sources.iter().zip(resolve_roots(&self.sources)) {
            let Ok(root) = root else { continue };
            let digest = Sha256::digest(root.canonical.to_str().ok_or(SaveError::SourceStale)?);
            if digest.as_slice() != namespace {
                continue;
            }
            if !matches!(source.kind, SourceKind::Narihito) {
                return Err(SaveError::UnsupportedFormat);
            }
            let path = normalize(key, root.insensitive).map_err(|_| SaveError::SourceStale)?;
            return Ok(Target { root, path });
        }
        Err(SaveError::SourceStale)
    }

    fn target(&self, id: &str) -> Result<(Target, Vec<u8>), SaveError> {
        let row = self
            .db
            .transact(|tx| {
                tx.query_row(
                    "SELECT namespace, path_key FROM source_identities WHERE source_id = ?1",
                    [id],
                    |r| Ok((r.get::<_, Vec<u8>>(0)?, r.get::<_, String>(1)?)),
                )
                .optional()
                .map_err(|_| crate::sqlite::DbError::Storage)
            })
            .map_err(|_| SaveError::Storage)?;
        let (namespace, key) = row.ok_or(SaveError::SourceStale)?;
        let target = self.locate(&namespace, &key)?;
        if source_id(&target.root, &target.path) != id {
            return Err(SaveError::SourceStale);
        }
        Ok((target, namespace))
    }

    fn disk_path(target: &Target) -> PathBuf {
        target.root.canonical.join(&target.path.spelling)
    }

    fn save(&self, req: SourceSaveRequest) -> Result<SourceSaved, Fail> {
        let hash = hash_input(
            KIND_SOURCE_SAVE,
            1,
            &[
                ("source", &req.source_id),
                ("base", &req.base_digest),
                ("body", &sha_hex(req.body.as_bytes())),
            ],
        );
        let _guard = self
            .db
            .source_mutations
            .lock()
            .map_err(|_| SaveError::Storage)?;
        if let Some(record) = self.db.get(&req.operation_id).map_err(ledger)? {
            if record.kind != KIND_SOURCE_SAVE
                || record.hash_version != 1
                || record.input_hash != hash
            {
                return Err(SaveError::Mismatch.into());
            }
            return self.settle(record);
        }
        nw::body_ok(&req.body).map_err(unwritable)?;
        if !self.enabled {
            return Err(SaveError::UnsupportedFormat.into());
        }
        if !lower_hex64(&req.base_digest) {
            return Err(SaveError::ExternalChange.into());
        }
        let (target, namespace) = self.target(&req.source_id)?;
        let obs = read_fresh(&target.root, &target.path).map_err(read_err)?;
        let old = sha_hex(&obs.bytes);
        if old != req.base_digest {
            return Err(SaveError::ExternalChange.into());
        }
        if obs.nlink != 1 {
            return Err(SaveError::UnsupportedFormat.into());
        }
        let bytes = nw::splice(&obs.bytes, &target.path.spelling, &req.body, (self.now)())
            .map_err(unwritable)?;
        if bytes == obs.bytes {
            return Ok(SourceSaved {
                operation_id: req.operation_id,
                digest: old,
                size: obs.size,
                unchanged: true,
            });
        }
        let prepared = Prepared {
            source_id: req.source_id,
            namespace: hex(&namespace),
            key: target.path.key.clone(),
            old,
            new: sha_hex(&bytes),
        };
        let op = req.operation_id;
        let stage = self.pending().join(format!("{}.md", op.as_str()));
        let admitted = self.stage_and_admit(&op, &hash, &prepared, &bytes, &stage);
        if let Err(Fail::Err(_)) = &admitted {
            let _ = fs::remove(&stage);
        }
        admitted?;
        let mut published = false;
        let run = self
            .hit(Step::Intent, When::After)
            .and_then(|()| self.publish(&op, &prepared, &target, &bytes, &mut published));
        match run {
            Ok(()) => {}
            Err(Fail::Crash) => return Err(Fail::Crash),
            Err(Fail::Err(e)) => return Err(self.abandon(&op, &prepared, &stage, e, published)),
        }
        let record = self
            .db
            .get(&op)
            .map_err(ledger)?
            .ok_or(SaveError::Storage)?;
        self.settle(record)
    }

    fn stage_and_admit(
        &self,
        op: &OperationId,
        hash: &str,
        p: &Prepared,
        bytes: &[u8],
        stage: &Path,
    ) -> Result<(), Fail> {
        let io = |_: io::Error| Fail::Err(SaveError::Storage);
        fs::ensure_dirs(
            &PathBuf::from(&self.identity.root).join(".diary"),
            &["pending"],
        )
        .map_err(io)?;
        fs::remove(stage).map_err(io)?;
        self.hit(Step::StageWrite, When::Before)?;
        fs::create_new(stage, bytes).map_err(io)?;
        self.hit(Step::StageWrite, When::After)?;
        self.hit(Step::StageSync, When::Before)?;
        fs::sync_file(stage).map_err(io)?;
        fs::sync_path(&self.pending()).map_err(io)?;
        self.hit(Step::StageSync, When::After)?;
        self.hit(Step::Intent, When::Before)?;
        let key =
            ResourceKey::parse(&format!("source:{}", p.source_id)).ok_or(SaveError::Storage)?;
        let first = NewStep {
            step_id: step_id(op, "prepared"),
            input_hash: step_hash(op, "prepared"),
            expected_checkpoint: 0,
            outcome: p.encode(),
            state: None,
        };
        match self.db.admit(NewOperation {
            id: op.clone(),
            kind: KIND_SOURCE_SAVE.into(),
            hash_version: 1,
            input_hash: hash.to_owned(),
            resources: vec![key],
            identity: self.identity.clone(),
            first_step: Some(first),
        }) {
            Ok(Admission::New(_)) => Ok(()),
            Ok(_) => Err(SaveError::Storage.into()),
            Err(e) => Err(ledger(e).into()),
        }
    }

    fn temp_of(op: &OperationId, dir: &Path) -> PathBuf {
        dir.join(format!(".diary-save-{}.tmp", op.as_str()))
    }

    /// Removes leftover temps of finished operations next to the target.
    fn sweep(&self, dir: &Path, keep: &OperationId) {
        let Ok(unfinished) = self.db.unfinished() else {
            return;
        };
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Some(id) = name
                .strip_prefix(".diary-save-")
                .and_then(|n| n.strip_suffix(".tmp"))
                .and_then(OperationId::parse)
            else {
                continue;
            };
            if &id != keep
                && !unfinished.iter().any(|r| r.id == id)
                && entry.file_type().is_ok_and(|t| t.is_file())
            {
                let _ = fs::remove(&entry.path());
            }
        }
    }

    fn publish(
        &self,
        op: &OperationId,
        p: &Prepared,
        target: &Target,
        bytes: &[u8],
        published: &mut bool,
    ) -> Result<(), Fail> {
        let io = |_: io::Error| Fail::Err(SaveError::Storage);
        let path = Self::disk_path(target);
        let dir = path.parent().ok_or(SaveError::Storage)?;
        self.sweep(dir, op);
        let tmp = Self::temp_of(op, dir);
        fs::remove(&tmp).map_err(io)?;
        let mode = std::fs::metadata(&path).map_err(io)?.permissions().mode() & 0o7777;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(mode)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&tmp)
            .map_err(io)?;
        let written = file
            .set_permissions(std::fs::Permissions::from_mode(mode))
            .and_then(|()| file.write_all(bytes))
            .and_then(|()| fs::full_sync(&file));
        drop(file);
        if let Err(e) = written {
            let _ = fs::remove(&tmp);
            return Err(io(e));
        }
        self.hit(Step::Publish, When::Before)?;
        let old = p.old.clone();
        let outcome = coordination::write(&path, || -> Result<(), SaveError> {
            // Recheck inside the coordinated access, right before publishing.
            let seen = read_fresh(&target.root, &target.path).map_err(read_err)?;
            if sha_hex(&seen.bytes) != old {
                return Err(SaveError::ExternalChange);
            }
            if seen.nlink != 1 {
                return Err(SaveError::UnsupportedFormat);
            }
            fs::rename_replace(&tmp, &path).map_err(|_| SaveError::Storage)
        })
        .map_err(|_| SaveError::Storage);
        match outcome.and_then(|r| r) {
            Ok(()) => *published = true,
            Err(e) => {
                let _ = fs::remove(&tmp);
                return Err(e.into());
            }
        }
        self.hit(Step::Publish, When::After)?;
        self.durable_commit(op, p, target, bytes.len() as u64)
    }

    fn durable_commit(
        &self,
        op: &OperationId,
        p: &Prepared,
        target: &Target,
        size: u64,
    ) -> Result<(), Fail> {
        let io = |_: io::Error| Fail::Err(SaveError::Storage);
        let path = Self::disk_path(target);
        let dir = path.parent().ok_or(SaveError::Storage)?;
        self.hit(Step::FileSync, When::Before)?;
        fs::sync_file(&path).map_err(io)?;
        self.hit(Step::FileSync, When::After)?;
        self.hit(Step::DirSync, When::Before)?;
        fs::sync_path(dir).map_err(io)?;
        self.hit(Step::DirSync, When::After)?;
        self.hit(Step::Verify, When::Before)?;
        let seen: Observation = read_fresh(&target.root, &target.path).map_err(read_err)?;
        if sha_hex(&seen.bytes) != p.new {
            return Err(SaveError::ExternalChange.into());
        }
        self.hit(Step::Verify, When::After)?;
        self.hit(Step::Receipt, When::Before)?;
        let result = format!("{}:{size}", p.new);
        self.db
            .transact(|tx| finish_tx(tx, op, OperationState::Succeeded, &result))
            .map_err(|_| SaveError::Storage)?
            .map_err(ledger)?;
        self.hit(Step::Receipt, When::After)?;
        self.hit(Step::Ack, When::Before)?;
        let _ = fs::remove(&self.pending().join(format!("{}.md", op.as_str())));
        let _ = fs::remove(&Self::temp_of(op, dir));
        self.hit(Step::Ack, When::After)
    }

    /// A non-crash failure after admission: before publication the save failed; after it only a
    /// retry of the same ID may settle the outcome.
    fn abandon(
        &self,
        op: &OperationId,
        p: &Prepared,
        stage: &Path,
        error: SaveError,
        published: bool,
    ) -> Fail {
        if published {
            let _ = self.db.advance(
                op,
                NewStep {
                    step_id: step_id(op, "uncertain"),
                    input_hash: step_hash(op, "uncertain"),
                    expected_checkpoint: 1,
                    outcome: "uncertain".into(),
                    state: Some(OperationState::Uncertain),
                },
            );
            return SaveError::Uncertain.into();
        }
        let _ = self.finish_failed(op, p, failure_tag(error), stage, false);
        error.into()
    }

    fn finish_failed(
        &self,
        op: &OperationId,
        p: &Prepared,
        tag: &str,
        stage: &Path,
        keep_stage: bool,
    ) -> Result<(), SaveError> {
        self.db
            .finish(op, OperationState::Failed, tag)
            .map_err(ledger)?;
        if let Ok(namespace) = unhex(&p.namespace)
            && let Ok(t) = self.locate(&namespace, &p.key)
            && let Some(dir) = Self::disk_path(&t).parent()
        {
            let _ = fs::remove(&Self::temp_of(op, dir));
        }
        if !keep_stage {
            let _ = fs::remove(stage);
        }
        Ok(())
    }

    fn settle(&self, record: Record) -> Result<SourceSaved, Fail> {
        let record = if record.state.is_terminal() {
            record
        } else {
            self.reconcile(&record)?
        };
        match record.state {
            OperationState::Succeeded => {
                let (digest, size) = record
                    .result
                    .as_deref()
                    .and_then(|r| r.split_once(':'))
                    .and_then(|(d, s)| Some((d.to_owned(), s.parse().ok()?)))
                    .ok_or(SaveError::Storage)?;
                Ok(SourceSaved {
                    operation_id: record.id,
                    digest,
                    size,
                    unchanged: false,
                })
            }
            OperationState::Failed | OperationState::Cancelled => {
                Err(match record.result.as_deref() {
                    Some("conflict") => SaveError::ExternalChange,
                    Some("unsupported_format") => SaveError::UnsupportedFormat,
                    Some("source_stale") => SaveError::SourceStale,
                    Some("source_changing") => SaveError::SourceChanging,
                    Some("cloud_unavailable") => SaveError::CloudUnavailable,
                    _ => SaveError::Storage,
                }
                .into())
            }
            _ => Err(SaveError::Uncertain.into()),
        }
    }

    /// Judges an unfinished save by the current bytes of its target; never restores.
    fn reconcile(&self, record: &Record) -> Result<Record, Fail> {
        let op = &record.id;
        let fresh = || -> Result<Record, Fail> {
            Ok(self.db.get(op).map_err(ledger)?.ok_or(SaveError::Storage)?)
        };
        let stage = self.pending().join(format!("{}.md", op.as_str()));
        let Some(p) = self
            .db
            .step_outcome(op, &step_id(op, "prepared"))
            .map_err(ledger)?
            .and_then(|s| Prepared::decode(&s))
        else {
            self.db
                .finish(op, OperationState::Failed, "no_intent")
                .map_err(ledger)?;
            return fresh();
        };
        let Some(target) = unhex(&p.namespace)
            .ok()
            .and_then(|ns| self.locate(&ns, &p.key).ok())
        else {
            return fresh(); // root removed or repointed: stays unfinished, reservation kept
        };
        let Ok(current) = read_fresh(&target.root, &target.path) else {
            return fresh();
        };
        let digest = sha_hex(&current.bytes);
        let unresolved = |r: Result<(), Fail>| match r {
            Err(Fail::Crash) => Err(Fail::Crash),
            _ => fresh(),
        };
        if digest == p.new {
            return unresolved(self.durable_commit(op, &p, &target, current.size));
        }
        if digest == p.old {
            let staged = fs::read_bounded(&stage, 9 << 20).ok().flatten();
            let Some(staged) = staged.filter(|s| sha_hex(&s.bytes) == p.new) else {
                self.finish_failed(op, &p, "stage_missing", &stage, false)?;
                return fresh();
            };
            if nw::check(&staged.bytes, &target.path.spelling).is_err() {
                self.finish_failed(op, &p, "unsupported_format", &stage, true)?;
                return fresh();
            }
            let mut published = false;
            return match self.publish(op, &p, &target, &staged.bytes, &mut published) {
                Err(Fail::Err(SaveError::ExternalChange)) if !published => {
                    self.finish_failed(op, &p, "conflict", &stage, true)?;
                    fresh()
                }
                other => unresolved(other),
            };
        }
        self.finish_failed(op, &p, "conflict", &stage, true)?;
        fresh()
    }

    /// Startup: finish what a crash left behind. Returns how many operations were examined.
    pub fn recover(&self, resume: &[OperationId]) -> Result<usize, SaveError> {
        let _guard = self
            .db
            .source_mutations
            .lock()
            .map_err(|_| SaveError::Storage)?;
        let mut n = 0;
        for record in self.db.unfinished().map_err(ledger)? {
            if record.kind == KIND_SOURCE_SAVE && resume.contains(&record.id) {
                self.reconcile(&record).map_err(|_| SaveError::Storage)?;
                n += 1;
            }
        }
        Ok(n)
    }
}

fn failure_tag(e: SaveError) -> &'static str {
    match e {
        SaveError::ExternalChange => "conflict",
        SaveError::UnsupportedFormat => "unsupported_format",
        SaveError::SourceStale => "source_stale",
        SaveError::SourceChanging => "source_changing",
        SaveError::CloudUnavailable => "cloud_unavailable",
        _ => "publish_failed",
    }
}
fn unhex(s: &str) -> Result<Vec<u8>, ()> {
    if !s.len().is_multiple_of(2) || !s.is_ascii() {
        return Err(());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|_| ()))
        .collect()
}

impl SourceWriter for Writer {
    fn save_body(&self, request: SourceSaveRequest) -> Result<SourceSaved, SaveError> {
        self.save(request).map_err(|f| match f {
            Fail::Crash => SaveError::Storage,
            Fail::Err(e) => e,
        })
    }
}

#[cfg(test)]
mod tests;

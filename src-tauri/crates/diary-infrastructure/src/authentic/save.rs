//! The guarded save protocol and its reconciliation.
//!
//! Order for a new action: observe and check the expected revision, stage the complete new file
//! under `.diary/pending` and sync it, record the `prepared` intent with the operation, publish
//! with a coordinated no-clobber create or atomic replace, sync, verify by read-back, then commit
//! the catalog row and the operation result in one SQLite transaction. Only then is it
//! acknowledged. Reconciliation after a crash or an ambiguous failure judges by current bytes.
use super::{Fail, MAX_FILE, Step, Store, When, catalog, io_err, ledger_err, revision_of, sha_hex};
use crate::{
    authentic_fs as fs, coordination,
    ledger::{finish_tx, hash_input},
    lifecycle::random_uuid,
    workspace::utc,
};
use diary_application::{
    authentic::{ConflictKind, SaveOutcome, StoreError, resource_key},
    operations::{
        Admission, NewOperation, NewStep, OperationId, OperationLedger, OperationState, Record,
        ResourceKey, StepId,
    },
};
use diary_domain::authentic::{AuthenticHeader, AuthenticRevision, DiaryDate, parse, render};
use std::{io, path::Path};

/// What an admitted save intends; stored as the outcome of its first step.
struct Prepared {
    date: DiaryDate,
    old: Option<String>,
    new: String,
    seq: u64,
}

fn encode(p: &Prepared) -> String {
    format!(
        "prepared {} {} {} {}",
        p.date,
        p.old.as_deref().unwrap_or("absent"),
        p.new,
        p.seq
    )
}

fn decode(text: &str) -> Option<Prepared> {
    let mut parts = text.split(' ');
    (parts.next()? == "prepared").then_some(())?;
    let date = DiaryDate::parse(parts.next()?)?;
    let old = match parts.next()? {
        "absent" => None,
        digest => Some(digest.to_owned()),
    };
    let new = parts.next()?.to_owned();
    let seq = parts.next()?.parse().ok()?;
    parts.next().is_none().then_some(Prepared {
        date,
        old,
        new,
        seq,
    })
}

/// A deterministic step identity per operation and label, so recovery can find it again.
fn step_id(op: &OperationId, label: &str) -> StepId {
    let digest = sha_hex(format!("authentic:{label}:{}", op.as_str()).as_bytes());
    let id = format!(
        "{}-{}-{}-{}-{}",
        &digest[..8],
        &digest[8..12],
        &digest[12..16],
        &digest[16..20],
        &digest[20..32]
    );
    StepId::parse(&id).expect("formatted as a lowercase UUID")
}

fn step_hash(op: &OperationId, label: &str) -> String {
    hash_input(
        "authentic_step",
        1,
        &[("operation", op.as_str()), ("label", label)],
    )
}

fn failure(tag: &str) -> StoreError {
    match tag {
        "exists" => StoreError::Conflict(ConflictKind::Exists),
        "conflict" => StoreError::Conflict(ConflictKind::External),
        other => StoreError::Failed(other.to_owned()),
    }
}

impl Store {
    pub(crate) fn execute(
        &self,
        kind: &'static str,
        op: OperationId,
        date: DiaryDate,
        expected: Option<AuthenticRevision>,
        body: String,
        generation: u64,
    ) -> Result<SaveOutcome, Fail> {
        let expected_text = expected.as_ref().map_or_else(
            || "absent".to_owned(),
            |r| format!("{}:{}", r.sequence, r.sha256),
        );
        let hash = hash_input(
            kind,
            1,
            &[
                ("date", &date.to_string()),
                ("expected", &expected_text),
                ("body", &sha_hex(body.as_bytes())),
            ],
        );
        // A known ID answers with its recorded outcome before any new-action precondition.
        if let Some(record) = self.db.get(&op).map_err(ledger_err)? {
            if record.kind != kind || record.hash_version != 1 || record.input_hash != hash {
                return Err(StoreError::Mismatch.into());
            }
            return self.settle(record, generation);
        }

        let path = self.path_of(&date);
        let seen = self.observe(&path)?;
        catalog::observed(&self.db, &date, seen.as_ref()).map_err(|_| StoreError::Storage)?;
        let current = match &seen {
            None => None,
            Some(o) => Some(
                parse(&o.bytes, &date)
                    .map_err(|e| StoreError::Blocked(e.to_string()))?
                    .0,
            ),
        };
        let current_revision = current
            .as_ref()
            .zip(seen.as_ref())
            .map(|(h, o)| revision_of(&o.bytes, h));
        match (&expected, &current_revision) {
            (None, Some(_)) => return Err(StoreError::Conflict(ConflictKind::Exists).into()),
            (Some(_), None) => return Err(StoreError::Conflict(ConflictKind::Stale).into()),
            (Some(e), Some(c)) if e != c => {
                return Err(StoreError::Conflict(ConflictKind::Stale).into());
            }
            _ => {}
        }

        let stamp = utc(self.clock_secs());
        let header = match current {
            Some(mut header) => {
                // The sequence only moves forward, whatever the clock says.
                header.revision = header.revision.checked_add(1).ok_or(StoreError::Storage)?;
                header.updated_at = stamp;
                header
            }
            None => {
                let (timezone, timezone_database) = (self.zone)();
                AuthenticHeader {
                    id: random_uuid().map_err(|_| StoreError::Storage)?,
                    date,
                    timezone,
                    timezone_database,
                    created_at: stamp.clone(),
                    updated_at: stamp,
                    revision: 1,
                    extensions: Vec::new(),
                }
            }
        };
        let bytes = render(&header, &body).map_err(StoreError::Format)?;
        let prepared = Prepared {
            date,
            old: seen.as_ref().map(|o| sha_hex(&o.bytes)),
            new: sha_hex(&bytes),
            seq: header.revision,
        };

        // Stage, then record the intent; a failure here has published nothing.
        let stage = self.pending().join(format!("{}.md", op.as_str()));
        let admitted = self.stage_and_admit(kind, &op, &hash, &prepared, &bytes, &stage);
        if let Err(Fail::Err(_)) = &admitted {
            let _ = fs::remove(&stage);
        }
        admitted?;

        let mut published = false;
        let run = self
            .hit(Step::Intent, When::After)
            .and_then(|()| self.publish(&op, &prepared, &bytes, &mut published));
        match run {
            Ok(()) => {}
            Err(Fail::Crash) => return Err(Fail::Crash),
            Err(Fail::Err(error)) => return self.abandon(&op, &prepared, &stage, error, published),
        }
        let record = self
            .db
            .get(&op)
            .map_err(ledger_err)?
            .ok_or(StoreError::Storage)?;
        self.settle(record, generation)
    }

    fn stage_and_admit(
        &self,
        kind: &'static str,
        op: &OperationId,
        hash: &str,
        p: &Prepared,
        bytes: &[u8],
        stage: &Path,
    ) -> Result<(), Fail> {
        fs::ensure_dirs(&self.root.join(".diary"), &["pending"]).map_err(io_err)?;
        // An unreferenced stage with this name can only be a crashed earlier attempt.
        fs::remove(stage).map_err(io_err)?;
        self.hit(Step::StageWrite, When::Before)?;
        fs::create_new(stage, bytes).map_err(io_err)?;
        self.hit(Step::StageWrite, When::After)?;
        self.hit(Step::StageSync, When::Before)?;
        fs::sync_file(stage).map_err(io_err)?;
        fs::sync_path(&self.pending()).map_err(io_err)?;
        self.hit(Step::StageSync, When::After)?;
        self.hit(Step::Intent, When::Before)?;
        let key = ResourceKey::parse(&resource_key(&p.date)).ok_or(StoreError::Storage)?;
        let first = NewStep {
            step_id: step_id(op, "prepared"),
            input_hash: step_hash(op, "prepared"),
            expected_checkpoint: 0,
            outcome: encode(p),
            state: None,
        };
        match self.db.admit(NewOperation {
            id: op.clone(),
            kind: kind.into(),
            hash_version: 1,
            input_hash: hash.to_owned(),
            resources: vec![key],
            identity: self.identity.clone(),
            first_step: Some(first),
        }) {
            Ok(Admission::New(_)) => Ok(()),
            Ok(_) => Err(StoreError::Storage.into()),
            Err(e) => Err(ledger_err(e).into()),
        }
    }

    /// Copies the staged bytes next to the target, publishes under coordination, then makes the
    /// result durable and commits it.
    fn publish(
        &self,
        op: &OperationId,
        p: &Prepared,
        bytes: &[u8],
        published: &mut bool,
    ) -> Result<(), Fail> {
        let path = self.path_of(&p.date);
        let dir = path.parent().ok_or(StoreError::Storage)?;
        let parts = Self::dir_parts(&p.date);
        let parts: Vec<&str> = parts.iter().map(String::as_str).collect();
        fs::ensure_dirs(&self.root, &parts).map_err(io_err)?;
        let tmp = self.temp_of(op, &p.date);
        fs::remove(&tmp).map_err(io_err)?;
        fs::create_new(&tmp, bytes).map_err(io_err)?;
        fs::sync_file(&tmp).map_err(io_err)?;
        self.hit(Step::Publish, When::Before)?;
        let old = p.old.clone();
        let outcome = coordination::write(&path, || -> io::Result<bool> {
            // Recheck the observation inside the coordinated access, right before publishing.
            let current = fs::read_bounded(&path, MAX_FILE)?.map(|o| sha_hex(&o.bytes));
            if current != old {
                return Ok(false);
            }
            if old.is_none() {
                fs::rename_new(&tmp, &path)?;
            } else {
                fs::rename_replace(&tmp, &path)?;
            }
            Ok(true)
        })
        .map_err(|_| StoreError::Coordination)?;
        match outcome {
            Ok(true) => *published = true,
            Ok(false) => {
                let _ = fs::remove(&tmp);
                return Err(StoreError::Conflict(ConflictKind::External).into());
            }
            Err(e) => {
                let _ = fs::remove(&tmp);
                return Err(if e.kind() == io::ErrorKind::AlreadyExists {
                    StoreError::Conflict(ConflictKind::Exists).into()
                } else {
                    StoreError::Storage.into()
                });
            }
        }
        self.hit(Step::Publish, When::After)?;
        self.durable_commit(op, p, &path, dir)
    }

    fn temp_of(&self, op: &OperationId, date: &DiaryDate) -> std::path::PathBuf {
        self.path_of(date)
            .with_file_name(format!(".{date}.md.{}.tmp", op.as_str()))
    }

    /// File and directory sync, read-back verification, the receipt transaction, cleanup.
    fn durable_commit(
        &self,
        op: &OperationId,
        p: &Prepared,
        path: &Path,
        dir: &Path,
    ) -> Result<(), Fail> {
        self.hit(Step::FileSync, When::Before)?;
        fs::sync_file(path).map_err(io_err)?;
        self.hit(Step::FileSync, When::After)?;
        self.hit(Step::DirSync, When::Before)?;
        fs::sync_path(dir).map_err(io_err)?;
        self.hit(Step::DirSync, When::After)?;
        self.hit(Step::Verify, When::Before)?;
        let seen = self.observe(path)?.ok_or(StoreError::Storage)?;
        if sha_hex(&seen.bytes) != p.new {
            return Err(StoreError::Conflict(ConflictKind::External).into());
        }
        self.hit(Step::Verify, When::After)?;
        self.hit(Step::Receipt, When::Before)?;
        let row = catalog::row_from(&p.date, &seen);
        let result = format!("{}:{}", p.seq, p.new);
        self.db
            .transact(|tx| {
                catalog::upsert(tx, &row)?;
                finish_tx(tx, op, OperationState::Succeeded, &result)
            })
            .map_err(|_| StoreError::Storage)?
            .map_err(ledger_err)?;
        self.hit(Step::Receipt, When::After)?;
        self.hit(Step::Ack, When::Before)?;
        let _ = fs::remove(&self.pending().join(format!("{}.md", op.as_str())));
        let _ = fs::remove(&self.temp_of(op, &p.date));
        self.hit(Step::Ack, When::After)
    }

    /// A non-crash failure after admission. Before publication the save simply failed; after it
    /// the outcome is unknown and only reconciliation (a retry of the same ID) may settle it.
    fn abandon(
        &self,
        op: &OperationId,
        p: &Prepared,
        stage: &Path,
        error: StoreError,
        published: bool,
    ) -> Result<SaveOutcome, Fail> {
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
            return Err(StoreError::Uncertain.into());
        }
        let tag = match &error {
            StoreError::Conflict(ConflictKind::Exists) => "exists",
            StoreError::Conflict(_) => "conflict",
            _ => "publish_failed",
        };
        self.finish_failed(op, p, tag, stage, false)?;
        Err(error.into())
    }

    fn finish_failed(
        &self,
        op: &OperationId,
        p: &Prepared,
        tag: &str,
        stage: &Path,
        keep_stage: bool,
    ) -> Result<(), Fail> {
        self.db
            .finish(op, OperationState::Failed, tag)
            .map_err(ledger_err)?;
        let _ = fs::remove(&self.temp_of(op, &p.date));
        if !keep_stage {
            let _ = fs::remove(stage);
        }
        Ok(())
    }

    /// The recorded outcome of an operation, reconciling an unfinished one first.
    fn settle(&self, record: Record, generation: u64) -> Result<SaveOutcome, Fail> {
        let record = if record.state.is_terminal() {
            record
        } else {
            self.reconcile(&record)?
        };
        match record.state {
            OperationState::Succeeded => {
                let (sequence, sha256) = record
                    .result
                    .as_deref()
                    .and_then(|r| r.split_once(':'))
                    .and_then(|(s, d)| Some((s.parse().ok()?, d.to_owned())))
                    .ok_or(StoreError::Storage)?;
                Ok(SaveOutcome {
                    operation_id: record.id,
                    revision: AuthenticRevision { sequence, sha256 },
                    buffer_generation: generation,
                })
            }
            OperationState::Failed | OperationState::Cancelled => {
                Err(failure(record.result.as_deref().unwrap_or("failed")).into())
            }
            _ => Err(StoreError::Uncertain.into()),
        }
    }

    /// Judges an unfinished save by the current bytes of its target and settles it, or leaves it
    /// unfinished when it cannot tell. Never restores over a changed file.
    pub(crate) fn reconcile(&self, record: &Record) -> Result<Record, Fail> {
        let op = &record.id;
        let fresh = || -> Result<Record, Fail> {
            Ok(self
                .db
                .get(op)
                .map_err(ledger_err)?
                .ok_or(StoreError::Storage)?)
        };
        let stage = self.pending().join(format!("{}.md", op.as_str()));
        let Some(p) = self
            .db
            .step_outcome(op, &step_id(op, "prepared"))
            .map_err(ledger_err)?
            .and_then(|s| decode(&s))
        else {
            self.db
                .finish(op, OperationState::Failed, "no_intent")
                .map_err(ledger_err)?;
            return fresh();
        };
        let path = self.path_of(&p.date);
        let dir = path.parent().ok_or(StoreError::Storage)?;
        let Ok(current) = self.observe(&path) else {
            return fresh(); // cannot judge; stays unfinished
        };
        let current_digest = current.as_ref().map(|o| sha_hex(&o.bytes));
        let unresolved = |result: Result<(), Fail>| -> Result<Record, Fail> {
            match result {
                Err(Fail::Crash) => Err(Fail::Crash),
                _ => fresh(),
            }
        };
        if current_digest.as_deref() == Some(p.new.as_str()) {
            // Published but not receipted: re-sync (the earlier sync may have failed) and commit.
            return unresolved(self.durable_commit(op, &p, &path, dir));
        }
        if current_digest == p.old {
            let staged = fs::read_bounded(&stage, MAX_FILE).ok().flatten();
            let Some(staged) = staged.filter(|s| sha_hex(&s.bytes) == p.new) else {
                self.finish_failed(op, &p, "stage_missing", &stage, false)?;
                return fresh();
            };
            if parse(&staged.bytes, &p.date).is_err() {
                self.finish_failed(op, &p, "stage_invalid", &stage, true)?;
                return fresh();
            }
            let mut published = false;
            return match self.publish(op, &p, &staged.bytes, &mut published) {
                Err(Fail::Err(StoreError::Conflict(_))) if !published => {
                    self.finish_failed(op, &p, "conflict", &stage, true)?;
                    fresh()
                }
                other => unresolved(other),
            };
        }
        // Neither the old nor the intended bytes: someone else changed the file. Keep both.
        self.finish_failed(op, &p, "conflict", &stage, true)?;
        fresh()
    }

    /// Removes stages whose operation is unknown or finished, and target-directory temporaries
    /// of operations that are not unfinished. Never touches a `.md` file in the tree.
    pub(crate) fn remove_orphans(&self) -> Result<usize, StoreError> {
        let mut removed = 0;
        if let Ok(entries) = std::fs::read_dir(self.pending()) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                let Some(id) = name.strip_suffix(".md").and_then(OperationId::parse) else {
                    continue;
                };
                let stale = match self.db.get(&id).map_err(ledger_err)? {
                    None => true,
                    Some(r) => r.state == OperationState::Succeeded,
                };
                if stale && entry.file_type().is_ok_and(|t| t.is_file()) {
                    fs::remove(&entry.path()).map_err(|_| StoreError::Storage)?;
                    removed += 1;
                }
            }
        }
        let unfinished: Vec<OperationId> = self
            .db
            .unfinished()
            .map_err(ledger_err)?
            .into_iter()
            .map(|r| r.id)
            .collect();
        for (_, year) in catalog::subdirs(&self.tree(), 4)? {
            for (_, month) in catalog::subdirs(&year, 2)? {
                let Ok(entries) = std::fs::read_dir(&month) else {
                    continue;
                };
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    let owned = name
                        .strip_prefix('.')
                        .and_then(|n| n.strip_suffix(".tmp"))
                        .and_then(|n| n.split_once(".md."))
                        .and_then(|(date, id)| {
                            DiaryDate::parse(date)?;
                            OperationId::parse(id)
                        });
                    if let Some(id) = owned
                        && !unfinished.contains(&id)
                        && entry.file_type().is_ok_and(|t| t.is_file())
                    {
                        fs::remove(&entry.path()).map_err(|_| StoreError::Storage)?;
                        removed += 1;
                    }
                }
            }
        }
        Ok(removed)
    }
}

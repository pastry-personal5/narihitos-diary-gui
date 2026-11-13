//! Export start: admission, step tokens and re-preview.
use super::*;

impl ExportCoordinatorState {
    /// A new step on an existing operation; the input hash is the operation's.
    pub(super) fn step(
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

    pub(super) fn park(
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

    pub(super) fn holder_blocks(
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

    pub(super) fn start(
        self: &Arc<Self>,
        session: &str,
        generation: u64,
        request: StartRequest,
    ) -> Result<Started, ExportError> {
        self.start_inner(session, generation, request, None)
    }

    pub(super) fn start_illustrated(
        self: &Arc<Self>,
        session: &str,
        generation: u64,
        request: IllustratedStartRequest,
    ) -> Result<Started, ExportError> {
        if !diary_domain::authentic::valid_uuid(&request.expected_version) {
            return Err(ExportError::Invalid);
        }
        let version = request.expected_version;
        let input = StartRequest {
            operation_id: request.operation_id,
            date: request.date,
            target: Target::Notion,
            expected: diary_domain::authentic::AuthenticRevision {
                sequence: 0,
                sha256: hex_sha(version.as_bytes()),
            },
        };
        self.start_inner(session, generation, input, Some(version))
    }

    fn start_inner(
        self: &Arc<Self>,
        session: &str,
        generation: u64,
        request: StartRequest,
        version: Option<String>,
    ) -> Result<Started, ExportError> {
        let kind = if version.is_some() {
            ContentKind::Illustrated
        } else {
            ContentKind::Authentic
        };
        let adapter = self.adapter(kind, request.target)?.clone();
        if !self.ready.load(Ordering::SeqCst) {
            return Err(ExportError::Storage);
        }
        if session.is_empty() || session.len() > 128 {
            return Err(ExportError::Invalid);
        }
        let key = self.key(&*adapter)?;
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
        // A saved operation still waiting for confirmation (e.g. from an earlier run) would hold
        // the destination forever: stop it so this export can proceed. Nothing was written for
        // it, and `uncertain`/`running` holders are left alone.
        if let Some((holder, OperationState::Paused)) =
            self.db.export_holder(&key).map_err(stored)?
        {
            self.cancel(&holder)?;
        }
        let snapshot = match &version {
            Some(v) => self.illustrated_snapshot(request.date, v)?,
            None => self.snapshot(&request)?,
        };
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
            content_kind: snapshot.content_kind,
            illustrated_version: snapshot.illustrated_version.clone(),
            photos: snapshot.photos.clone(),
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
        let projected = if kind == ContentKind::Illustrated {
            receipt
                .as_ref()
                .filter(|r| r.revision_sha == snapshot.revision.sha256)
                .map(|r| r.projection_digest.clone())
        } else {
            None
        };
        let (confirm, write) = match decide(
            receipt.as_ref(),
            &projected.unwrap_or_else(|| {
                Self::expected(&*adapter, &snapshot.bytes, &snapshot.revision.sha256)
            }),
            &seen,
            false,
        ) {
            Decision::Unavailable => {
                let why = Self::why(&seen);
                self.db
                    .finish_export(
                        &request.operation_id,
                        OperationState::Failed,
                        why.as_str(),
                        None,
                    )
                    .map_err(stored)?;
                self.notify(&request.operation_id);
                return Err(ExportError::CloudUnavailable(why));
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
                    (reason, write, false),
                )?;
                self.notify(&request.operation_id);
                return Ok(Started::Preview(preview));
            }
            (None, Some(write)) => {
                self.spawn(Job {
                    id: request.operation_id.clone(),
                    target: request.target,
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
    pub(super) fn repreview(
        self: &Arc<Self>,
        session: &str,
        generation: u64,
        rec: &Record,
        row: &Row,
        adapter: &dyn ExportAdapter,
    ) -> Result<Started, ExportError> {
        if self.key(adapter)? != row.destination_key {
            return Err(ExportError::Disabled);
        }
        let date = DiaryDate::parse(&row.date).ok_or(ExportError::Storage)?;
        let seen = adapter.observe(&date);
        let receipt = self
            .db
            .export_receipt(&row.destination_key, &row.authentic_id)
            .map_err(stored)?;
        let (unresolved, _) = self.holder_blocks(&row.destination_key, Some(&rec.id))?;
        let want = Self::row_expected(adapter, row);
        match decide(receipt.as_ref(), &want, &seen, unresolved) {
            Decision::Unavailable => Err(ExportError::CloudUnavailable(Self::why(&seen))),
            Decision::Uncertain => Err(ExportError::Uncertain),
            Decision::Noop => {
                self.succeed(&rec.id, row, &seen)?;
                self.notify(&rec.id);
                let (rec, row) = self.load(&rec.id)?;
                Ok(Started::Result(self.result(&rec, &row)))
            }
            Decision::Write(w) | Decision::Confirm(_, w) => {
                let reason = match decide(receipt.as_ref(), &want, &seen, false) {
                    Decision::Confirm(r, _) => r,
                    _ => Reason::Changed,
                };
                Ok(Started::Preview(self.issue_preview(
                    session,
                    generation,
                    &rec.id,
                    rec.checkpoint,
                    &seen,
                    (reason, w, false),
                )?))
            }
        }
    }
}

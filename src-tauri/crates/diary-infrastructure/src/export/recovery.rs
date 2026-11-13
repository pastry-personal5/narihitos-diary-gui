//! Abandoning uncertain exports and crash recovery.
use super::*;

impl ExportCoordinatorState {
    /// A read-back equal to the saved revision proves the write and resolves the export;
    /// otherwise the user may abandon it after a fresh preview. The remote is never touched.
    pub(super) fn abandon(
        &self,
        session: &str,
        generation: u64,
        id: &OperationId,
    ) -> Result<Started, ExportError> {
        let (rec, row) = self.load(id)?;
        if rec.state != OperationState::Uncertain {
            return Err(ExportError::CheckpointMismatch);
        }
        let adapter = self.adapter(row.content_kind, row.target)?.clone();
        if self.key(&*adapter)? != row.destination_key || !self.ready.load(Ordering::SeqCst) {
            return Err(ExportError::Disabled);
        }
        let date = DiaryDate::parse(&row.date).ok_or(ExportError::Storage)?;
        let back = adapter.read_back(&date);
        if !back.available {
            return Err(ExportError::CloudUnavailable(Self::why(&back)));
        }
        if back.present
            && adapter.verified(&date, &back)
            && back.digest == Self::row_expected(&*adapter, &row)
        {
            self.succeed(id, &row, &back)?;
            self.notify(id);
            let (rec, row) = self.load(id)?;
            return Ok(Started::Result(self.result(&rec, &row)));
        }
        Ok(Started::Preview(self.issue_preview(
            session,
            generation,
            id,
            rec.checkpoint,
            &back,
            (Reason::Uncertain, Write::Replace, true),
        )?))
    }

    pub(super) fn abandon_confirm(
        &self,
        session: &str,
        generation: u64,
        request: ConfirmRequest,
    ) -> Result<ExportResult, ExportError> {
        if request.expected_checkpoint == 0
            || request.expected_checkpoint > i64::MAX as u64 - 1
            || request.token.is_empty()
            || request.token.len() > 128
        {
            return Err(ExportError::Invalid);
        }
        let (rec, row) = self.load(&request.operation_id)?;
        let hash = hash_input(
            "export_abandon",
            1,
            &[
                ("operation", &rec.input_hash),
                ("checkpoint", &request.expected_checkpoint.to_string()),
            ],
        );
        // A duplicate answers from the recorded step.
        if self
            .db
            .export_step(&request.operation_id, request.step_id.as_str(), &hash)
            .map_err(stored)?
            .is_some()
        {
            return Ok(self.result(&rec, &row));
        }
        if rec.state != OperationState::Uncertain || rec.checkpoint != request.expected_checkpoint {
            return Err(ExportError::CheckpointMismatch);
        }
        let adapter = self.adapter(row.content_kind, row.target)?.clone();
        if self.key(&*adapter)? != row.destination_key {
            return Err(ExportError::Disabled);
        }
        let fingerprint_seen = {
            let previews = self.previews.lock().map_err(|_| ExportError::Storage)?;
            let p = previews.get(&request.token).ok_or(ExportError::Expired)?;
            if p.expires <= (self.clock)() {
                return Err(ExportError::Expired);
            }
            if !p.abandon
                || p.session != session
                || p.generation != generation
                || p.operation != request.operation_id
                || p.checkpoint != request.expected_checkpoint
            {
                return Err(ExportError::PreviewStale);
            }
            p.fingerprint.clone()
        };
        let date = DiaryDate::parse(&row.date).ok_or(ExportError::Storage)?;
        let back = adapter.read_back(&date);
        if !back.available {
            return Err(ExportError::CloudUnavailable(Self::why(&back)));
        }
        if back.fingerprint != fingerprint_seen {
            return Err(ExportError::PreviewStale);
        }
        let step = NewStep {
            step_id: request.step_id.clone(),
            input_hash: hash,
            expected_checkpoint: request.expected_checkpoint,
            outcome: "abandon_admitted".into(),
            state: Some(OperationState::Running),
        };
        self.db
            .confirm_export(
                &request.operation_id,
                &step,
                &hex_sha(request.token.as_bytes()),
            )
            .map_err(stored)?;
        if let Ok(mut previews) = self.previews.lock() {
            previews.remove(&request.token);
        }
        // A crash between the two writes leaves `running`; recovery re-proves or re-parks it.
        self.db
            .finish_export(
                &request.operation_id,
                OperationState::Cancelled,
                "abandoned",
                None,
            )
            .map_err(stored)?;
        self.notify(&request.operation_id);
        let (rec, row) = self.load(&request.operation_id)?;
        Ok(self.result(&rec, &row))
    }

    pub(super) fn recover(&self) -> Result<usize, ExportError> {
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
                    .adapter(row.content_kind, row.target)
                    .ok()
                    .filter(|a| self.key(&***a).is_ok_and(|k| k == row.destination_key))
                    .and_then(|a| {
                        let date = DiaryDate::parse(&row.date)?;
                        let back = a.read_back(&date);
                        (a.verified(&date, &back)
                            && back.available
                            && back.present
                            && back.digest == Self::row_expected(&**a, &row))
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

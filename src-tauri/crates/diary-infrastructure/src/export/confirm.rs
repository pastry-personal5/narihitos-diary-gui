//! External-change confirmation and success recording.
use super::*;

impl ExportCoordinatorState {
    pub(super) fn confirm(
        self: &Arc<Self>,
        session: &str,
        generation: u64,
        request: ConfirmRequest,
    ) -> Result<Admitted, ExportError> {
        if request.expected_checkpoint == 0
            || request.expected_checkpoint > i64::MAX as u64 - 1
            || request.token.is_empty()
            || request.token.len() > 128
        {
            return Err(ExportError::Invalid);
        }
        let (rec, row) = self.load(&request.operation_id)?;
        let hash = hash_input(
            "export_confirm",
            1,
            &[
                ("operation", &rec.input_hash),
                ("checkpoint", &request.expected_checkpoint.to_string()),
            ],
        );
        // A duplicate answers from the recorded step, before token, expiry or checkpoint checks.
        if let Some(checkpoint) = self
            .db
            .export_step(&request.operation_id, request.step_id.as_str(), &hash)
            .map_err(stored)?
        {
            return Ok(Admitted {
                operation_id: request.operation_id,
                step_id: request.step_id,
                admitted_checkpoint: checkpoint,
            });
        }
        let adapter = self.adapter(row.content_kind, row.target)?.clone();
        if self.key(&*adapter)? != row.destination_key {
            return Err(ExportError::Disabled);
        }
        if !self.ready.load(Ordering::SeqCst) {
            return Err(ExportError::Storage);
        }
        match rec.state {
            OperationState::Uncertain => return Err(ExportError::Uncertain),
            OperationState::Paused if rec.checkpoint == request.expected_checkpoint => {}
            _ => return Err(ExportError::CheckpointMismatch),
        }
        let prepared = {
            let previews = self.previews.lock().map_err(|_| ExportError::Storage)?;
            let p = previews.get(&request.token).ok_or(ExportError::Expired)?;
            if p.expires <= (self.clock)() {
                return Err(ExportError::Expired);
            }
            if p.session != session
                || p.generation != generation
                || p.operation != request.operation_id
                || p.checkpoint != request.expected_checkpoint
            {
                return Err(ExportError::PreviewStale);
            }
            if p.abandon {
                return Err(ExportError::PreviewStale);
            }
            (p.fingerprint.clone(), p.write)
        };
        let date = DiaryDate::parse(&row.date).ok_or(ExportError::Storage)?;
        let seen = adapter.observe(&date);
        if !seen.available {
            return Err(ExportError::CloudUnavailable(Self::why(&seen)));
        }
        if seen.fingerprint != prepared.0 {
            return Err(ExportError::PreviewStale);
        }
        let snapshot_bytes = row.snapshot.clone().ok_or(ExportError::Storage)?;
        let step = NewStep {
            step_id: request.step_id.clone(),
            input_hash: hash,
            expected_checkpoint: request.expected_checkpoint,
            outcome: format!("admitted:{}", request.expected_checkpoint + 1),
            state: Some(OperationState::Running),
        };
        let fingerprint = hex_sha(request.token.as_bytes());
        let admission = self
            .db
            .confirm_export(&request.operation_id, &step, &fingerprint)
            .map_err(stored)?;
        if let StepOutcome::Recorded { checkpoint, .. } = admission {
            return Ok(Admitted {
                operation_id: request.operation_id,
                step_id: request.step_id,
                admitted_checkpoint: checkpoint,
            });
        }
        if let Ok(mut previews) = self.previews.lock() {
            previews.remove(&request.token);
        }
        self.notify(&request.operation_id);
        self.spawn(Job {
            id: request.operation_id.clone(),
            target: row.target,
            snapshot: Snapshot {
                content_kind: row.content_kind,
                illustrated_version: row.illustrated_version.clone(),
                photos: self
                    .db
                    .export_photos(&request.operation_id)
                    .map_err(stored)?,
                date,
                authentic_id: row.authentic_id.clone(),
                revision: diary_domain::authentic::AuthenticRevision {
                    sequence: row.revision_sequence,
                    sha256: row.revision_sha.clone(),
                },
                bytes: snapshot_bytes,
            },
            write: prepared.1,
            expected: seen,
        });
        Ok(Admitted {
            operation_id: request.operation_id,
            step_id: request.step_id,
            admitted_checkpoint: request.expected_checkpoint + 1,
        })
    }

    pub(super) fn succeed(
        &self,
        id: &OperationId,
        row: &Row,
        seen: &Observation,
    ) -> Result<(), ExportError> {
        let (receipt, key, authentic_id) = Self::publication(row, &row.revision_sha, seen);
        self.db
            .finish_export(
                id,
                OperationState::Succeeded,
                "export",
                Some(Publication {
                    destination_key: &key,
                    authentic_id: &authentic_id,
                    date: &row.date,
                    receipt: &receipt,
                }),
            )
            .map_err(stored)
    }
}

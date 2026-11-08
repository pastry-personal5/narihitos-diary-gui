//! The background export step: spawn, run and cancel.
use super::*;

impl ExportCoordinatorState {
    pub(super) fn spawn(self: &Arc<Self>, job: Job) {
        let inner = self.clone();
        std::thread::spawn(move || {
            // An error leaves the durable state for recovery; nothing is replayed here.
            let _ = inner.run(&job);
            if let Ok(mut cancels) = inner.cancels.lock() {
                cancels.remove(job.id.as_str());
            }
            inner.notify(&job.id);
        });
    }

    pub(super) fn run(&self, job: &Job) -> Result<(), ExportError> {
        self.hit("after_admission")?;
        let adapter = self.adapter(job.target)?.clone();
        let date = job.snapshot.date;
        // Re-read immediately before mutation; a changed observation needs a new preview.
        let now = adapter.observe(&date);
        let stop = self
            .cancels
            .lock()
            .map(|mut c| c.remove(job.id.as_str()))
            .unwrap_or(false);
        if stop {
            // Nothing was applied: cancelling is a clean terminal state with no rollback.
            self.db
                .export_chunk(&job.id, "failed", None)
                .map_err(stored)?;
            return self
                .db
                .finish_export(&job.id, OperationState::Cancelled, "cancelled", None)
                .map_err(stored);
        }
        if !now.available
            || now.present != job.expected.present
            || now.fingerprint != job.expected.fingerprint
        {
            return self.park(
                &job.id,
                OperationState::Paused,
                "parked",
                Some(if now.available {
                    "changed"
                } else {
                    "unavailable"
                }),
            );
        }
        self.db
            .export_chunk(&job.id, "applying", None)
            .map_err(stored)?;
        self.hit("before_apply")?;
        let applied = adapter.apply(&date, &job.snapshot.bytes, job.write, &job.expected);
        self.hit("after_apply")?;
        if applied == Applied::Failed {
            self.db
                .export_chunk(&job.id, "failed", None)
                .map_err(stored)?;
            return self
                .db
                .finish_export(&job.id, OperationState::Failed, "apply_failed", None)
                .map_err(stored);
        }
        let back = adapter.read_back(&date);
        let sha = &Self::expected(
            &*adapter,
            &job.snapshot.bytes,
            &job.snapshot.revision.sha256,
        );
        if applied == Applied::Done && back.available && back.present && &back.digest == sha
            || applied == Applied::Unknown && back.available && back.present && &back.digest == sha
        {
            self.hit("before_receipt")?;
            let (_, row) = self.load(&job.id)?;
            self.db
                .export_chunk(&job.id, "done", Some(&back.ids))
                .map_err(stored)?;
            if adapter.warnings(&job.snapshot.bytes) > 0 {
                self.db
                    .set_export_reason(&job.id, "projection_warnings")
                    .map_err(stored)?;
            }
            return self.succeed(&job.id, &row, &back);
        }
        // Ambiguous: no definitive evidence either way. Never replay.
        self.db
            .export_chunk(&job.id, "unknown", None)
            .map_err(stored)?;
        self.park(
            &job.id,
            OperationState::Uncertain,
            "uncertain",
            Some("uncertain"),
        )
    }

    pub(super) fn cancel(&self, id: &OperationId) -> Result<ExportResult, ExportError> {
        let (rec, row) = self.load(id)?;
        match rec.state {
            OperationState::Paused => {
                self.db
                    .finish_export(id, OperationState::Cancelled, "cancelled", None)
                    .map_err(stored)?;
                self.notify(id);
            }
            OperationState::Running => {
                // Read by `run` before `apply`; past that point the export finishes by read-back.
                if let Ok(mut c) = self.cancels.lock() {
                    c.insert(id.as_str().to_owned());
                }
            }
            OperationState::Uncertain => return Err(ExportError::Uncertain),
            _ => return Ok(self.result(&rec, &row)),
        }
        let (rec, row) = self.load(id)?;
        Ok(self.result(&rec, &row))
    }
}

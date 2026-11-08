//! Saved-revision snapshot and session-bound preview issuing.
use super::*;

impl ExportCoordinatorState {
    pub(super) fn snapshot(&self, request: &StartRequest) -> Result<Snapshot, ExportError> {
        match self.authentic.load(&request.date) {
            Ok(Loaded::File {
                revision,
                header,
                body,
            }) => {
                if revision != request.expected {
                    return Err(ExportError::StaleRevision);
                }
                let bytes = render(&header, &body).map_err(|_| ExportError::Storage)?;
                if bytes.len() > MAX_SNAPSHOT_BYTES {
                    return Err(ExportError::Limit);
                }
                if hex_sha(&bytes) != revision.sha256 {
                    return Err(ExportError::Storage);
                }
                Ok(Snapshot {
                    date: request.date,
                    authentic_id: header.id.clone(),
                    revision,
                    bytes,
                })
            }
            Ok(Loaded::Virtual(_)) => Err(ExportError::NothingToExport),
            Err(_) => Err(ExportError::Storage),
        }
    }

    pub(super) fn issue_preview(
        &self,
        session: &str,
        generation: u64,
        id: &OperationId,
        checkpoint: u64,
        observed: &Observation,
        (reason, write, abandon): (Reason, Write, bool),
    ) -> Result<Preview, ExportError> {
        let token = format!("{}{}", uuid()?, uuid()?);
        let now = (self.clock)();
        let mut previews = self.previews.lock().map_err(|_| ExportError::Storage)?;
        previews
            .retain(|_, p| p.session == session && p.generation == generation && p.expires > now);
        if previews.len() >= MAX_PREVIEWS {
            return Err(ExportError::Limit);
        }
        previews.insert(
            token.clone(),
            Prepared {
                operation: id.clone(),
                session: session.into(),
                generation,
                checkpoint,
                fingerprint: observed.fingerprint.clone(),
                write,
                abandon,
                expires: now.saturating_add(TOKEN_TTL_MS),
            },
        );
        Ok(Preview {
            operation_id: id.clone(),
            expected_checkpoint: checkpoint,
            token,
            observed_fingerprint: observed.fingerprint.clone(),
            reason,
            write,
            expires_in_ms: TOKEN_TTL_MS,
        })
    }
}

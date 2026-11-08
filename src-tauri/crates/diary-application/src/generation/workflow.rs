//! Transport-independent generation preflight, capture and admission.
use super::*;
use crate::review::Reviews;
use std::hash::{Hash, Hasher};

pub struct StartRequest {
    pub request_id: String,
    pub date: DiaryDate,
    pub selections: Vec<Selection>,
    pub profile: String,
    pub locale: Locale,
    pub count: u8,
}
pub struct CaptureServices<'a> {
    pub profiles: &'a dyn ProfileDirectory,
    pub runner: &'a dyn AgentRunner,
    pub imports: &'a dyn ImportStore,
    pub authentic: &'a dyn AuthenticStore,
    pub configuration: &'a dyn Fn() -> u64,
    pub cancelled: &'a dyn Fn() -> bool,
}
impl StartRequest {
    /// Idempotency fingerprint only; preserves the existing non-cryptographic wire identity.
    pub fn fingerprint(&self) -> String {
        let mut pairs: Vec<_> = self
            .selections
            .iter()
            .map(|s| (&s.import_id, &s.source_id))
            .collect();
        pairs.sort();
        (0u8..4)
            .map(|k| {
                let mut h = std::collections::hash_map::DefaultHasher::new();
                (
                    k,
                    &self.date.to_string(),
                    &pairs,
                    &self.profile,
                    self.locale.as_str(),
                    self.count,
                )
                    .hash(&mut h);
                format!("{:016x}", h.finish())
            })
            .collect()
    }
    /// Replay precedes fresh source/profile checks. Call under the coordinator-before-store locks.
    pub fn preflight(
        &self,
        coordinator: &mut Coordinator,
        store: &Store,
        reviews: &Reviews,
        session: &str,
        now: u64,
    ) -> Result<Option<Job>, Error> {
        if let Some(job) =
            coordinator.lookup_request(session, now, &self.request_id, &self.fingerprint())?
        {
            store.get(session, &job.id).ok_or(Error::NotFound)?;
            return Ok(Some(job));
        }
        if coordinator.closing(session, now).unwrap_or(true)
            || coordinator
                .active_agent(session, now)
                .is_ok_and(|j| j.is_some())
        {
            return Err(Error::Busy);
        }
        if reviews.has_date(session, &self.date) {
            return Err(Error::ReviewActive);
        }
        if self.selections.is_empty() {
            return Err(Error::EmptySelection);
        }
        Ok(None)
    }
    /// Blocking capture is scheduled by the runtime adapter, without coordinator/store locks.
    pub fn capture(self, services: CaptureServices<'_>) -> Result<GenerationContext, Error> {
        let view = services
            .profiles
            .check(&self.profile)
            .ok_or(Error::ProfileNotReady)?;
        if view.status != Some(ProfileStatus::Ready) || view.generate != Capability::Ready {
            return Err(Error::ProfileNotReady);
        }
        let generation = (services.configuration)();
        let context = capture(
            Capture {
                prompt: services
                    .runner
                    .resolve(PromptSelection {
                        provider: view.kind,
                        action: PromptAction::GenerateCandidate,
                    })
                    .map_err(|_| Error::ProfileNotReady)?,
                date: self.date,
                selections: self.selections,
                profile: ProfileHandle {
                    alias: view.alias,
                    kind: view.kind,
                },
                locale: self.locale,
                count: self.count,
                config_generation: generation,
            },
            services.imports,
            services.authentic,
            services.cancelled,
        )?;
        if (services.configuration)() != generation {
            return Err(Error::ChangedSource);
        }
        Ok(context)
    }
}

pub struct StartedGeneration {
    pub job: Job,
    pub generation: Shared,
    pub execute: bool,
}
impl Store {
    /// Commit after blocking capture; reuses a simultaneous winner without scheduling it twice.
    pub fn start(
        &mut self,
        coordinator: &mut Coordinator,
        admission: Admission<'_>,
        context: GenerationContext,
    ) -> Result<StartedGeneration, Error> {
        let (expected, session, now) = (admission.job_id, admission.session, admission.now);
        let (job, generation) = self.admit(coordinator, admission, context)?;
        let execute = job.id == expected;
        let job = if execute {
            coordinator.transition(session, now, &job.id, JobState::Running)?
        } else {
            job
        };
        Ok(StartedGeneration {
            job,
            generation,
            execute,
        })
    }
    pub fn finish(
        &self,
        coordinator: &mut Coordinator,
        session: &str,
        now: u64,
        job_id: &str,
        generation: &Shared,
    ) -> Result<Job, Error> {
        let state = generation
            .lock()
            .expect("generation poisoned")
            .terminal
            .ok_or(Error::Busy)?;
        let job = coordinator.transition(session, now, job_id, state)?;
        // Keep terminal acknowledgement authoritative even if result accounting cannot settle.
        let _ = self.settle(coordinator, session, now, job_id);
        Ok(job)
    }
}

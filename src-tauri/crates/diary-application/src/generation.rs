//! Session-owned generation: fresh source capture, immutable context, one sequential job and
//! explicit transient-result retention. No filesystem, serialization, prompt prose or logging.
mod workflow;
pub use workflow::{CaptureServices, StartRequest, StartedGeneration};

use crate::{
    Coordinator, Job, JobState,
    agent::*,
    authentic::{AuthenticStore, Loaded},
    imports::ImportStore,
};
use diary_domain::{
    authentic::{AuthenticRevision, DiaryDate},
    source::window::utc_date,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
};

pub const MAX_JOBS: usize = 5;
pub const MAX_STORE_BYTES: usize = 8 * 1024 * 1024;
const HEADER_BYTES: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    EmptySelection,
    ReviewActive,
    ChangedSource,
    InvalidInput,
    Context,
    Unavailable,
    ProfileNotReady,
    DiscardRequired,
    NotFound,
    Busy,
    Coordination(crate::Error),
}
impl From<crate::Error> for Error {
    fn from(e: crate::Error) -> Self {
        Self::Coordination(e)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Selection {
    pub import_id: String,
    pub source_id: String,
}

/// Body-bearing context intentionally has no Debug implementation.
#[derive(Clone)]
pub struct GenerationContext {
    date: DiaryDate,
    profile: ProfileHandle,
    prompt: ResolvedPrompt,
    locale: Locale,
    count: u8,
    baseline: Option<AuthenticRevision>,
    items: Vec<SnippetItem>,
}
impl GenerationContext {
    pub fn date(&self) -> DiaryDate {
        self.date
    }
    pub fn prompt(&self) -> &ResolvedPrompt {
        &self.prompt
    }
    pub fn profile(&self) -> &ProfileHandle {
        &self.profile
    }
    pub fn locale(&self) -> &Locale {
        &self.locale
    }
    pub fn count(&self) -> u8 {
        self.count
    }
    pub fn baseline(&self) -> Option<&AuthenticRevision> {
        self.baseline.as_ref()
    }

    pub fn packet(&self, index: usize) -> Result<GenerationPacket, AgentError> {
        let style = Style::ALL
            .get(index)
            .copied()
            .ok_or(AgentErrorCode::Context)?;
        GenerationPacket::new(style, self.date, self.items.clone())
    }
    fn bytes(&self) -> usize {
        HEADER_BYTES
            + self.prompt.retained_bytes()
            + self
                .items
                .iter()
                .map(|s| {
                    s.id.len()
                        + s.instant_utc.len()
                        + s.civil_date.len()
                        + s.offset.len()
                        + s.body.len()
                        + 256
                })
                .sum::<usize>()
    }
}
pub struct Capture {
    pub prompt: ResolvedPrompt,
    pub date: DiaryDate,
    pub selections: Vec<Selection>,
    pub profile: ProfileHandle,
    pub locale: Locale,
    pub count: u8,
    pub config_generation: u64,
}
/// Resolves only published eligible rows, then reads their exact observed bytes afresh.
/// All body locks are released before this value can reach an AgentRunner.
pub fn capture(
    input: Capture,
    imports: &dyn ImportStore,
    authentic: &dyn AuthenticStore,
    cancelled: &dyn Fn() -> bool,
) -> Result<GenerationContext, Error> {
    if input.prompt.selection()
        != (PromptSelection {
            provider: input.profile.kind,
            action: PromptAction::GenerateCandidate,
        })
    {
        return Err(Error::ProfileNotReady);
    }
    if !(1..=5).contains(&input.count) {
        return Err(Error::InvalidInput);
    }
    if input.selections.is_empty() {
        return Err(Error::EmptySelection);
    }
    let mut seen = BTreeSet::new();
    let mut rows = Vec::new();
    let mut bytes = HEADER_BYTES;
    for selection in input.selections {
        // Imports are observations, not separate sources: stable IDs may recur across scans.
        if !seen.insert(selection.source_id.clone()) {
            return Err(Error::InvalidInput);
        }
        if cancelled() {
            return Err(Error::Unavailable);
        }
        let listing = imports
            .list(&selection.import_id, cancelled)
            .map_err(|_| Error::ChangedSource)?;
        if listing.context.date != input.date
            || listing.context.config_generation != input.config_generation
        {
            return Err(Error::ChangedSource);
        }
        let Some(source) = listing
            .included
            .iter()
            .find(|s| s.source.id == selection.source_id)
        else {
            // A known conflict/unknown-date row is excluded; arbitrary IDs are refused.
            if listing.warnings.iter().any(|s| s.id == selection.source_id) {
                continue;
            }
            return Err(Error::ChangedSource);
        };
        let local = listing
            .navigation
            .as_ref()
            .and_then(|n| n.sources.get(&selection.source_id))
            .and_then(|s| s.included.as_ref())
            .ok_or(Error::ChangedSource)?;
        let view = imports
            .read(
                &selection.import_id,
                &selection.source_id,
                input.config_generation,
                cancelled,
            )
            .map_err(|_| Error::ChangedSource)?;
        if view.listing.token != source.source.token {
            return Err(Error::ChangedSource);
        }
        bytes += view.body.len() + 256;
        if bytes > MAX_PACKET_BYTES {
            return Err(Error::Context);
        }
        let t = source.instant.unix_ms;
        let date = utc_date(t).map_err(|_| Error::ChangedSource)?;
        let seconds = t.div_euclid(1000).rem_euclid(86400);
        let instant = format!(
            "{date}T{:02}:{:02}:{:02}.{:03}Z",
            seconds / 3600,
            seconds / 60 % 60,
            seconds % 60,
            t.rem_euclid(1000)
        );
        let offset = local.offset_seconds;
        let magnitude = offset.unsigned_abs();
        let offset = if magnitude.is_multiple_of(60) {
            format!(
                "{}{:02}:{:02}",
                if offset < 0 { '-' } else { '+' },
                magnitude / 3600,
                magnitude / 60 % 60
            )
        } else {
            format!(
                "{}{:02}:{:02}:{:02}",
                if offset < 0 { '-' } else { '+' },
                magnitude / 3600,
                magnitude / 60 % 60,
                magnitude % 60
            )
        };
        rows.push((
            t,
            source.source.token.key.clone(),
            selection.source_id,
            SnippetItem {
                id: String::new(),
                instant_utc: instant,
                civil_date: local.date.to_string(),
                offset,
                body: view.body,
            },
        ));
    }
    if rows.is_empty() {
        return Err(Error::EmptySelection);
    }
    rows.sort_by(|a, b| (&a.0, &a.1, &a.2).cmp(&(&b.0, &b.1, &b.2)));
    let items = rows
        .into_iter()
        .enumerate()
        .map(|(i, (_, _, _, mut item))| {
            item.id = format!("snippet-{}", i + 1);
            item
        })
        .collect();
    let baseline = match authentic
        .load(&input.date)
        .map_err(|_| Error::Unavailable)?
    {
        Loaded::Virtual(_) => None,
        Loaded::File { revision, .. } => Some(revision),
    };
    if cancelled() {
        return Err(Error::Unavailable);
    }
    Ok(GenerationContext {
        prompt: input.prompt,
        date: input.date,
        profile: input.profile,
        locale: input.locale,
        count: input.count,
        baseline,
        items,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    NotStarted,
    Running,
    Ready,
    Failed(AgentErrorCode),
    Cancelled,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Counts {
    pub requested: u8,
    pub finished: u8,
    pub ready: u8,
    pub failed: u8,
    pub cancelled: u8,
    pub not_started: u8,
}

pub struct Generation {
    pub job_id: String,
    pub request_id: String,
    pub session: String,
    pub context: GenerationContext,
    pub run: Run,
    pub shared: crate::run::Shared,
    pub slots: Vec<Slot>,
    pub candidates: Vec<Option<Candidate>>,
    pub terminal: Option<JobState>,
    pub error: Option<AgentErrorCode>,
    pub candidate_index: Option<u8>,
    pub started_at: u64,
    pub finished_at: Option<u64>,
    reservation: usize,
    output_bytes: usize,
}
impl Generation {
    pub fn counts(&self) -> Counts {
        let mut c = Counts {
            requested: self.context.count,
            ..Default::default()
        };
        for s in &self.slots {
            match s {
                Slot::NotStarted => c.not_started += 1,
                Slot::Ready => {
                    c.ready += 1;
                    c.finished += 1;
                }
                Slot::Failed(_) => {
                    c.failed += 1;
                    c.finished += 1;
                }
                Slot::Cancelled => {
                    c.cancelled += 1;
                    c.finished += 1;
                }
                Slot::Running => {}
            }
        }
        c
    }
    pub fn bytes(&self) -> usize {
        self.context.bytes() + self.output_bytes
    }
    pub fn cancel(&self) -> bool {
        self.terminal.is_none() && self.run.request_cancel()
    }
    pub fn cancellation_requested(&self) -> bool {
        self.run.token().is_cancelled()
    }
    pub fn candidate_id(&self, index: usize) -> String {
        format!("{}-{}", self.job_id, index + 1)
    }
}
pub type Shared = Arc<Mutex<Generation>>;
#[derive(Default)]
pub struct Store {
    jobs: BTreeMap<String, Shared>,
}
pub struct Admission<'a> {
    pub session: &'a str,
    pub now: u64,
    pub request_id: &'a str,
    pub digest: &'a str,
    pub job_id: &'a str,
}
impl Store {
    pub fn is_empty(&self) -> bool {
        self.jobs.is_empty()
    }
    pub fn get(&self, session: &str, job_id: &str) -> Option<Shared> {
        self.jobs
            .get(job_id)
            .filter(|g| g.lock().expect("generation poisoned").session == session)
            .cloned()
    }
    pub fn admit(
        &mut self,
        coordinator: &mut Coordinator,
        a: Admission<'_>,
        frozen: GenerationContext,
    ) -> Result<(Job, Shared), Error> {
        if let Some(job) = coordinator.lookup_request(a.session, a.now, a.request_id, a.digest)? {
            let existing = self.get(a.session, &job.id).ok_or(Error::NotFound)?;
            return Ok((job, existing));
        }
        if coordinator.active_agent(a.session, a.now)?.is_some() {
            return Err(Error::Coordination(crate::Error::Busy));
        }
        if self.jobs.len() >= MAX_JOBS {
            return Err(Error::DiscardRequired);
        }
        let used: usize = self
            .jobs
            .values()
            .map(|g| g.lock().expect("generation poisoned").reservation)
            .sum();
        let reservation = MAX_STORE_BYTES.saturating_sub(used);
        // Reserve the frozen input, request clone, one preview, and at least one draft before inference.
        if reservation < frozen.bytes() * 2 + MAX_TEXT_BYTES * 2 {
            return Err(Error::DiscardRequired);
        }
        let job = coordinator.admit_agent(
            a.session,
            a.now,
            a.request_id,
            a.digest,
            a.job_id,
            reservation,
        )?;
        let g = Arc::new(Mutex::new(Generation {
            job_id: job.id.clone(),
            request_id: a.request_id.into(),
            session: a.session.into(),
            slots: vec![Slot::NotStarted; frozen.count as usize],
            candidates: vec![None; frozen.count as usize],
            context: frozen,
            run: Run::new(),
            shared: Default::default(),
            terminal: None,
            error: None,
            candidate_index: None,
            started_at: a.now,
            finished_at: None,
            reservation,
            output_bytes: 0,
        }));
        self.jobs.insert(job.id.clone(), g.clone());
        Ok((job, g))
    }
    /// Called only after the coordinator records terminal acknowledgement.
    pub fn settle(
        &self,
        coordinator: &mut Coordinator,
        session: &str,
        now: u64,
        job_id: &str,
    ) -> Result<(), Error> {
        let g = self.get(session, job_id).ok_or(Error::NotFound)?;
        let mut g = g.lock().expect("generation poisoned");
        if g.terminal.is_none() {
            return Err(Error::Busy);
        }
        let bytes = g.bytes();
        coordinator.settle_result(session, now, job_id, bytes)?;
        g.reservation = bytes;
        Ok(())
    }
    pub fn discard(
        &mut self,
        coordinator: &mut Coordinator,
        session: &str,
        now: u64,
        job_id: &str,
    ) -> Result<(), Error> {
        let g = self.get(session, job_id).ok_or(Error::NotFound)?;
        if g.lock().expect("generation poisoned").terminal.is_none()
            || !coordinator.get(session, now, job_id)?.state.is_terminal()
        {
            return Err(Error::Busy);
        }
        coordinator.settle_result(session, now, job_id, 0)?;
        self.jobs.remove(job_id);
        Ok(())
    }
    pub fn cancel_job(&self, session: &str, job_id: &str) -> bool {
        self.get(session, job_id)
            .is_some_and(|g| g.lock().expect("generation poisoned").cancel())
    }
    pub fn clear(&mut self) {
        for g in self.jobs.values() {
            g.lock().expect("generation poisoned").cancel();
        }
        self.jobs.clear();
    }
}

fn candidate_bytes(c: &Candidate) -> usize {
    c.draft_markdown.len()
        + c.claim_support
            .iter()
            .map(|c| c.claim.len() + c.snippet_ids.iter().map(|s| s.len() + 24).sum::<usize>() + 64)
            .sum::<usize>()
        + c.warnings.iter().map(|s| s.len() + 24).sum::<usize>()
        + 256
}
/// One reservation spans this entire loop. No content/store lock is held while the adapter runs.
/// `notify` observes slot changes after commit; it may cancel from any phase including a gap.
pub fn execute(
    generation: &Shared,
    runner: &dyn AgentRunner,
    clock: &dyn Fn() -> u64,
    notify: &mut dyn FnMut(),
) {
    let count = generation
        .lock()
        .expect("generation poisoned")
        .context
        .count;
    for index in 0..count as usize {
        let next = {
            let mut g = generation.lock().expect("generation poisoned");
            if g.cancellation_requested() {
                None
            } else {
                g.slots[index] = Slot::Running;
                g.candidate_index = Some(index as u8 + 1);
                let request = AgentRequest::new(
                    g.context.profile.clone(),
                    g.context.locale.clone(),
                    Mode::Candidate {
                        index: index as u8 + 1,
                        count,
                        packet: g.context.packet(index).expect("validated count"),
                    },
                );
                *g.shared.lock().expect("snapshot poisoned") = crate::run::Snapshot::default();
                Some((
                    request,
                    g.context.prompt.clone(),
                    Run::from(g.run.token()),
                    g.shared.clone(),
                ))
            }
        };
        let Some((request, prompt, run, shared)) = next else {
            break;
        };
        notify();
        let result = prompt.prepare(request).and_then(|prepared| {
            crate::run::execute(runner, prepared, &run, &shared, clock, notify)
        });
        {
            let mut g = generation.lock().expect("generation poisoned");
            let result = if g.cancellation_requested() {
                Err(AgentErrorCode::Cancelled.into())
            } else {
                result
            };
            match result {
                Ok(AgentOutput::Candidate(c)) => {
                    let bytes = candidate_bytes(&c);
                    if g.context.bytes() * 2 + MAX_TEXT_BYTES + g.output_bytes + bytes
                        <= g.reservation
                    {
                        g.output_bytes += bytes;
                        g.candidates[index] = Some(c);
                        g.slots[index] = Slot::Ready;
                    } else {
                        g.slots[index] = Slot::Failed(AgentErrorCode::Context);
                        g.error = Some(AgentErrorCode::Context);
                    }
                }
                Err(AgentError(AgentErrorCode::Cancelled)) => {
                    g.run.request_cancel();
                    g.slots[index] = Slot::Cancelled;
                }
                Err(e) => {
                    g.slots[index] = Slot::Failed(e.0);
                    g.error = Some(e.0);
                }
                Ok(_) => {
                    g.slots[index] = Slot::Failed(AgentErrorCode::InvalidOutput);
                    g.error = Some(AgentErrorCode::InvalidOutput);
                }
            }
            let mut snap = g.shared.lock().expect("snapshot poisoned");
            snap.text = String::new();
            snap.phase = Some(Phase::BetweenCandidates);
            snap.revision += 1;
        }
        notify();
    }
    {
        let mut g = generation.lock().expect("generation poisoned");
        g.finished_at = Some(clock());
        g.terminal = Some(if g.cancellation_requested() {
            JobState::Cancelled
        } else if g.counts().ready > 0 {
            JobState::Succeeded
        } else {
            JobState::Failed
        });
        let mut s = g.shared.lock().expect("snapshot poisoned");
        s.text = String::new();
        s.phase = Some(Phase::Finished);
        s.revision += 1;
    }
    notify();
}

#[cfg(test)]
mod tests;

//! Transport-independent, bounded transient session/job coordination.
//!
//! Callers supply monotonic elapsed milliseconds and opaque IDs from infrastructure.
//! No state in this module survives restart and no job implies a durable content operation;
//! durable operations live in [`operations`].

pub mod authentic;
pub mod diagnostics;
pub mod operations;
use std::collections::{BTreeMap, VecDeque};

pub const HEARTBEAT_INTERVAL_MS: u64 = 5_000;
pub const LEASE_DURATION_MS: u64 = 30_000;
pub const MAX_BUSINESS_JOBS: usize = 32;
pub const RESERVED_SAVE_SLOTS: usize = 8;
pub const MAX_TRANSIENT_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_REPLAY_EVENTS: usize = 2_048;
pub const MAX_REPLAY_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_EVENT_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    ClientBusy,
    SessionExpired,
    RequestMismatch,
    NotFound,
    Limit,
    InvalidTransition,
    InvalidInput,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobState {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

impl JobState {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Cancelled)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Job {
    pub id: String,
    pub sequence: u64,
    pub state: JobState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub instance_id: String,
    pub session_id: String,
    pub event_id: u64,
    pub job: Job,
}

/// Events already encoded by the outbound adapter are bounded by their wire size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Replay {
    Events(Vec<Event>),
    ResyncRequired,
}

#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub optional_jobs: usize,
    pub retained_bytes: usize,
    pub replay_events: usize,
    pub replay_bytes: usize,
    pub event_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            optional_jobs: MAX_BUSINESS_JOBS - RESERVED_SAVE_SLOTS,
            retained_bytes: MAX_TRANSIENT_BYTES,
            replay_events: MAX_REPLAY_EVENTS,
            replay_bytes: MAX_REPLAY_BYTES,
            event_bytes: MAX_EVENT_BYTES,
        }
    }
}

struct Request {
    digest: String,
    job_id: String,
}

struct Session {
    id: String,
    last_heartbeat: u64,
    jobs: BTreeMap<String, Job>,
    requests: BTreeMap<String, Request>,
    published: BTreeMap<String, Event>,
    retained_bytes: usize,
    events: VecDeque<(Event, usize)>,
    replay_bytes: usize,
    /// The last event no longer available, even if its gap notification was lost.
    replay_floor: u64,
}

pub struct Coordinator {
    instance_id: String,
    limits: Limits,
    session: Option<Session>,
    event_id: u64,
}

impl Coordinator {
    pub fn new(instance_id: String, limits: Limits) -> Self {
        Self {
            instance_id,
            limits,
            session: None,
            event_id: 0,
        }
    }

    /// A production clock adapter must provide monotonic time, including wake gaps.
    pub fn expire(&mut self, now: u64) -> bool {
        if self
            .session
            .as_ref()
            .is_some_and(|s| now.saturating_sub(s.last_heartbeat) >= LEASE_DURATION_MS)
        {
            // Dropping the session also erases all transient requests and jobs.
            self.session = None;
            true
        } else {
            false
        }
    }

    /// Only the bridge possessing the current ID may reconnect to that lease.
    pub fn open(&mut self, proposed_id: &str, now: u64) -> Result<(), Error> {
        validate_id(proposed_id)?;
        self.expire(now);
        match self.session.as_mut() {
            Some(session) if session.id == proposed_id => {
                session.last_heartbeat = now;
                Ok(())
            }
            Some(_) => Err(Error::ClientBusy),
            None => {
                self.session = Some(Session {
                    id: proposed_id.into(),
                    last_heartbeat: now,
                    jobs: BTreeMap::new(),
                    requests: BTreeMap::new(),
                    published: BTreeMap::new(),
                    retained_bytes: 0,
                    events: VecDeque::new(),
                    replay_bytes: 0,
                    replay_floor: self.event_id,
                });
                Ok(())
            }
        }
    }

    fn authorized(&mut self, id: &str, now: u64) -> Result<&mut Session, Error> {
        self.expire(now);
        self.session
            .as_mut()
            .filter(|s| s.id == id)
            .ok_or(Error::SessionExpired)
    }

    pub fn heartbeat(&mut self, id: &str, now: u64) -> Result<(), Error> {
        self.authorized(id, now)?.last_heartbeat = now;
        Ok(())
    }

    pub fn close(&mut self, id: &str, now: u64) -> Result<(), Error> {
        self.authorized(id, now)?;
        self.session = None;
        Ok(())
    }

    /// Application port for admission; it is deliberately not an HTTP endpoint.
    /// `reserved_bytes` accounts for the maximum result/metadata owned by the run.
    pub fn admit(
        &mut self,
        id: &str,
        now: u64,
        request_id: &str,
        digest: &str,
        job_id: &str,
        reserved_bytes: usize,
    ) -> Result<Job, Error> {
        validate_id(request_id)?;
        validate_id(job_id)?;
        if digest.len() != 64 || !digest.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(Error::InvalidInput);
        }
        let limits = self.limits;
        let session = self.authorized(id, now)?;
        // Lookup precedes busy/capacity checks, so a lost reply is recoverable.
        if let Some(request) = session.requests.get(request_id) {
            return if request.digest == digest {
                session
                    .jobs
                    .get(&request.job_id)
                    .cloned()
                    .ok_or(Error::NotFound)
            } else {
                Err(Error::RequestMismatch)
            };
        }
        if session.jobs.contains_key(job_id) {
            return Err(Error::InvalidInput);
        }
        let metadata_bytes = request_id.len() + digest.len() + job_id.len() + 2048;
        let bytes = reserved_bytes
            .checked_add(metadata_bytes)
            .ok_or(Error::Limit)?;
        let total = session
            .retained_bytes
            .checked_add(bytes)
            .ok_or(Error::Limit)?;
        // Terminal jobs are retained for retries; bytes bound their metadata too.
        if session
            .jobs
            .values()
            .filter(|j| !j.state.is_terminal())
            .count()
            >= limits.optional_jobs
            || total > limits.retained_bytes
        {
            return Err(Error::Limit);
        }
        let job = Job {
            id: job_id.into(),
            sequence: 1,
            state: JobState::Queued,
        };
        session.retained_bytes = total;
        session.requests.insert(
            request_id.into(),
            Request {
                digest: digest.into(),
                job_id: job_id.into(),
            },
        );
        session.jobs.insert(job_id.into(), job.clone());
        Ok(job)
    }

    pub fn get(&mut self, id: &str, now: u64, job_id: &str) -> Result<Job, Error> {
        self.authorized(id, now)?
            .jobs
            .get(job_id)
            .cloned()
            .ok_or(Error::NotFound)
    }

    /// Stable bounded pagination; adapters map `after` to a validated opaque cursor.
    pub fn event_cursor(&self) -> u64 {
        self.event_id
    }

    pub fn list(
        &mut self,
        id: &str,
        now: u64,
        after: Option<&str>,
        limit: usize,
    ) -> Result<Vec<Job>, Error> {
        if limit == 0 || limit > 200 {
            return Err(Error::Limit);
        }
        Ok(self
            .authorized(id, now)?
            .jobs
            .values()
            .filter(|job| after.is_none_or(|cursor| job.id.as_str() > cursor))
            .take(limit)
            .cloned()
            .collect())
    }

    pub fn transition(
        &mut self,
        id: &str,
        now: u64,
        job_id: &str,
        state: JobState,
    ) -> Result<Job, Error> {
        let job = self
            .authorized(id, now)?
            .jobs
            .get_mut(job_id)
            .ok_or(Error::NotFound)?;
        if job.state == state {
            return Ok(job.clone());
        }
        if job.state.is_terminal()
            || (job.state == JobState::Queued && state == JobState::Succeeded)
        {
            return Err(Error::InvalidTransition);
        }
        job.sequence = job.sequence.checked_add(1).ok_or(Error::Limit)?;
        job.state = state;
        Ok(job.clone())
    }

    pub fn cancel(&mut self, id: &str, now: u64, job_id: &str) -> Result<Job, Error> {
        let job = self.get(id, now, job_id)?;
        if job.state.is_terminal() {
            return Ok(job);
        }
        self.transition(id, now, job_id, JobState::Cancelled)
    }

    /// The API adapter supplies the actual encoded envelope size. State is already
    /// authoritative before publication. A dropped oversized event forces resync.
    pub fn publish(
        &mut self,
        id: &str,
        now: u64,
        job_id: &str,
        encoded_bytes: usize,
    ) -> Result<Event, Error> {
        let job = self.get(id, now, job_id)?;
        if let Some(event) = self.authorized(id, now)?.published.get(job_id)
            && event.job.sequence == job.sequence
        {
            return Ok(event.clone());
        }
        let next = self.event_id.checked_add(1).ok_or(Error::Limit)?;
        self.event_id = next;
        let event = Event {
            instance_id: self.instance_id.clone(),
            session_id: id.into(),
            event_id: next,
            job,
        };
        let limits = self.limits;
        let session = self.authorized(id, now)?;
        if encoded_bytes > limits.event_bytes
            || encoded_bytes > limits.replay_bytes
            || limits.replay_events == 0
        {
            session.events.clear();
            session.replay_bytes = 0;
            session.replay_floor = next;
            return Err(Error::Limit);
        }
        while session.events.len() >= limits.replay_events
            || session.replay_bytes.saturating_add(encoded_bytes) > limits.replay_bytes
        {
            if let Some((old, bytes)) = session.events.pop_front() {
                session.replay_bytes -= bytes;
                session.replay_floor = old.event_id;
            } else {
                break;
            }
        }
        session.replay_bytes += encoded_bytes;
        session.events.push_back((event.clone(), encoded_bytes));
        session.published.insert(job_id.into(), event.clone());
        Ok(event)
    }

    /// Read one notification without cloning the entire retained replay ring.
    pub fn next_event(
        &mut self,
        id: &str,
        now: u64,
        instance_id: &str,
        after: u64,
    ) -> Result<Replay, Error> {
        let same = instance_id == self.instance_id;
        let latest = self.event_id;
        let session = self.authorized(id, now)?;
        if !same || after < session.replay_floor || after > latest {
            return Ok(Replay::ResyncRequired);
        }
        Ok(Replay::Events(
            session
                .events
                .iter()
                .find(|(event, _)| event.event_id > after)
                .map(|(event, _)| vec![event.clone()])
                .unwrap_or_default(),
        ))
    }

    pub fn replay(
        &mut self,
        id: &str,
        now: u64,
        instance_id: &str,
        after: u64,
    ) -> Result<Replay, Error> {
        let same_instance = instance_id == self.instance_id;
        let latest = self.event_id;
        let session = self.authorized(id, now)?;
        if !same_instance || after < session.replay_floor || after > latest {
            return Ok(Replay::ResyncRequired);
        }
        Ok(Replay::Events(
            session
                .events
                .iter()
                .filter(|(e, _)| e.event_id > after)
                .map(|(e, _)| e.clone())
                .collect(),
        ))
    }
}

fn validate_id(value: &str) -> Result<(), Error> {
    if value.is_empty()
        || value.len() > 64
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        Err(Error::InvalidInput)
    } else {
        Ok(())
    }
}

/// A redacted reload problem: a coarse config area and a stable code, never a value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReloadIssue {
    pub path: &'static str,
    pub code: &'static str,
}
pub enum ReloadOutcome {
    /// The new snapshot is validated and adopted.
    Applied(StartupSummary),
    /// A change needs a restart; the last config stays in force.
    RestartRequired(Vec<ReloadIssue>),
    /// Profile or export settings changed while an operation may be using them.
    Busy,
    /// The new snapshot is invalid; the last config stays in force.
    Rejected(Vec<ReloadIssue>),
}
/// Re-reads and validates the private server config. `busy` is true while any job is not terminal.
pub trait ConfigReloader: Send + Sync {
    fn reload(&self, busy: bool) -> ReloadOutcome;
}

#[cfg(test)]
mod tests;

/// Content-free startup observations. A readable folder is not a verified adapter.
#[derive(Default, Clone)]
pub struct StartupSummary {
    pub sources: Vec<Availability>,
    pub profiles: Vec<Availability>,
}
#[derive(Clone)]
pub struct Availability {
    pub alias: String,
    pub reason: UnavailableReason,
}
#[derive(Clone, Copy)]
pub enum UnavailableReason {
    SourceRootMissing,
    SourceRootDenied,
    SourceAdapterUnverified,
    ProfileHomeMissing,
    ProfileHomeDenied,
    ExecutableMissing,
    AgentAdapterUnverified,
    NativeAdapterUnverified,
}

pub mod imports;
pub mod source;

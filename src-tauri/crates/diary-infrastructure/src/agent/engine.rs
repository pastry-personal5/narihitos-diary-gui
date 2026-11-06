//! The one lifecycle every CLI agent runs through: gate, lookup, assets, packet, executable and home
//! resolution, run directory, phases, preflight, supervised spawn, stop/escape mapping, output
//! validation, `Run` finish, active counter and run log. A `Protocol` supplies only what differs
//! (env, version/auth probes, argv, stream parsing); it never touches the supervisor, run directory
//! lifetime, counter or logger. Ordering here is the contract the adapter suites pin.
use super::{
    assets::{AssetError, AssetReason, CHAT, GENERATE, Prompt, PromptAssets},
    host::{Host, RunDir},
    output::{validate_generation, validate_reply},
    packet::encode_packet,
    process::{Exit, Outcome, Spec, Stop, Supervisor},
    profiles::{Entry, ProfileTable},
};
use diary_application::{
    agent::*,
    diagnostics::{
        ActionTag, AgentDetail, EventCode, ExitTag, Level, PhaseTag, Record, StatusTag, StopTag,
    },
};
use std::{
    collections::HashMap,
    ffi::OsString,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Instant,
};

/// Everything a protocol may use while probing: resolved paths and the child environment.
pub struct Ctx<'a> {
    pub host: &'a Host,
    pub exe: &'a Path,
    pub home: &'a Path,
    pub dir: &'a RunDir,
    pub env: Vec<(OsString, OsString)>,
}
impl Ctx<'_> {
    /// Status probe (never inference): outcome plus stdout and stderr-tail text.
    pub fn probe(&self, args: &[&str], cancel: &CancelToken) -> Option<(Outcome, String)> {
        self.host
            .probe(self.exe, self.env.clone(), self.dir, args, cancel)
    }
}

/// What the protocol decided to launch with.
pub struct Invocation {
    pub argv: Vec<OsString>,
    /// A file the child writes its final message to, when the protocol reads results from one.
    pub result_file: Option<PathBuf>,
}

/// Final bytes plus, for protocols that report it, whether the output was structured.
pub struct Collected {
    pub bytes: Vec<u8>,
    pub structured: Option<bool>,
}

pub trait Stream: Send {
    fn record(&mut self, rec: &[u8], chat: bool, run: &Run, sink: &mut dyn EventSink);
    /// Checks after stop and escape mapping and before the `ValidatingOutput` phase.
    fn conclude(
        &mut self,
        outcome: &Outcome,
        inv: &Invocation,
        prompt: &Prompt,
    ) -> Result<(), AgentError>;
    /// Reads the final output after the `ValidatingOutput` phase.
    fn collect(&mut self, inv: &Invocation) -> Result<Collected, AgentError>;
    /// A more specific log reason for the last `conclude` failure, when the protocol knows one.
    fn hint(&self) -> Option<Reason> {
        None
    }
}

pub trait Protocol: Send + Sync + 'static {
    fn kind(&self) -> AgentKind;
    fn env(&self, host: &Host, exe: &Path, home: &Path, tmp: &Path) -> Vec<(OsString, OsString)>;
    /// `Ok` only for a supported version; runs before every run and every check.
    fn version(&self, ctx: &Ctx, cancel: &CancelToken) -> Result<(), Reason>;
    /// Check-only login probe, after `version`. Default: nothing to probe.
    fn auth(&self, _ctx: &Ctx) -> ProfileCheck {
        ProfileStatus::Ready.into()
    }
    /// Runs after the asset lookup and before the packet is encoded.
    fn check_prompt(&self, _prompt: &Prompt) -> Result<(), Reason> {
        Ok(())
    }
    /// Builds argv (and any private files in the run directory) after preflight.
    fn invocation(
        &self,
        dir: &RunDir,
        prompt: &Prompt,
        chat: bool,
    ) -> Result<Invocation, AgentError>;
    fn stream(&self) -> Box<dyn Stream>;
}

/// The status a reason implies; the one table every adapter shares.
pub fn status_of(r: Reason) -> ProfileStatus {
    match r {
        Reason::ExecutableMissing => ProfileStatus::Missing,
        Reason::VersionUnsupported => ProfileStatus::UnsupportedVersion,
        Reason::HostBusy => ProfileStatus::Busy,
        Reason::ProbeFailed
        | Reason::ProbeTimedOut
        | Reason::SpawnFailed
        | Reason::VersionUnreadable
        | Reason::RunDirUnavailable
        | Reason::HelperFailed => ProfileStatus::Failed,
        _ => ProfileStatus::Unavailable,
    }
}
fn check_of(r: Reason) -> ProfileCheck {
    ProfileCheck {
        status: status_of(r),
        reason: Some(r),
    }
}
/// Why a status probe that did not finish cleanly failed.
pub fn probe_fault(o: &Outcome) -> Reason {
    match o.stop {
        Some(Stop::Inactivity | Stop::Wall) => Reason::ProbeTimedOut,
        _ => Reason::ProbeFailed,
    }
}

/// Why a status probe that did not finish cleanly failed, or that it never started.
pub fn probe_failure(o: Option<&Outcome>) -> Reason {
    o.map_or(Reason::SpawnFailed, probe_fault)
}
/// Failed login probe as a check result.
pub fn auth_failed(o: Option<&Outcome>) -> ProfileCheck {
    check_of(probe_failure(o))
}

/// Counts events and remembers the last phase for the run record.
struct Watch<'a> {
    inner: &'a mut dyn EventSink,
    events: u32,
    phase: Option<PhaseTag>,
}
impl EventSink for Watch<'_> {
    fn event(&mut self, e: AgentEvent) {
        self.events = self.events.saturating_add(1);
        if let AgentEvent::Phase(p) = &e {
            self.phase = Some((*p).into());
        }
        self.inner.event(e);
    }
}

/// What the run record needs besides the sink's counts.
#[derive(Default)]
struct Diag {
    reason: Option<Reason>,
    stop: Option<StopTag>,
    exit: Option<(ExitTag, Option<i32>)>,
    escaped: u32,
    input: u32,
    output: u32,
    spawned: bool,
}
impl Diag {
    fn fail(&mut self, reason: Reason, code: AgentErrorCode) -> AgentError {
        self.reason = Some(reason);
        code.into()
    }
}

pub struct Adapter<P: Protocol> {
    table: Arc<ProfileTable>,
    assets: PromptAssets,
    host: Arc<Host>,
    proto: P,
    /// Last `(status, reason)` per alias, so a check record can say whether the reason changed.
    last: Mutex<HashMap<String, (ProfileStatus, Option<Reason>)>>,
}

impl<P: Protocol> Adapter<P> {
    pub fn with_protocol(table: Arc<ProfileTable>, host: Arc<Host>, proto: P) -> Self {
        Self {
            table,
            assets: PromptAssets::embedded(),
            host,
            proto,
            last: Mutex::new(HashMap::new()),
        }
    }
    pub fn assets(&self) -> &PromptAssets {
        &self.assets
    }

    fn binary(&self) -> &'static str {
        self.proto
            .kind()
            .spec()
            .default_binary
            .expect("CLI kinds have a default binary")
    }

    fn ctx<'a>(&'a self, exe: &'a Path, home: &'a Path, dir: &'a RunDir) -> Ctx<'a> {
        Ctx {
            host: &self.host,
            exe,
            home,
            dir,
            env: self.proto.env(&self.host, exe, home, dir.path()),
        }
    }

    fn check_entry(&self, e: &Entry) -> ProfileCheck {
        if self.host.is_busy() {
            return check_of(Reason::HostBusy);
        }
        let Some(exe) = self.host.resolve(e, self.binary()) else {
            return check_of(Reason::ExecutableMissing);
        };
        let home = e.home.as_ref().map(|h| self.host.launch_cwd.join(h));
        let Some(home) = home.as_deref().filter(|h| h.is_dir()) else {
            return check_of(home_reason(home.as_deref()));
        };
        let Ok(dir) = RunDir::create(&self.host.runtime_root) else {
            return check_of(Reason::RunDirUnavailable);
        };
        let ctx = self.ctx(&exe, home, &dir);
        match self.proto.version(&ctx, &CancelToken::default()) {
            Ok(()) => self.proto.auth(&ctx),
            Err(r) => check_of(r),
        }
    }

    fn log_check(&self, profile: &ProfileHandle, c: ProfileCheck, started: Instant) {
        let changed = {
            let mut last = self.last.lock().expect("check log poisoned");
            let now = (c.status, c.reason);
            last.insert(profile.alias.clone(), now) != Some(now)
        };
        let mut d = AgentDetail::new(self.proto.kind());
        d.action = Some(ActionTag::Check);
        d.status = Some(StatusTag::from(c.status));
        d.reason = c.reason;
        d.changed = Some(changed);
        let level = match c.status {
            ProfileStatus::Ready => Level::Info,
            ProfileStatus::Busy | ProfileStatus::Unauthenticated => Level::Info,
            _ => Level::Warn,
        };
        self.host.emit(
            Record::new(EventCode::AgentCheck, level)
                .count(elapsed_ms(started))
                .agent(d),
        );
    }

    fn execute(
        &self,
        request: &AgentRequest,
        events: &mut dyn EventSink,
        run: &Run,
        diag: &mut Diag,
    ) -> Result<AgentOutput, AgentError> {
        use AgentErrorCode as C;
        let phase = |events: &mut dyn EventSink, p: Phase| {
            if let Some(e) = run.accept(AgentEvent::Phase(p)) {
                events.event(e);
            }
        };
        if run.token().is_cancelled() || self.host.is_shutdown() {
            return Err(diag.fail(
                if self.host.is_shutdown() {
                    Reason::HostShuttingDown
                } else {
                    Reason::CancelledBeforeStart
                },
                C::Cancelled,
            ));
        }
        let stop = self.host.stop_flag();
        let handle = request.profile();
        if handle.kind != self.proto.kind() {
            return Err(diag.fail(Reason::KindMismatch, C::Unavailable));
        }
        // An entry configured for another kind is also refused: `check` already does, and a stale or
        // forged handle must not run one agent's profile with another agent's argv.
        let entry = match self.table.get(&handle.alias) {
            None => return Err(diag.fail(Reason::ProfileUnknown, C::Unavailable)),
            Some(e) if e.kind != self.proto.kind() => {
                return Err(diag.fail(Reason::KindMismatch, C::Unavailable));
            }
            Some(e) => e,
        };
        let (chat, name) = match request.mode() {
            Mode::Chat(_) => (true, CHAT),
            Mode::Candidate { .. } => (false, GENERATE),
        };
        let prompt = self
            .assets
            .get(self.proto.kind(), name)
            .map_err(|e: AssetError| {
                diag.fail(
                    match e.reason {
                        AssetReason::Missing => Reason::AssetsMissing,
                        AssetReason::Malformed(_) => Reason::AssetsInvalid,
                    },
                    C::AssetsUnavailable,
                )
            })?;
        self.proto
            .check_prompt(prompt)
            .map_err(|r| diag.fail(r, C::AssetsUnavailable))?;
        let data =
            encode_packet(request.mode()).map_err(|e| diag.fail(Reason::PacketTooLarge, e.0))?;
        let stdin = prompt.render(request.locale(), Some(&data)).into_bytes();
        diag.input = u32::try_from(stdin.len()).unwrap_or(u32::MAX);
        let exe = self
            .host
            .resolve(&entry, self.binary())
            .ok_or_else(|| diag.fail(Reason::ExecutableMissing, C::Unavailable))?;
        // The child runs in its own cwd, so a relative `profile_home` must be made absolute here.
        let home = entry.home.as_deref().map(|h| self.host.launch_cwd.join(h));
        let Some(home) = home.filter(|h| h.is_dir()) else {
            let missing = entry.home.as_deref().map(|h| self.host.launch_cwd.join(h));
            return Err(diag.fail(home_reason(missing.as_deref()), C::Unavailable));
        };
        let dir = RunDir::create(&self.host.runtime_root)
            .map_err(|_| diag.fail(Reason::RunDirUnavailable, C::Failed))?;
        phase(events, Phase::StartingAgent);
        let ctx = self.ctx(&exe, &home, &dir);
        self.proto.version(&ctx, &run.token()).map_err(|r| {
            if run.token().is_cancelled() {
                diag.fail(Reason::CancelledBeforeStart, C::Cancelled)
            } else {
                diag.fail(r, C::Unavailable)
            }
        })?;
        let inv = self
            .proto
            .invocation(&dir, prompt, chat)
            .map_err(|e| diag.fail(Reason::RunDirUnavailable, e.0))?;
        let spec = Spec {
            argv: inv.argv.clone(),
            env: ctx.env.clone(),
            executable: exe.clone(),
            stdin,
            cwd: dir.path().to_path_buf(),
            limits: self.host.limits,
            shutdown: Some(stop.1.clone()),
        };
        let mut stream = self.proto.stream();
        let outcome = Supervisor::run(spec, &run.token(), &mut |rec| {
            stream.record(rec, chat, run, events)
        })
        .map_err(|_| diag.fail(Reason::SpawnFailed, C::Unavailable))?;
        diag.spawned = true;
        diag.escaped = u32::try_from(outcome.escaped).unwrap_or(u32::MAX);
        diag.output = u32::try_from(outcome.output_bytes).unwrap_or(u32::MAX);
        diag.exit = Some(match outcome.exit {
            Exit::Success => (ExitTag::Success, None),
            Exit::Status(c) => (ExitTag::Code, Some(c)),
            Exit::Signal(s) => (ExitTag::Signal, Some(s)),
        });

        if let Some(stop) = outcome.stop {
            diag.stop = Some(match stop {
                Stop::Cancelled => StopTag::Cancelled,
                Stop::Inactivity => StopTag::Inactivity,
                Stop::Wall => StopTag::Wall,
                Stop::RecordTooLarge => StopTag::RecordTooLarge,
                Stop::OutputTooLarge => StopTag::OutputTooLarge,
            });
            diag.reason = match stop {
                Stop::Cancelled if self.host.is_shutdown() => Some(Reason::HostShuttingDown),
                // Stopped with neither the job nor the server shut down: the lease ended.
                Stop::Cancelled if !run.token().is_cancelled() => Some(Reason::LeaseEnded),
                Stop::RecordTooLarge => Some(Reason::RecordTooLarge),
                Stop::OutputTooLarge => Some(Reason::OutputTooLarge),
                _ => None,
            };
            return Err(match stop {
                Stop::Cancelled => C::Cancelled,
                Stop::Inactivity | Stop::Wall => C::Timeout,
                Stop::RecordTooLarge | Stop::OutputTooLarge => C::InvalidOutput,
            }
            .into());
        }
        if outcome.escaped > 0 {
            return Err(diag.fail(Reason::EscapedDescendants, C::Failed));
        }
        stream.conclude(&outcome, &inv, prompt).map_err(|e| {
            let r = stream.hint().unwrap_or(if outcome.exit == Exit::Success {
                Reason::ResultMissing
            } else {
                Reason::ExitNonzero
            });
            diag.fail(r, e.0)
        })?;
        phase(events, Phase::ValidatingOutput);
        let out = stream
            .collect(&inv)
            .map_err(|e| diag.fail(Reason::ResultMissing, e.0))?;
        match request.mode() {
            Mode::Candidate { packet, .. } => {
                if out.structured == Some(false) {
                    return Err(C::InvalidOutput.into());
                }
                validate_generation(&out.bytes, packet.snippets().len()).map(AgentOutput::Candidate)
            }
            Mode::Chat(_) => {
                let text =
                    String::from_utf8(out.bytes).map_err(|_| AgentError::from(C::InvalidOutput))?;
                if text.trim().is_empty() {
                    return Err(C::InvalidOutput.into());
                }
                validate_reply(text)
            }
        }
    }
}

fn elapsed_ms(started: Instant) -> u32 {
    u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX)
}
/// A configured `profile_home` that is not a usable directory: absent or not a directory.
fn home_reason(home: Option<&Path>) -> Reason {
    match home {
        Some(h) if h.exists() => Reason::ProfileHomeNotDirectory,
        _ => Reason::ProfileHomeMissing,
    }
}

impl<P: Protocol> AgentRunner for Adapter<P> {
    fn check(&self, profile: &ProfileHandle) -> ProfileCheck {
        let started = Instant::now();
        let c = match self.table.get(&profile.alias) {
            Some(e) if e.kind == self.proto.kind() => self.check_entry(&e),
            Some(_) => check_of(Reason::KindMismatch),
            None => check_of(Reason::ProfileUnknown),
        };
        self.log_check(profile, c, started);
        c
    }
    fn run(
        &self,
        request: AgentRequest,
        events: &mut dyn EventSink,
        cancel: &CancelToken,
    ) -> Result<AgentOutput, AgentError> {
        let _active = self.host.enter();
        let started = Instant::now();
        let run = Run::from(cancel.clone());
        let mut diag = Diag::default();
        let mut watch = Watch {
            inner: events,
            events: 0,
            phase: None,
        };
        let result = run.finish(self.execute(&request, &mut watch, &run, &mut diag));
        let last_phase = watch.phase;
        if let Some(e) = run.accept(AgentEvent::Phase(Phase::Finished)) {
            watch.event(e);
        }
        let mut d = AgentDetail::new(self.proto.kind());
        d.action = Some(match request.mode() {
            Mode::Chat(_) => ActionTag::Chat,
            Mode::Candidate { .. } => ActionTag::Generate,
        });
        // The last phase before `Finished`, which every run reaches.
        d.phase = last_phase;
        d.code = result.as_ref().err().map(|e| e.0);
        d.stop = diag.stop;
        d.exit = diag.exit.map(|(t, _)| t);
        d.exit_number = diag.exit.and_then(|(_, n)| n);
        d.reason = diag.reason;
        d.escaped = Some(diag.escaped);
        d.events = Some(watch.events);
        d.input_bytes = Some(diag.input);
        d.output_bytes = Some(diag.output);
        let (code, level) = match &result {
            Ok(_) => (EventCode::AgentRun, Level::Info),
            Err(e) if e.0 == AgentErrorCode::Cancelled => (
                if diag.spawned {
                    EventCode::AgentRun
                } else {
                    EventCode::AgentBlocked
                },
                Level::Info,
            ),
            Err(_) if diag.reason == Some(Reason::EscapedDescendants) => {
                (EventCode::AgentRun, Level::Error)
            }
            Err(_) => (
                if diag.spawned {
                    EventCode::AgentRun
                } else {
                    EventCode::AgentBlocked
                },
                Level::Warn,
            ),
        };
        let mut rec = Record::new(code, level).count(elapsed_ms(started)).agent(d);
        if let Err(e) = &result {
            rec = rec.error(super::host::code_tag(e.0));
        }
        self.host.emit(rec);
        result
    }
}

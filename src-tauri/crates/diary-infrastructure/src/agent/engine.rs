//! The one lifecycle every CLI agent runs through: gate, lookup, assets, packet, executable and home
//! resolution, run directory, phases, preflight, supervised spawn, stop/escape mapping, output
//! validation, `Run` finish, active counter and run log. A `Protocol` supplies only what differs
//! (env, version/auth probes, argv, stream parsing); it never touches the supervisor, run directory
//! lifetime, counter or logger. Ordering here is the contract the adapter suites pin.
use super::{
    assets::{AssetError, CHAT, GENERATE, Prompt, PromptAssets},
    host::{Host, RunDir},
    output::{validate_generation, validate_reply},
    packet::encode_packet,
    process::{Exit, Outcome, Spec, Stop, Supervisor},
    profiles::{Entry, ProfileTable},
};
use diary_application::agent::*;
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    sync::Arc,
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
}

pub trait Protocol: Send + Sync + 'static {
    fn kind(&self) -> AgentKind;
    fn env(&self, host: &Host, exe: &Path, home: &Path, tmp: &Path) -> Vec<(OsString, OsString)>;
    /// `Ok` only for a supported version; runs before every run and every check.
    fn version(&self, ctx: &Ctx, cancel: &CancelToken) -> Result<(), ProfileStatus>;
    /// Check-only login probe, after `version`. Default: nothing to probe.
    fn auth(&self, _ctx: &Ctx) -> ProfileStatus {
        ProfileStatus::Ready
    }
    /// Runs after the asset lookup and before the packet is encoded.
    fn check_prompt(&self, _prompt: &Prompt) -> Result<(), AgentError> {
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

pub struct Adapter<P: Protocol> {
    table: Arc<ProfileTable>,
    assets: PromptAssets,
    host: Arc<Host>,
    proto: P,
}

impl<P: Protocol> Adapter<P> {
    pub fn with_protocol(table: Arc<ProfileTable>, host: Arc<Host>, proto: P) -> Self {
        Self {
            table,
            assets: PromptAssets::embedded(),
            host,
            proto,
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

    fn check_entry(&self, e: &Entry) -> ProfileStatus {
        if self.host.is_busy() {
            return ProfileStatus::Busy;
        }
        let Some(exe) = self.host.resolve(e, self.binary()) else {
            return ProfileStatus::Missing;
        };
        let home = e.home.as_ref().map(|h| self.host.launch_cwd.join(h));
        let Some(home) = home.as_deref().filter(|h| h.is_dir()) else {
            return ProfileStatus::Unavailable;
        };
        let Ok(dir) = RunDir::create(&self.host.runtime_root) else {
            return ProfileStatus::Failed;
        };
        let ctx = self.ctx(&exe, home, &dir);
        match self.proto.version(&ctx, &CancelToken::default()) {
            Ok(()) => self.proto.auth(&ctx),
            Err(s) => s,
        }
    }

    fn execute(
        &self,
        request: &AgentRequest,
        events: &mut dyn EventSink,
        run: &Run,
    ) -> Result<AgentOutput, AgentError> {
        let unavailable = || AgentError::from(AgentErrorCode::Unavailable);
        let phase = |events: &mut dyn EventSink, p: Phase| {
            if let Some(e) = run.accept(AgentEvent::Phase(p)) {
                events.event(e);
            }
        };
        if run.token().is_cancelled() || self.host.is_shutdown() {
            return Err(AgentErrorCode::Cancelled.into());
        }
        let stop = self.host.stop_flag();
        let handle = request.profile();
        if handle.kind != self.proto.kind() {
            return Err(unavailable());
        }
        // Also reject an entry configured for another kind: `check` already does, and a stale or
        // forged handle must not run one agent's profile with another agent's argv.
        let entry = self
            .table
            .get(&handle.alias)
            .filter(|e| e.kind == self.proto.kind())
            .ok_or_else(unavailable)?;
        let (chat, name) = match request.mode() {
            Mode::Chat(_) => (true, CHAT),
            Mode::Candidate { .. } => (false, GENERATE),
        };
        let prompt = self
            .assets
            .get(self.proto.kind(), name)
            .map_err(|_: AssetError| AgentError::from(AgentErrorCode::AssetsUnavailable))?;
        self.proto.check_prompt(prompt)?;
        let data = encode_packet(request.mode())?;
        let stdin = prompt.render(request.locale(), Some(&data)).into_bytes();
        let exe = self
            .host
            .resolve(&entry, self.binary())
            .ok_or_else(unavailable)?;
        // The child runs in its own cwd, so a relative `profile_home` must be made absolute here.
        let home = self
            .host
            .launch_cwd
            .join(entry.home.as_deref().ok_or_else(unavailable)?);
        if !home.is_dir() {
            return Err(unavailable());
        }
        let dir = RunDir::create(&self.host.runtime_root)
            .map_err(|_| AgentError::from(AgentErrorCode::Failed))?;
        phase(events, Phase::StartingAgent);
        let ctx = self.ctx(&exe, &home, &dir);
        self.proto.version(&ctx, &run.token()).map_err(|_| {
            if run.token().is_cancelled() {
                AgentErrorCode::Cancelled.into()
            } else {
                unavailable()
            }
        })?;
        let inv = self.proto.invocation(&dir, prompt, chat)?;
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
        .map_err(|_| unavailable())?;

        if let Some(stop) = outcome.stop {
            return Err(match stop {
                Stop::Cancelled => AgentErrorCode::Cancelled,
                Stop::Inactivity | Stop::Wall => AgentErrorCode::Timeout,
                Stop::RecordTooLarge | Stop::OutputTooLarge => AgentErrorCode::InvalidOutput,
            }
            .into());
        }
        if outcome.escaped > 0 {
            return Err(AgentErrorCode::Failed.into());
        }
        stream.conclude(&outcome, &inv, prompt)?;
        phase(events, Phase::ValidatingOutput);
        let out = stream.collect(&inv)?;
        match request.mode() {
            Mode::Candidate { packet, .. } => {
                if out.structured == Some(false) {
                    return Err(AgentErrorCode::InvalidOutput.into());
                }
                validate_generation(&out.bytes, packet.snippets().len()).map(AgentOutput::Candidate)
            }
            Mode::Chat(_) => {
                let text = String::from_utf8(out.bytes)
                    .map_err(|_| AgentError::from(AgentErrorCode::InvalidOutput))?;
                if text.trim().is_empty() {
                    return Err(AgentErrorCode::InvalidOutput.into());
                }
                validate_reply(text)
            }
        }
    }
}

/// `Exit::Success` with no stop and no escaped descendants: the shape every version probe wants.
pub fn clean(o: &Outcome) -> bool {
    o.exit == Exit::Success && o.stop.is_none() && o.escaped == 0
}

impl<P: Protocol> AgentRunner for Adapter<P> {
    fn check(&self, profile: &ProfileHandle) -> ProfileCheck {
        match self.table.get(&profile.alias) {
            Some(e) if e.kind == self.proto.kind() => self.check_entry(&e).into(),
            _ => ProfileStatus::Unavailable.into(),
        }
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
        let result = run.finish(self.execute(&request, events, &run));
        if let Some(e) = run.accept(AgentEvent::Phase(Phase::Finished)) {
            events.event(e);
        }
        self.host.log_run(&result, started);
        result
    }
}

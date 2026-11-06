//! Codex adapter for `codex-cli 0.167.0` only: status check, fixed-argv one-shot run, NDJSON event
//! parsing and error mapping. Every flag lives in `argv`; no diary or chat text ever reaches argv,
//! logs or `AgentError`. Output is accepted only through the M16 validators and `Run`.
use super::{
    assets::{AssetError, CHAT, GENERATE, PromptAssets},
    output::{validate_generation, validate_reply},
    packet::encode_packet,
    process::{Exit, Limits, Outcome, Spec, Stop, Supervisor},
    profiles::{Entry, ProfileTable},
};
use crate::config::select_executable;
use diary_application::{
    agent::*,
    diagnostics::{Diagnostics, ErrorTag, EventCode, Level, Record},
};
use serde_json::Value;
use std::{
    ffi::OsString,
    io::Read,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Instant,
};

pub const SUPPORTED_VERSION: &str = "codex-cli 0.167.0";

/// Strict structured-output schema for schema-1 generation: closed objects, every property required.
pub const GENERATION_SCHEMA: &str = r#"{"type":"object","additionalProperties":false,"required":["draft_markdown","claim_support","warnings"],"properties":{"draft_markdown":{"type":"string"},"claim_support":{"type":"array","items":{"type":"object","additionalProperties":false,"required":["claim","snippet_ids"],"properties":{"claim":{"type":"string"},"snippet_ids":{"type":"array","items":{"type":"string"}}}}},"warnings":{"type":"array","items":{"type":"string"}}}}"#;

/// Fresh owner-private directory under `<runtime_root>/agent-runs`, removed on drop.
struct RunDir(PathBuf);
impl RunDir {
    fn create(runtime_root: &Path) -> std::io::Result<Self> {
        let parent = runtime_root.join("agent-runs");
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&parent)?;
        let name = crate::lifecycle::random_uuid()
            .map_err(|_| std::io::Error::other("entropy unavailable"))?;
        let dir = parent.join(name);
        std::fs::DirBuilder::new().mode(0o700).create(&dir)?;
        Ok(Self(dir.canonicalize()?))
    }
    fn path(&self) -> &Path {
        &self.0
    }
}
impl Drop for RunDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
/// Startup cleanup: nothing is running yet, so every leftover run directory is stale.
pub fn sweep_stale_runs(runtime_root: &Path) {
    let _ = std::fs::remove_dir_all(runtime_root.join("agent-runs"));
}

pub struct CodexAdapter {
    table: Arc<ProfileTable>,
    assets: PromptAssets,
    runtime_root: PathBuf,
    launch_path: OsString,
    launch_cwd: PathBuf,
    home: Option<OsString>,
    lang: Option<OsString>,
    limits: Limits,
    probe: Limits,
    diagnostics: Arc<dyn Diagnostics>,
    active: AtomicUsize,
    shutdown: Arc<AtomicBool>,
    /// One stop flag per in-flight probe or run, so a lease end can stop them without poisoning later ones.
    flags: Mutex<Vec<Arc<AtomicBool>>>,
}
struct StopFlag<'a>(&'a CodexAdapter, Arc<AtomicBool>);
impl Drop for StopFlag<'_> {
    fn drop(&mut self) {
        self.0
            .flags
            .lock()
            .expect("flags poisoned")
            .retain(|f| !Arc::ptr_eq(f, &self.1));
    }
}
struct Active<'a>(&'a AtomicUsize);
impl Drop for Active<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

fn code_tag(c: AgentErrorCode) -> ErrorTag {
    match c {
        AgentErrorCode::Busy => ErrorTag::Busy,
        AgentErrorCode::Unavailable | AgentErrorCode::AssetsUnavailable => ErrorTag::Unavailable,
        AgentErrorCode::RateLimited | AgentErrorCode::Context | AgentErrorCode::Timeout => {
            ErrorTag::Limit
        }
        AgentErrorCode::InvalidOutput | AgentErrorCode::UnsupportedLanguage => ErrorTag::Invalid,
        _ => ErrorTag::Internal,
    }
}

/// Fixed class patterns recorded with the fixtures. The first group is observed in 0.167.0 captures;
/// the rate-limit and context patterns are synthetic (taken from the binary's strings), not captured.
pub fn classify(message: &str) -> AgentErrorCode {
    let m = message.to_ascii_lowercase();
    let has = |needles: &[&str]| needles.iter().any(|n| m.contains(n));
    // Status codes match whole tokens only, so "req_14290" is not a 429.
    let code = |c: &str| {
        m.split(|ch: char| !ch.is_ascii_alphanumeric())
            .any(|t| t == c)
    };
    if has(&["usage limit", "rate limit", "too many requests"]) || code("429") {
        AgentErrorCode::RateLimited
    } else if has(&["context window", "context length", "maximum context"]) {
        AgentErrorCode::Context
    } else if code("401") || has(&["unauthorized", "not logged in", "log in", "sign in"]) {
        AgentErrorCode::Unavailable
    } else {
        AgentErrorCode::Failed
    }
}

#[derive(Default)]
struct Stream {
    turn_completed: bool,
    failed: Option<String>,
    last_error: Option<String>,
    bad: bool,
    unknown: u32,
}
impl Stream {
    fn record(&mut self, rec: &[u8], chat: bool, run: &Run, sink: &mut dyn EventSink) {
        if rec.iter().all(u8::is_ascii_whitespace) {
            return;
        }
        let Ok(v) = serde_json::from_slice::<Value>(rec) else {
            self.bad = true;
            return;
        };
        let mut emit = |e: AgentEvent| {
            if let Some(e) = run.accept(e) {
                sink.event(e);
            }
        };
        match v["type"].as_str() {
            Some("thread.started") => {}
            Some("turn.started") => emit(AgentEvent::Phase(Phase::WaitingForOutput)),
            Some("item.completed") => match v["item"]["type"].as_str() {
                Some("agent_message") => {
                    emit(AgentEvent::Phase(Phase::ReceivingOutput));
                    if chat && let Some(t) = v["item"]["text"].as_str() {
                        emit(AgentEvent::Delta(t.to_owned()));
                    }
                }
                Some("error") => {} // non-fatal notices (model metadata, transport fallback)
                _ => self.unknown += 1,
            },
            Some("turn.completed") => self.turn_completed = true,
            Some("turn.failed") => {
                self.failed = Some(
                    v["error"]["message"]
                        .as_str()
                        .unwrap_or_default()
                        .to_owned(),
                );
            }
            Some("error") => {
                self.last_error = v["message"].as_str().map(str::to_owned);
            }
            _ => self.unknown += 1,
        }
    }
}

/// What a recorded stream says, for fixture replay: the same parser the adapter uses.
#[derive(Debug, PartialEq, Eq)]
pub struct Summary {
    pub turn_completed: bool,
    /// Classified `turn.failed`; `None` when the turn did not fail.
    pub failure: Option<AgentErrorCode>,
    pub malformed: bool,
    pub unknown_events: u32,
}
pub fn summarize(ndjson: &[u8]) -> Summary {
    let run = Run::new();
    struct Null;
    impl EventSink for Null {
        fn event(&mut self, _: AgentEvent) {}
    }
    let mut stream = Stream::default();
    for rec in ndjson.split(|&b| b == b'\n') {
        stream.record(rec, true, &run, &mut Null);
    }
    Summary {
        turn_completed: stream.turn_completed,
        failure: stream.failed.as_deref().map(classify),
        malformed: stream.bad,
        unknown_events: stream.unknown,
    }
}

fn read_bounded(path: &Path) -> Result<Vec<u8>, AgentError> {
    let f = std::fs::File::open(path).map_err(|_| AgentError::from(AgentErrorCode::Failed))?;
    let mut buf = Vec::new();
    f.take(MAX_RESULT_BYTES as u64 + 1)
        .read_to_end(&mut buf)
        .map_err(|_| AgentError::from(AgentErrorCode::Failed))?;
    if buf.len() > MAX_RESULT_BYTES {
        return Err(AgentErrorCode::InvalidOutput.into());
    }
    Ok(buf)
}

impl CodexAdapter {
    pub fn new(
        table: Arc<ProfileTable>,
        runtime_root: PathBuf,
        launch_path: OsString,
        launch_cwd: PathBuf,
        diagnostics: Arc<dyn Diagnostics>,
    ) -> Self {
        Self::with_limits(
            table,
            runtime_root,
            launch_path,
            launch_cwd,
            diagnostics,
            Limits::production(),
            Limits::probe(),
        )
    }
    pub fn with_limits(
        table: Arc<ProfileTable>,
        runtime_root: PathBuf,
        launch_path: OsString,
        launch_cwd: PathBuf,
        diagnostics: Arc<dyn Diagnostics>,
        limits: Limits,
        probe: Limits,
    ) -> Self {
        Self {
            table,
            assets: PromptAssets::embedded(),
            runtime_root,
            launch_path,
            launch_cwd,
            home: std::env::var_os("HOME"),
            lang: std::env::var_os("LANG"),
            limits,
            probe,
            diagnostics,
            active: AtomicUsize::new(0),
            shutdown: Arc::new(AtomicBool::new(false)),
            flags: Mutex::new(Vec::new()),
        }
    }
    /// Server shutdown: in-flight runs and probes stop like a cancel (TERM, grace, KILL, reap) and
    /// new ones are refused. Lease expiry and close reach a run through its job's `Run` cancel.
    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
        self.cancel_all();
    }
    /// The client lease ended (expiry or close): stop everything in flight; later runs are unaffected.
    pub fn cancel_all(&self) {
        for f in self.flags.lock().expect("flags poisoned").iter() {
            f.store(true, Ordering::SeqCst);
        }
    }
    fn stop_flag(&self) -> StopFlag<'_> {
        let f = Arc::new(AtomicBool::new(self.shutdown.load(Ordering::SeqCst)));
        self.flags.lock().expect("flags poisoned").push(f.clone());
        StopFlag(self, f)
    }
    pub fn assets(&self) -> &PromptAssets {
        &self.assets
    }

    fn env(&self, exe: &Path, codex_home: &Path, tmp: &Path) -> Vec<(OsString, OsString)> {
        // The launch PATH (absolute entries) lets a symlinked shim such as `#!/usr/bin/env node` find
        // its interpreter; `exe` is already canonical, so its own directory alone is not enough.
        let mut dirs: Vec<PathBuf> = exe.parent().map(Path::to_path_buf).into_iter().collect();
        dirs.extend(std::env::split_paths(&self.launch_path).filter(|d| d.is_absolute()));
        dirs.extend(["/usr/bin", "/bin", "/usr/sbin", "/sbin"].map(PathBuf::from));
        let path = std::env::join_paths(dirs).unwrap_or_default();
        let mut env = vec![
            ("PATH".into(), path),
            ("TMPDIR".into(), tmp.as_os_str().to_owned()),
            ("CODEX_HOME".into(), codex_home.as_os_str().to_owned()),
        ];
        env.extend(self.home.clone().map(|h| ("HOME".into(), h)));
        env.extend(self.lang.clone().map(|l| ("LANG".into(), l)));
        env
    }

    fn resolve(&self, e: &Entry) -> Option<PathBuf> {
        select_executable(
            e.executable.as_deref(),
            "codex",
            &self.launch_path,
            &self.launch_cwd,
        )
    }

    /// Runs a status probe and returns its stdout text plus stderr tail for matching.
    fn probe(
        &self,
        exe: &Path,
        home: &Path,
        dir: &RunDir,
        args: &[&str],
        cancel: &CancelToken,
    ) -> Option<(Outcome, String)> {
        let stop = self.stop_flag();
        let spec = Spec {
            executable: exe.to_path_buf(),
            argv: args.iter().map(OsString::from).collect(),
            env: self.env(exe, home, dir.path()),
            stdin: Vec::new(),
            cwd: dir.path().to_path_buf(),
            limits: self.probe,
            shutdown: Some(stop.1.clone()),
        };
        let mut text = String::new();
        let o = Supervisor::run(spec, cancel, &mut |r| {
            text.push_str(&String::from_utf8_lossy(r));
            text.push('\n');
        })
        .ok()?;
        text.push_str(&String::from_utf8_lossy(&o.stderr_tail));
        Some((o, text))
    }

    /// `Ok` only for the allowlisted version.
    fn version(
        &self,
        exe: &Path,
        home: &Path,
        dir: &RunDir,
        cancel: &CancelToken,
    ) -> Result<(), ProfileStatus> {
        let (o, text) = self
            .probe(exe, home, dir, &["--version"], cancel)
            .ok_or(ProfileStatus::Failed)?;
        if o.exit != Exit::Success || o.stop.is_some() || o.escaped > 0 {
            return Err(ProfileStatus::Failed);
        }
        let first = text.lines().next().unwrap_or_default().trim();
        if first == SUPPORTED_VERSION {
            Ok(())
        } else if first.starts_with("codex-cli ") {
            Err(ProfileStatus::UnsupportedVersion)
        } else {
            Err(ProfileStatus::Failed)
        }
    }

    fn check_entry(&self, e: &Entry) -> ProfileStatus {
        if self.active.load(Ordering::SeqCst) > 0 {
            return ProfileStatus::Busy;
        }
        let Some(exe) = self.resolve(e) else {
            return ProfileStatus::Missing;
        };
        let home = e.home.as_ref().map(|h| self.launch_cwd.join(h));
        let Some(home) = home.as_deref().filter(|h| h.is_dir()) else {
            return ProfileStatus::Unavailable;
        };
        let Ok(dir) = RunDir::create(&self.runtime_root) else {
            return ProfileStatus::Failed;
        };
        if let Err(s) = self.version(&exe, home, &dir, &CancelToken::default()) {
            return s;
        }
        match self.probe(&exe, home, &dir, &["login", "status"], &CancelToken::default()) {
            Some((o, _)) if o.stop.is_none() && o.escaped == 0 && o.exit == Exit::Success => {
                ProfileStatus::Ready
            }
            Some((o, text))
                if o.stop.is_none()
                    && o.exit == Exit::Status(1)
                    && text.contains("Not logged in") =>
            {
                ProfileStatus::Unauthenticated
            }
            _ => ProfileStatus::Failed,
        }
    }

    /// Fixed argv for 0.167.0. Values come from assets and private paths only.
    fn argv(
        model: &str,
        effort: &str,
        dir: &Path,
        schema: Option<&Path>,
        last: &Path,
    ) -> Vec<OsString> {
        let s = |v: &str| OsString::from(v);
        let mut a: Vec<OsString> = [
            "exec",
            "--json",
            "--ephemeral",
            "--ignore-user-config",
            "--ignore-rules",
            "--skip-git-repo-check",
            "--sandbox",
            "read-only",
            "--color",
            "never",
            "-C",
        ]
        .into_iter()
        .map(s)
        .collect();
        a.push(dir.into());
        if let Some(schema) = schema {
            a.push(s("--output-schema"));
            a.push(schema.into());
        }
        a.push(s("--output-last-message"));
        a.push(last.into());
        a.push(s("-m"));
        a.push(s(model));
        a.push(s("-c"));
        a.push(s(&format!("model_reasoning_effort=\"{effort}\"")));
        a.push(s("-"));
        a
    }

    fn execute(
        &self,
        request: &AgentRequest,
        events: &mut dyn EventSink,
        run: &Run,
    ) -> Result<AgentOutput, AgentError> {
        let unavailable = || AgentError::from(AgentErrorCode::Unavailable);
        if run.token().is_cancelled() || self.shutdown.load(Ordering::SeqCst) {
            return Err(AgentErrorCode::Cancelled.into());
        }
        let stop = self.stop_flag();
        let handle = request.profile();
        if handle.kind != AgentKind::Codex {
            return Err(unavailable());
        }
        let entry = self.table.get(&handle.alias).ok_or_else(unavailable)?;
        let (chat, name) = match request.mode() {
            Mode::Chat(_) => (true, CHAT),
            Mode::Candidate { .. } => (false, GENERATE),
        };
        let prompt = self
            .assets
            .get(AgentKind::Codex, name)
            .map_err(|_: AssetError| AgentError::from(AgentErrorCode::AssetsUnavailable))?;
        if prompt.model.contains(['"', '\\']) || prompt.reasoning_effort.contains(['"', '\\']) {
            return Err(AgentErrorCode::AssetsUnavailable.into());
        }
        let data = encode_packet(request.mode())?;
        let stdin = prompt.render(request.locale(), Some(&data)).into_bytes();
        let exe = self.resolve(&entry).ok_or_else(unavailable)?;
        // The child runs in its own cwd, so a relative `profile_home` must be made absolute here.
        let home = self
            .launch_cwd
            .join(entry.home.as_deref().ok_or_else(unavailable)?);
        let home = home.as_path();
        if !home.is_dir() {
            return Err(unavailable());
        }
        let dir = RunDir::create(&self.runtime_root)
            .map_err(|_| AgentError::from(AgentErrorCode::Failed))?;
        if let Some(e) = run.accept(AgentEvent::Phase(Phase::StartingAgent)) {
            events.event(e);
        }
        self.version(&exe, home, &dir, &run.token()).map_err(|_| {
            if run.token().is_cancelled() {
                AgentErrorCode::Cancelled.into()
            } else {
                unavailable()
            }
        })?;

        let last = dir.path().join("last-message.txt");
        let schema = if chat {
            None
        } else {
            let p = dir.path().join("schema.json");
            let mut f = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&p)
                .map_err(|_| AgentError::from(AgentErrorCode::Failed))?;
            std::io::Write::write_all(&mut f, GENERATION_SCHEMA.as_bytes())
                .map_err(|_| AgentError::from(AgentErrorCode::Failed))?;
            Some(p)
        };
        let spec = Spec {
            argv: Self::argv(
                &prompt.model,
                &prompt.reasoning_effort,
                dir.path(),
                schema.as_deref(),
                &last,
            ),
            env: self.env(&exe, home, dir.path()),
            executable: exe,
            stdin,
            cwd: dir.path().to_path_buf(),
            limits: self.limits,
            shutdown: Some(stop.1.clone()),
        };

        let mut stream = Stream::default();
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
        if outcome.exit != Exit::Success || stream.failed.is_some() {
            let msg = stream
                .failed
                .clone()
                .or(stream.last_error.clone())
                .unwrap_or_else(|| String::from_utf8_lossy(&outcome.stderr_tail).into_owned());
            return Err(classify(&msg).into());
        }
        if stream.bad || !stream.turn_completed {
            return Err(AgentErrorCode::InvalidOutput.into());
        }
        if let Some(e) = run.accept(AgentEvent::Phase(Phase::ValidatingOutput)) {
            events.event(e);
        }
        let bytes = read_bounded(&last)?;
        match request.mode() {
            Mode::Candidate { packet, .. } => {
                validate_generation(&bytes, packet.snippets().len()).map(AgentOutput::Candidate)
            }
            Mode::Chat(_) => {
                let text = String::from_utf8(bytes)
                    .map_err(|_| AgentError::from(AgentErrorCode::InvalidOutput))?;
                if text.trim().is_empty() {
                    return Err(AgentErrorCode::InvalidOutput.into());
                }
                validate_reply(text)
            }
        }
    }
}

impl AgentRunner for CodexAdapter {
    fn check(&self, profile: &ProfileHandle) -> ProfileStatus {
        match self.table.get(&profile.alias) {
            Some(e) if e.kind == AgentKind::Codex => self.check_entry(&e),
            _ => ProfileStatus::Unavailable,
        }
    }
    fn run(
        &self,
        request: AgentRequest,
        events: &mut dyn EventSink,
        cancel: &CancelToken,
    ) -> Result<AgentOutput, AgentError> {
        self.active.fetch_add(1, Ordering::SeqCst);
        let _active = Active(&self.active);
        let started = Instant::now();
        let run = Run::from(cancel.clone());
        let result = run.finish(self.execute(&request, events, &run));
        if let Some(e) = run.accept(AgentEvent::Phase(Phase::Finished)) {
            events.event(e);
        }
        // Content-free: code tag and duration only.
        let mut rec = Record::new(
            EventCode::AgentRun,
            if result.is_ok() {
                Level::Info
            } else {
                Level::Warn
            },
        )
        .count(u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX));
        if let Err(e) = &result {
            rec = rec.error(code_tag(e.0));
        }
        self.diagnostics.emit(rec);
        result
    }
}

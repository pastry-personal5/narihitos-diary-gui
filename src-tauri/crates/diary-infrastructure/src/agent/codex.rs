//! Codex adapter for `codex-cli 0.167.0` only: status check, fixed-argv one-shot run, NDJSON event
//! parsing and error mapping. Every flag lives in `argv`; no diary or chat text ever reaches argv,
//! logs or `AgentError`. Output is accepted only through the M16 validators and `Run`.
use super::{
    assets::{AssetError, CHAT, GENERATE, PromptAssets},
    host::{Host, RunDir, read_bounded},
    output::{validate_generation, validate_reply},
    packet::encode_packet,
    process::{Exit, Outcome, Spec, Stop, Supervisor},
    profiles::{Entry, ProfileTable},
};
use diary_application::agent::*;
use serde_json::Value;
use std::{
    ffi::OsString,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    sync::Arc,
    time::Instant,
};

pub const SUPPORTED_VERSION: &str = "codex-cli 0.167.0";

pub use super::host::{GENERATION_SCHEMA, classify, sweep_stale_runs};

pub struct CodexAdapter {
    table: Arc<ProfileTable>,
    assets: PromptAssets,
    host: Arc<Host>,
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

impl CodexAdapter {
    pub fn new(table: Arc<ProfileTable>, host: Arc<Host>) -> Self {
        Self {
            table,
            assets: PromptAssets::embedded(),
            host,
        }
    }
    pub fn assets(&self) -> &PromptAssets {
        &self.assets
    }

    fn env(&self, exe: &Path, codex_home: &Path, tmp: &Path) -> Vec<(OsString, OsString)> {
        self.host.env(
            exe,
            tmp,
            vec![("CODEX_HOME".into(), codex_home.as_os_str().to_owned())],
        )
    }

    fn resolve(&self, e: &Entry) -> Option<PathBuf> {
        self.host.resolve(e, "codex")
    }

    fn probe(
        &self,
        exe: &Path,
        home: &Path,
        dir: &RunDir,
        args: &[&str],
        cancel: &CancelToken,
    ) -> Option<(Outcome, String)> {
        self.host
            .probe(exe, self.env(exe, home, dir.path()), dir, args, cancel)
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
        if self.host.is_busy() {
            return ProfileStatus::Busy;
        }
        let Some(exe) = self.resolve(e) else {
            return ProfileStatus::Missing;
        };
        let home = e.home.as_ref().map(|h| self.host.launch_cwd.join(h));
        let Some(home) = home.as_deref().filter(|h| h.is_dir()) else {
            return ProfileStatus::Unavailable;
        };
        let Ok(dir) = RunDir::create(&self.host.runtime_root) else {
            return ProfileStatus::Failed;
        };
        if let Err(s) = self.version(&exe, home, &dir, &CancelToken::default()) {
            return s;
        }
        match self.probe(
            &exe,
            home,
            &dir,
            &["login", "status"],
            &CancelToken::default(),
        ) {
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
        if run.token().is_cancelled() || self.host.is_shutdown() {
            return Err(AgentErrorCode::Cancelled.into());
        }
        let stop = self.host.stop_flag();
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
            .host
            .launch_cwd
            .join(entry.home.as_deref().ok_or_else(unavailable)?);
        let home = home.as_path();
        if !home.is_dir() {
            return Err(unavailable());
        }
        let dir = RunDir::create(&self.host.runtime_root)
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
            limits: self.host.limits,
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
    fn check(&self, profile: &ProfileHandle) -> ProfileCheck {
        match self.table.get(&profile.alias) {
            Some(e) if e.kind == AgentKind::Codex => self.check_entry(&e).into(),
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

//! Claude Code adapter for the `2.1.x` family: status check (`--version`, `auth status`; no model
//! turn), fixed-argv one-shot run on stdin, stream-json parsing and error mapping. No diary or chat
//! text reaches argv, logs or `AgentError`. Output is accepted only through the M16 validators.
use super::{
    assets::{AssetError, CHAT, GENERATE, PromptAssets},
    host::{GENERATION_SCHEMA, Host, RunDir, classify},
    output::{validate_generation, validate_reply},
    packet::encode_packet,
    process::{Exit, Spec, Stop, Supervisor},
    profiles::{Entry, ProfileTable},
};
use diary_application::agent::*;
use serde_json::Value;
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    sync::Arc,
    time::Instant,
};

/// Whether `--version` output is `2.1.<n> (Claude Code)`.
pub fn supported_version(first_line: &str) -> bool {
    let Some(v) = first_line.trim().strip_suffix(" (Claude Code)") else {
        return false;
    };
    let p: Vec<&str> = v.split('.').collect();
    p.len() == 3
        && p[0] == "2"
        && p[1] == "1"
        && !p[2].is_empty()
        && p[2].bytes().all(|b| b.is_ascii_digit())
}

#[derive(Default)]
struct Stream {
    /// `(is_error, text, structured output present, api status, completed)` of the terminal event.
    result: Option<Final>,
    delta_started: bool,
    started: bool,
    bad: bool,
    unknown: u32,
}
struct Final {
    is_error: bool,
    text: String,
    structured: bool,
    status: Option<i64>,
    completed: bool,
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
            Some("system") => {
                if !self.started {
                    self.started = true;
                    emit(AgentEvent::Phase(Phase::WaitingForOutput));
                }
            }
            Some("stream_event") => {
                // Text deltas exist only when partial messages are requested (chat).
                if chat
                    && v["event"]["type"] == "content_block_delta"
                    && v["event"]["delta"]["type"] == "text_delta"
                    && let Some(t) = v["event"]["delta"]["text"].as_str()
                {
                    if !self.delta_started {
                        self.delta_started = true;
                        emit(AgentEvent::Phase(Phase::ReceivingOutput));
                    }
                    emit(AgentEvent::Delta(t.to_owned()));
                }
            }
            Some("assistant") => {
                if !self.delta_started {
                    self.delta_started = true;
                    emit(AgentEvent::Phase(Phase::ReceivingOutput));
                }
            }
            Some("user" | "rate_limit_event") => {}
            Some("result") => {
                self.result = Some(Final {
                    is_error: v["is_error"].as_bool() != Some(false),
                    text: v["result"].as_str().unwrap_or_default().to_owned(),
                    structured: v["structured_output"].is_object(),
                    status: v["api_error_status"].as_i64(),
                    completed: v["subtype"] == "success" && v["terminal_reason"] == "completed",
                });
            }
            _ => self.unknown += 1,
        }
    }
    /// Class of a failed terminal event; the status code joins the message so `classify` sees it.
    fn failure(&self) -> Option<AgentErrorCode> {
        let r = self.result.as_ref().filter(|r| r.is_error)?;
        let status = r.status.map(|s| format!(" {s}")).unwrap_or_default();
        Some(classify(&format!("{}{status}", r.text)))
    }
}

/// What a recorded stream says, for fixture replay: the same parser the adapter uses.
#[derive(Debug, PartialEq, Eq)]
pub struct Summary {
    pub result_seen: bool,
    pub completed: bool,
    pub structured_output: bool,
    /// Classified error result; `None` when the result is not an error.
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
        result_seen: stream.result.is_some(),
        completed: stream.result.as_ref().is_some_and(|r| r.completed),
        structured_output: stream.result.as_ref().is_some_and(|r| r.structured),
        failure: stream.failure(),
        malformed: stream.bad,
        unknown_events: stream.unknown,
    }
}

pub struct ClaudeAdapter {
    table: Arc<ProfileTable>,
    assets: PromptAssets,
    host: Arc<Host>,
    user: Option<OsString>,
}

impl ClaudeAdapter {
    pub fn new(table: Arc<ProfileTable>, host: Arc<Host>) -> Self {
        Self {
            table,
            assets: PromptAssets::embedded(),
            host,
            // The macOS keychain lookup of the login needs USER; LOGNAME is deliberately not passed.
            user: std::env::var_os("USER"),
        }
    }
    pub fn assets(&self) -> &PromptAssets {
        &self.assets
    }

    /// The owner's default login is keyed to an unset `CLAUDE_CONFIG_DIR`, so the variable is
    /// omitted when `profile_home` is `$HOME/.claude` and set for any other directory.
    fn env(&self, exe: &Path, home: &Path, tmp: &Path) -> Vec<(OsString, OsString)> {
        let default = self.host.home.as_ref().is_some_and(|h| {
            Path::new(h)
                .join(".claude")
                .canonicalize()
                .is_ok_and(|d| home.canonicalize().is_ok_and(|p| p == d))
        });
        let mut extra: Vec<(OsString, OsString)> = vec![("DISABLE_AUTOUPDATER".into(), "1".into())];
        if !default {
            extra.push(("CLAUDE_CONFIG_DIR".into(), home.as_os_str().to_owned()));
        }
        extra.extend(self.user.clone().map(|u| ("USER".into(), u)));
        self.host.env(exe, tmp, extra)
    }

    fn probe(
        &self,
        exe: &Path,
        home: &Path,
        dir: &RunDir,
        args: &[&str],
        cancel: &CancelToken,
    ) -> Option<(super::process::Outcome, String)> {
        self.host
            .probe(exe, self.env(exe, home, dir.path()), dir, args, cancel)
    }

    fn check_entry(&self, e: &Entry) -> ProfileStatus {
        if self.host.is_busy() {
            return ProfileStatus::Busy;
        }
        let Some(exe) = self.host.resolve(e, "claude") else {
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
        let Some((o, text)) = self.probe(
            &exe,
            home,
            &dir,
            &["auth", "status"],
            &CancelToken::default(),
        ) else {
            return ProfileStatus::Failed;
        };
        if o.stop.is_some() || o.escaped > 0 {
            return ProfileStatus::Failed;
        }
        // shortcut: first `{` to last `}` of stdout plus stderr tail; upgrade if stderr gains braces.
        let json = text
            .find('{')
            .zip(text.rfind('}'))
            .and_then(|(a, b)| serde_json::from_str::<Value>(text.get(a..=b)?).ok());
        match (json.as_ref().and_then(|v| v["loggedIn"].as_bool()), o.exit) {
            (Some(true), Exit::Success) => ProfileStatus::Ready,
            (Some(false), Exit::Status(1)) => ProfileStatus::Unauthenticated,
            _ => ProfileStatus::Failed,
        }
    }

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
        let first = text.lines().next().unwrap_or_default();
        if supported_version(first) {
            Ok(())
        } else if first.trim().ends_with("(Claude Code)") {
            Err(ProfileStatus::UnsupportedVersion)
        } else {
            Err(ProfileStatus::Failed)
        }
    }

    /// Fixed argv. Values come from assets and the schema constant only. Chat streams text deltas.
    fn argv(model: &str, effort: &str, chat: bool) -> Vec<OsString> {
        let s = OsString::from;
        let mut a: Vec<OsString> = [
            "-p",
            "--output-format",
            "stream-json",
            "--verbose",
            "--no-session-persistence",
            "--setting-sources",
            "",
            "--strict-mcp-config",
            "--disable-slash-commands",
            "--tools",
            "",
            "--permission-mode",
            "dontAsk",
        ]
        .into_iter()
        .map(s)
        .collect();
        if chat {
            a.push(s("--include-partial-messages"));
        } else {
            a.extend([s("--json-schema"), s(GENERATION_SCHEMA)]);
        }
        a.extend([s("--model"), s(model), s("--effort"), s(effort)]);
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
        if handle.kind != AgentKind::Claude {
            return Err(unavailable());
        }
        let entry = self.table.get(&handle.alias).ok_or_else(unavailable)?;
        let (chat, name) = match request.mode() {
            Mode::Chat(_) => (true, CHAT),
            Mode::Candidate { .. } => (false, GENERATE),
        };
        let prompt = self
            .assets
            .get(AgentKind::Claude, name)
            .map_err(|_: AssetError| AgentError::from(AgentErrorCode::AssetsUnavailable))?;
        let data = encode_packet(request.mode())?;
        let stdin = prompt.render(request.locale(), Some(&data)).into_bytes();
        let exe = self
            .host
            .resolve(&entry, "claude")
            .ok_or_else(unavailable)?;
        let home: PathBuf = self
            .host
            .launch_cwd
            .join(entry.home.as_deref().ok_or_else(unavailable)?);
        if !home.is_dir() {
            return Err(unavailable());
        }
        let dir = RunDir::create(&self.host.runtime_root)
            .map_err(|_| AgentError::from(AgentErrorCode::Failed))?;
        if let Some(e) = run.accept(AgentEvent::Phase(Phase::StartingAgent)) {
            events.event(e);
        }
        self.version(&exe, &home, &dir, &run.token()).map_err(|_| {
            if run.token().is_cancelled() {
                AgentErrorCode::Cancelled.into()
            } else {
                unavailable()
            }
        })?;
        let spec = Spec {
            argv: Self::argv(&prompt.model, &prompt.reasoning_effort, chat),
            env: self.env(&exe, &home, dir.path()),
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
        if let Some(code) = stream.failure() {
            return Err(code.into());
        }
        if outcome.exit != Exit::Success {
            let msg = String::from_utf8_lossy(&outcome.stderr_tail).into_owned();
            return Err(classify(&msg).into());
        }
        let Some(fin) = stream.result.filter(|r| r.completed) else {
            return Err(AgentErrorCode::InvalidOutput.into());
        };
        if stream.bad || fin.text.len() > MAX_RESULT_BYTES {
            return Err(AgentErrorCode::InvalidOutput.into());
        }
        if let Some(e) = run.accept(AgentEvent::Phase(Phase::ValidatingOutput)) {
            events.event(e);
        }
        match request.mode() {
            Mode::Candidate { packet, .. } => {
                if !fin.structured {
                    return Err(AgentErrorCode::InvalidOutput.into());
                }
                validate_generation(fin.text.as_bytes(), packet.snippets().len())
                    .map(AgentOutput::Candidate)
            }
            Mode::Chat(_) => {
                if fin.text.trim().is_empty() {
                    return Err(AgentErrorCode::InvalidOutput.into());
                }
                validate_reply(fin.text)
            }
        }
    }
}

impl AgentRunner for ClaudeAdapter {
    fn check(&self, profile: &ProfileHandle) -> ProfileCheck {
        match self.table.get(&profile.alias) {
            Some(e) if e.kind == AgentKind::Claude => self.check_entry(&e).into(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_family_is_2_1_x() {
        for ok in ["2.1.319 (Claude Code)", "2.1.0 (Claude Code)"] {
            assert!(supported_version(ok), "{ok}");
        }
        for bad in [
            "2.2.0 (Claude Code)",
            "3.1.1 (Claude Code)",
            "2.1. (Claude Code)",
            "2.1.x (Claude Code)",
            "2.1.1.1 (Claude Code)",
            "2.1.319",
            "codex-cli 0.167.0",
            "",
        ] {
            assert!(!supported_version(bad), "{bad}");
        }
    }

    #[test]
    fn argv_has_no_user_text_and_disables_tools() {
        for chat in [false, true] {
            let a: Vec<String> = ClaudeAdapter::argv("m", "low", chat)
                .iter()
                .map(|s| s.to_string_lossy().into_owned())
                .collect();
            let pos = |f: &str| a.iter().position(|x| x == f).unwrap();
            assert_eq!(a[pos("--tools") + 1], "");
            assert_eq!(a[pos("--setting-sources") + 1], "");
            assert_eq!(a[pos("--model") + 1], "m");
            for f in [
                "--no-session-persistence",
                "--strict-mcp-config",
                "--disable-slash-commands",
            ] {
                assert!(a.iter().any(|x| x == f), "{f}");
            }
            assert!(!a.iter().any(|x| x == "--bare" || x == "--max-budget-usd"));
            assert_eq!(a.iter().any(|x| x == "--json-schema"), !chat);
            assert_eq!(a.iter().any(|x| x == "--include-partial-messages"), chat);
        }
    }
}

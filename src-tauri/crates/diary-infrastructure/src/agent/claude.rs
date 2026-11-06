//! Claude Code adapter for the `2.1.x` family: status check (`--version`, `auth status`; no model
//! turn), fixed-argv one-shot run on stdin, stream-json parsing and error mapping. No diary or chat
//! text reaches argv, logs or `AgentError`. Output is accepted only through the M16 validators.
use super::{
    assets::Prompt,
    engine::{Adapter, Collected, Ctx, Invocation, Protocol, Stream},
    host::{GENERATION_SCHEMA, Host, RunDir, classify},
    process::{Exit, Outcome},
    profiles::ProfileTable,
};
use diary_application::agent::*;
use serde_json::Value;
use std::{ffi::OsString, path::Path, sync::Arc};

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
struct ClaudeStream {
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
impl ClaudeStream {
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
    let mut stream = ClaudeStream::default();
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

impl Stream for ClaudeStream {
    fn record(&mut self, rec: &[u8], chat: bool, run: &Run, sink: &mut dyn EventSink) {
        ClaudeStream::record(self, rec, chat, run, sink)
    }
    fn conclude(&mut self, o: &Outcome, _: &Invocation, _: &Prompt) -> Result<(), AgentError> {
        if let Some(code) = self.failure() {
            return Err(code.into());
        }
        if o.exit != Exit::Success {
            let msg = String::from_utf8_lossy(&o.stderr_tail).into_owned();
            return Err(classify(&msg).into());
        }
        let ok = self.result.as_ref().is_some_and(|r| r.completed);
        if !ok
            || self.bad
            || self
                .result
                .as_ref()
                .is_some_and(|r| r.text.len() > MAX_RESULT_BYTES)
        {
            return Err(AgentErrorCode::InvalidOutput.into());
        }
        Ok(())
    }
    fn collect(&mut self, _: &Invocation) -> Result<Collected, AgentError> {
        let fin = self.result.take().expect("concluded with a result");
        Ok(Collected {
            bytes: fin.text.into_bytes(),
            structured: Some(fin.structured),
        })
    }
}

pub struct Claude {
    user: Option<OsString>,
}
pub type ClaudeAdapter = Adapter<Claude>;
impl ClaudeAdapter {
    pub fn new(table: Arc<ProfileTable>, host: Arc<Host>) -> Self {
        // The macOS keychain lookup of the login needs USER; LOGNAME is deliberately not passed.
        let user = std::env::var_os("USER");
        Adapter::with_protocol(table, host, Claude { user })
    }
}

impl Claude {
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
}

impl Protocol for Claude {
    fn kind(&self) -> AgentKind {
        AgentKind::Claude
    }
    /// The owner's default login is keyed to an unset `CLAUDE_CONFIG_DIR`, so the variable is
    /// omitted when `profile_home` is `$HOME/.claude` and set for any other directory.
    fn env(&self, host: &Host, exe: &Path, home: &Path, tmp: &Path) -> Vec<(OsString, OsString)> {
        let default = host.home.as_ref().is_some_and(|h| {
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
        host.env(exe, tmp, extra)
    }
    fn version(&self, ctx: &Ctx, cancel: &CancelToken) -> Result<(), ProfileStatus> {
        let (o, text) = ctx
            .probe(&["--version"], cancel)
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
    fn auth(&self, ctx: &Ctx) -> ProfileStatus {
        let Some((o, text)) = ctx.probe(&["auth", "status"], &CancelToken::default()) else {
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
    fn invocation(
        &self,
        _: &RunDir,
        prompt: &Prompt,
        chat: bool,
    ) -> Result<Invocation, AgentError> {
        Ok(Invocation {
            argv: Self::argv(&prompt.model, &prompt.reasoning_effort, chat),
            result_file: None,
        })
    }
    fn stream(&self) -> Box<dyn Stream> {
        Box::new(ClaudeStream::default())
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
            let a: Vec<String> = Claude::argv("m", "low", chat)
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

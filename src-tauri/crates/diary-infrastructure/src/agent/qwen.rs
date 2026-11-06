//! Qwen Code adapter for the `0.26.x` family: status check (`--version` only; no auth probe exists,
//! so `ready` means installed and supported), fixed-argv one-shot run on stdin, stream-json parsing
//! and error mapping. No diary or chat text reaches argv, logs or `AgentError`. Output is accepted
//! only through the M16 validators. The CLI silently falls back to the profile's configured model
//! when `--model` is unknown, so the `system.init` record must report the asset's model.
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

/// Run-level budgets passed to the CLI so it ends first; the supervisor's deadline stays the backstop.
const MAX_SESSION_TURNS: &str = "4";
const MAX_WALL_TIME: &str = "540s";

/// Whether `--version` output is `0.26.<n>`.
pub fn supported_version(v: &str) -> bool {
    let p: Vec<&str> = v.trim().split('.').collect();
    p.len() == 3
        && p[0] == "0"
        && p[1] == "26"
        && !p[2].is_empty()
        && p[2].bytes().all(|b| b.is_ascii_digit())
}

/// Endpoint and credential failures are not rate or context limits: they mean the profile cannot run.
fn qwen_classify(message: &str) -> AgentErrorCode {
    let m = message.to_ascii_lowercase();
    if [
        "no auth type",
        "econnrefused",
        "connection refused",
        "fetch failed",
        "forbidden",
    ]
    .iter()
    .any(|n| m.contains(n))
    {
        AgentErrorCode::Unavailable
    } else {
        classify(message)
    }
}

#[derive(Default)]
struct QwenStream {
    result: Option<Final>,
    /// `(model, version)` of the `system.init` record.
    init: Option<(String, String)>,
    delta_started: bool,
    bad: bool,
    unknown: u32,
}
struct Final {
    is_error: bool,
    text: String,
    structured: bool,
    completed: bool,
    error: String,
}
impl QwenStream {
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
            // Only the `init` record names the model and version; other system records must not stand in for it.
            Some("system") if v["subtype"] == "init" => {
                if self.init.is_none() {
                    let s = |k: &str| v[k].as_str().unwrap_or_default().to_owned();
                    self.init = Some((s("model"), s("qwen_code_version")));
                    emit(AgentEvent::Phase(Phase::WaitingForOutput));
                }
            }
            Some("stream_event") => {
                // Reasoning arrives as `thinking_delta`; only answer text is forwarded, and only for chat.
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
            Some("user") => {}
            Some("result") => {
                self.result = Some(Final {
                    is_error: v["is_error"].as_bool() != Some(false),
                    text: v["result"].as_str().unwrap_or_default().to_owned(),
                    structured: v["structured_result"].is_object(),
                    completed: v["subtype"] == "success",
                    error: v["error"]["message"]
                        .as_str()
                        .unwrap_or_default()
                        .to_owned(),
                });
            }
            _ => self.unknown += 1,
        }
    }
    /// Class of a failed terminal event, from its error message or result text.
    fn failure(&self) -> Option<AgentErrorCode> {
        let r = self.result.as_ref().filter(|r| r.is_error)?;
        Some(qwen_classify(if r.error.is_empty() {
            &r.text
        } else {
            &r.error
        }))
    }
    /// The run reported another model or version than the asset and allowlist (F15).
    fn init_mismatch(&self, model: &str) -> bool {
        self.init
            .as_ref()
            .is_some_and(|(m, v)| m != model || !supported_version(v))
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
    pub init: Option<(String, String)>,
}
pub fn summarize(ndjson: &[u8]) -> Summary {
    let run = Run::new();
    struct Null;
    impl EventSink for Null {
        fn event(&mut self, _: AgentEvent) {}
    }
    let mut stream = QwenStream::default();
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
        init: stream.init,
    }
}

impl Stream for QwenStream {
    fn record(&mut self, rec: &[u8], chat: bool, run: &Run, sink: &mut dyn EventSink) {
        QwenStream::record(self, rec, chat, run, sink)
    }
    fn conclude(&mut self, o: &Outcome, _: &Invocation, prompt: &Prompt) -> Result<(), AgentError> {
        if self.init_mismatch(&prompt.model) {
            return Err(AgentErrorCode::Unavailable.into());
        }
        if let Some(code) = self.failure() {
            return Err(code.into());
        }
        if o.exit != Exit::Success {
            // Exit 55 (a run budget) carries no result record; stderr only classifies it.
            let msg = String::from_utf8_lossy(&o.stderr_tail).into_owned();
            return Err(qwen_classify(&msg).into());
        }
        let ok = self.result.as_ref().is_some_and(|r| r.completed);
        if !ok
            || self.init.is_none()
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

pub struct Qwen;
pub type QwenAdapter = Adapter<Qwen>;
impl QwenAdapter {
    pub fn new(table: Arc<ProfileTable>, host: Arc<Host>) -> Self {
        Adapter::with_protocol(table, host, Qwen)
    }
}

impl Qwen {
    /// Fixed argv. Values come from assets and the schema constant only. Chat streams text deltas.
    /// No `--bare` (drops the endpoint), `--approval-mode plan` (aborts the run) or key flag.
    fn argv(model: &str, chat: bool) -> Vec<OsString> {
        let s = OsString::from;
        let mut a: Vec<OsString> = [
            "--safe-mode",
            "--output-format",
            "stream-json",
            "--max-tool-calls",
            "0",
            "--max-session-turns",
            MAX_SESSION_TURNS,
            "--max-wall-time",
            MAX_WALL_TIME,
            "--chat-recording=false",
        ]
        .into_iter()
        .map(s)
        .collect();
        if chat {
            a.push(s("--include-partial-messages"));
        } else {
            a.extend([s("--json-schema"), s(GENERATION_SCHEMA)]);
        }
        a.extend([s("--model"), s(model)]);
        a
    }
}

impl Protocol for Qwen {
    fn kind(&self) -> AgentKind {
        AgentKind::Qwen
    }
    fn default_binary(&self) -> &'static str {
        "qwen"
    }
    /// `QWEN_HOME` is always set (it also stops the CLI reading `.env` files for an override);
    /// telemetry is forced off because the environment beats settings. No key is ever passed.
    fn env(&self, host: &Host, exe: &Path, home: &Path, tmp: &Path) -> Vec<(OsString, OsString)> {
        host.env(
            exe,
            tmp,
            vec![
                ("QWEN_HOME".into(), home.as_os_str().to_owned()),
                ("QWEN_TELEMETRY_ENABLED".into(), "0".into()),
            ],
        )
    }
    fn version(&self, ctx: &Ctx, cancel: &CancelToken) -> Result<(), ProfileStatus> {
        let (o, text) = ctx
            .probe(&["--version"], cancel)
            .ok_or(ProfileStatus::Failed)?;
        if o.exit != Exit::Success || o.stop.is_some() || o.escaped > 0 {
            return Err(ProfileStatus::Failed);
        }
        let first = text.lines().next().unwrap_or_default().trim();
        let numeric = first.split('.').count() == 3
            && first
                .split('.')
                .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()));
        if supported_version(first) {
            Ok(())
        } else if numeric {
            Err(ProfileStatus::UnsupportedVersion)
        } else {
            Err(ProfileStatus::Failed)
        }
    }
    fn invocation(
        &self,
        _: &RunDir,
        prompt: &Prompt,
        chat: bool,
    ) -> Result<Invocation, AgentError> {
        Ok(Invocation {
            argv: Self::argv(&prompt.model, chat),
            result_file: None,
        })
    }
    fn stream(&self) -> Box<dyn Stream> {
        Box::new(QwenStream::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_family_is_0_26_x() {
        for ok in ["0.26.4", "0.26.0", " 0.26.12\n"] {
            assert!(supported_version(ok), "{ok}");
        }
        for bad in [
            "0.27.0", "1.26.4", "0.26.", "0.26.x", "0.26.4.1", "0.26", "v0.26.4", "", "2.1.319",
        ] {
            assert!(!supported_version(bad), "{bad}");
        }
    }

    #[test]
    fn cli_wall_budget_ends_before_the_supervisor_deadline() {
        let secs: u64 = MAX_WALL_TIME.trim_end_matches('s').parse().unwrap();
        assert!(secs < diary_application::agent::WALL_SECS);
    }

    #[test]
    fn argv_has_no_user_text_no_key_and_disables_tools() {
        for chat in [false, true] {
            let a: Vec<String> = Qwen::argv("m", chat)
                .iter()
                .map(|s| s.to_string_lossy().into_owned())
                .collect();
            let pos = |f: &str| a.iter().position(|x| x == f).unwrap();
            assert_eq!(a[pos("--max-tool-calls") + 1], "0");
            assert_eq!(a[pos("--model") + 1], "m");
            assert!(a.iter().any(|x| x == "--safe-mode"));
            assert!(a.iter().any(|x| x == "--chat-recording=false"));
            for banned in [
                "--bare",
                "--approval-mode",
                "--exclude-tools",
                "--openai-api-key",
                "--openai-base-url",
                "-p",
                "--prompt",
                "--yolo",
            ] {
                assert!(!a.iter().any(|x| x == banned), "{banned}");
            }
            assert_eq!(a.iter().any(|x| x == "--json-schema"), !chat);
            assert_eq!(a.iter().any(|x| x == "--include-partial-messages"), chat);
        }
    }

    #[test]
    fn endpoint_and_credential_failures_are_unavailable() {
        for m in [
            "No auth type is selected. Please configure",
            "connect ECONNREFUSED 127.0.0.1:8186",
            "fetch failed",
            "401 Unauthorized",
        ] {
            assert_eq!(qwen_classify(m), AgentErrorCode::Unavailable, "{m}");
        }
        assert_eq!(
            qwen_classify("429 too many requests"),
            AgentErrorCode::RateLimited
        );
        assert_eq!(qwen_classify("odd"), AgentErrorCode::Failed);
    }
}

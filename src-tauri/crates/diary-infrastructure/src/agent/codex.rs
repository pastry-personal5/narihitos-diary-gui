//! Codex adapter for `codex-cli 0.167.0` only: status check, fixed-argv one-shot run, NDJSON event
//! parsing and error mapping. Every flag lives in `argv`; no diary or chat text ever reaches argv,
//! logs or `AgentError`. Output is accepted only through the M16 validators and `Run`.
use super::{
    assets::Prompt,
    engine::{Adapter, Collected, Ctx, Invocation, Protocol, Stream, auth_failed, probe_fault},
    host::{Host, RunDir, read_bounded},
    process::{Exit, Outcome},
    profiles::ProfileTable,
};
use diary_application::agent::*;
use serde_json::Value;
use std::{ffi::OsString, os::unix::fs::OpenOptionsExt, path::Path, sync::Arc};

pub const SUPPORTED_VERSION: &str = "codex-cli 0.167.0";

pub use super::host::{GENERATION_SCHEMA, classify, sweep_stale_runs};

#[derive(Default)]
struct CodexStream {
    turn_completed: bool,
    failed: Option<String>,
    last_error: Option<String>,
    bad: bool,
    unknown: u32,
}
impl CodexStream {
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
    let mut stream = CodexStream::default();
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

impl Stream for CodexStream {
    fn record(&mut self, rec: &[u8], chat: bool, run: &Run, sink: &mut dyn EventSink) {
        CodexStream::record(self, rec, chat, run, sink)
    }
    fn conclude(&mut self, o: &Outcome, _: &Invocation, _: &Prompt) -> Result<(), AgentError> {
        if o.exit != Exit::Success || self.failed.is_some() {
            let msg = self
                .failed
                .clone()
                .or(self.last_error.clone())
                .unwrap_or_else(|| String::from_utf8_lossy(&o.stderr_tail).into_owned());
            return Err(classify(&msg).into());
        }
        if self.bad || !self.turn_completed {
            return Err(AgentErrorCode::InvalidOutput.into());
        }
        Ok(())
    }
    fn collect(&mut self, inv: &Invocation) -> Result<Collected, AgentError> {
        let last = inv
            .result_file
            .as_deref()
            .expect("codex sets a result file");
        Ok(Collected {
            bytes: read_bounded(last)?,
            structured: None,
        })
    }
}

pub struct Codex;
pub type CodexAdapter = Adapter<Codex>;
impl CodexAdapter {
    pub fn new(table: Arc<ProfileTable>, host: Arc<Host>) -> Self {
        Adapter::with_protocol(table, host, Codex)
    }
    pub fn with_prompt_assets(
        table: Arc<ProfileTable>,
        host: Arc<Host>,
        assets: super::assets::PromptAssets,
    ) -> Self {
        Adapter::with_assets(table, host, Codex, assets)
    }
}

impl Codex {
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
}

impl Protocol for Codex {
    fn kind(&self) -> AgentKind {
        AgentKind::Codex
    }
    fn env(&self, host: &Host, exe: &Path, home: &Path, tmp: &Path) -> Vec<(OsString, OsString)> {
        host.env(
            exe,
            tmp,
            vec![("CODEX_HOME".into(), home.as_os_str().to_owned())],
        )
    }
    /// `Ok` only for the allowlisted version.
    fn version(&self, ctx: &Ctx, cancel: &CancelToken) -> Result<(), Reason> {
        let (o, text) = ctx
            .probe(&["--version"], cancel)
            .ok_or(Reason::SpawnFailed)?;
        if o.exit != Exit::Success || o.stop.is_some() || o.escaped > 0 {
            return Err(probe_fault(&o));
        }
        let first = text.lines().next().unwrap_or_default().trim();
        if first == SUPPORTED_VERSION {
            Ok(())
        } else if first.starts_with("codex-cli ") {
            Err(Reason::VersionUnsupported)
        } else {
            Err(Reason::VersionUnreadable)
        }
    }
    fn auth(&self, ctx: &Ctx) -> ProfileCheck {
        match ctx.probe(&["login", "status"], &CancelToken::default()) {
            Some((o, _)) if o.stop.is_none() && o.escaped == 0 && o.exit == Exit::Success => {
                ProfileStatus::Ready.into()
            }
            Some((o, text))
                if o.stop.is_none()
                    && o.exit == Exit::Status(1)
                    && text.contains("Not logged in") =>
            {
                ProfileStatus::Unauthenticated.into()
            }
            other => auth_failed(other.as_ref().map(|(o, _)| o)),
        }
    }
    fn check_prompt(&self, prompt: &Prompt) -> Result<(), Reason> {
        // The effort is spliced into a `-c key="value"` argument, so quoting characters are refused.
        if prompt.model.contains(['"', '\\']) || prompt.reasoning_effort.contains(['"', '\\']) {
            return Err(Reason::PromptPinRejected);
        }
        Ok(())
    }
    fn invocation(
        &self,
        dir: &RunDir,
        prompt: &Prompt,
        chat: bool,
    ) -> Result<Invocation, AgentError> {
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
        Ok(Invocation {
            argv: Self::argv(
                &prompt.model,
                &prompt.reasoning_effort,
                dir.path(),
                schema.as_deref(),
                &last,
            ),
            result_file: Some(last),
        })
    }
    fn stream(&self) -> Box<dyn Stream> {
        Box::new(CodexStream::default())
    }
}

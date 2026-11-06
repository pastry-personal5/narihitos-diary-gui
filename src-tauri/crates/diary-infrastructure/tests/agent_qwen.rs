//! Qwen adapter against the `fake_cli` example standing in for `qwen` (scenario in `$QWEN_HOME`).
use diary_application::{
    agent::*,
    diagnostics::{Diagnostics, EventCode, Record},
};
use diary_domain::authentic::DiaryDate;
mod support;
use diary_infrastructure::agent::{
    host::Host,
    host::GENERATION_SCHEMA,
    process::Limits,
    profiles::{Entry, ProfileTable},
    qwen::QwenAdapter,
};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

const SECRET: &str = "SECRET-DIARY-TEXT";

fn fake() -> PathBuf {
    let p = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples/fake_cli");
    assert!(
        p.is_file(),
        "build examples first: cargo test builds fake_cli"
    );
    p
}

#[derive(Default)]
struct Logs(Mutex<Vec<Record>>);
impl Diagnostics for Logs {
    fn emit(&self, r: Record) {
        self.0.lock().unwrap().push(r);
    }
}

struct Rig {
    root: tempfile::TempDir,
    adapter: QwenAdapter,
    /// Owns the shared host; lease end and shutdown go through it.
    host: Arc<Host>,
    logs: Arc<Logs>,
}
impl Rig {
    fn home(&self) -> PathBuf {
        self.root.path().join("home")
    }
    fn set(&self, name: &str, value: &str) {
        std::fs::write(self.home().join(name), value).unwrap();
    }
}
fn limits() -> Limits {
    Limits {
        inactivity: Duration::from_millis(1500),
        wall: Duration::from_secs(20),
        record: 1024 * 1024,
        total: 1024 * 1024,
        grace: Duration::from_millis(300),
        drain: Duration::from_millis(300),
    }
}
fn rig_with(exe: Option<PathBuf>, make_home: bool) -> Rig {
    let root = tempfile::tempdir().unwrap();
    let home = root.path().join("home");
    if make_home {
        std::fs::create_dir(&home).unwrap();
    }
    std::fs::create_dir(root.path().join("runtime")).unwrap();
    let table = Arc::new(ProfileTable::from_entries(vec![Entry {
        alias: "main".into(),
        kind: AgentKind::Qwen,
        home: Some(home),
        executable: Some(exe.unwrap_or_else(fake)),
    }]));
    let logs = Arc::new(Logs::default());
    let host = support::host(root.path(), logs.clone(), limits());
    let adapter = QwenAdapter::new(table, host.clone());
    Rig {
        host,
        root,
        adapter,
        logs,
    }
}
fn rig() -> Rig {
    rig_with(None, true)
}

fn handle() -> ProfileHandle {
    ProfileHandle {
        alias: "main".into(),
        kind: AgentKind::Qwen,
    }
}
fn generate_request() -> AgentRequest {
    let snip = SnippetItem {
        id: "snippet-1".into(),
        instant_utc: "2026-11-06T01:00:00Z".into(),
        civil_date: "2026-11-06".into(),
        offset: "+09:00".into(),
        body: SECRET.into(),
    };
    let packet = GenerationPacket::new(
        Style::Standard,
        DiaryDate::from_ymd(2026, 11, 6).unwrap(),
        vec![snip],
    )
    .unwrap();
    AgentRequest::new(
        handle(),
        Locale::parse("en").unwrap(),
        Mode::Candidate {
            index: 0,
            count: 1,
            packet,
        },
    )
}
fn chat_request() -> AgentRequest {
    AgentRequest::new(
        handle(),
        Locale::parse("en").unwrap(),
        Mode::Chat(ChatPacket::new(None, vec![], SECRET.into()).unwrap()),
    )
}

#[derive(Default)]
struct Sink(Vec<AgentEvent>);
impl EventSink for Sink {
    fn event(&mut self, e: AgentEvent) {
        self.0.push(e);
    }
}
fn run(r: &Rig, req: AgentRequest) -> Result<AgentOutput, AgentError> {
    r.adapter
        .run(req, &mut Sink::default(), &CancelToken::default())
}
fn code(r: Result<AgentOutput, AgentError>) -> AgentErrorCode {
    r.expect_err("expected an error").0
}

#[test]
fn qwen_check_maps_each_status() {
    let r = rig();
    assert_eq!(r.adapter.check(&handle()).status, ProfileStatus::Ready);
    for bad in ["garbage", "v0.26.4"] {
        r.set("version", bad);
        assert_eq!(r.adapter.check(&handle()).status, ProfileStatus::Failed, "{bad:?}");
    }
    assert_eq!(
        rig_with(Some("/nonexistent/qwen".into()), true)
            .adapter
            .check(&handle()).status,
        ProfileStatus::Missing
    );
    assert_eq!(
        rig_with(None, false).adapter.check(&handle()).status,
        ProfileStatus::Unavailable
    );
    let other = ProfileHandle {
        alias: "nope".into(),
        kind: AgentKind::Qwen,
    };
    assert_eq!(r.adapter.check(&other).status, ProfileStatus::Unavailable);
}

#[test]
fn qwen_unknown_version_is_unsupported() {
    let r = rig();
    for v in ["0.25.9", "0.27.0", "1.0.0"] {
        r.set("version", v);
        assert_eq!(
            r.adapter.check(&handle()).status,
            ProfileStatus::UnsupportedVersion,
            "{v}"
        );
    }
    r.set("version", "0.26.7");
    assert_eq!(r.adapter.check(&handle()).status, ProfileStatus::Ready);
    r.set("version", "0.27.0");
    assert_eq!(code(run(&r, chat_request())), AgentErrorCode::Unavailable);
}

#[test]
fn qwen_argv_is_fixed_and_contains_no_user_text() {
    let r = rig();
    run(&r, generate_request()).unwrap();
    let argv = std::fs::read_to_string(r.home().join("argv")).unwrap();
    assert!(!argv.contains(SECRET));
    let args: Vec<&str> = argv.split('\n').collect();
    assert_eq!(
        args,
        [
            "--safe-mode",
            "--output-format",
            "stream-json",
            "--max-tool-calls",
            "0",
            "--max-session-turns",
            "4",
            "--max-wall-time",
            "540s",
            "--chat-recording=false",
            "--json-schema",
            GENERATION_SCHEMA,
            "--model",
            "qwen3.6:35b-a3b"
        ]
    );
    let stdin = std::fs::read_to_string(r.home().join("stdin")).unwrap();
    assert!(stdin.contains(SECRET), "the prompt travels on stdin");
}

#[test]
fn qwen_runs_in_private_cwd_with_allowlisted_env() {
    let r = rig();
    run(&r, chat_request()).unwrap();
    let cwd = std::fs::read_to_string(r.home().join("cwd")).unwrap();
    assert!(cwd.contains("agent-runs"), "{cwd}");
    let env = std::fs::read_to_string(r.home().join("env")).unwrap();
    let mut keys: Vec<&str> = env.lines().collect();
    keys.sort_unstable();
    for k in keys {
        assert!(
            [
                "PATH",
                "TMPDIR",
                "QWEN_HOME",
                "QWEN_TELEMETRY_ENABLED",
                "HOME",
                "LANG"
            ]
            .contains(&k),
            "unexpected variable {k}"
        );
    }
    assert!(env.contains("QWEN_HOME") && env.contains("QWEN_TELEMETRY_ENABLED"));
}

#[test]
fn qwen_chat_run_omits_the_schema_and_streams_deltas() {
    let r = rig();
    let mut sink = Sink::default();
    let out = r
        .adapter
        .run(chat_request(), &mut sink, &CancelToken::default())
        .unwrap();
    assert_eq!(out, AgentOutput::Reply("A reply.".into()));
    let argv = std::fs::read_to_string(r.home().join("argv")).unwrap();
    assert!(!argv.contains("--json-schema") && argv.contains("--include-partial-messages"));
    let deltas: Vec<_> = sink
        .0
        .iter()
        .filter_map(|e| match e {
            AgentEvent::Delta(t) => Some(t.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(deltas, ["A ", "reply."]);
}

#[test]
fn qwen_generation_run_returns_a_validated_candidate() {
    let r = rig();
    let mut sink = Sink::default();
    let out = r
        .adapter
        .run(generate_request(), &mut sink, &CancelToken::default())
        .unwrap();
    let AgentOutput::Candidate(c) = out else {
        panic!("candidate expected")
    };
    assert_eq!(c.draft_markdown, "Tea was nice.");
    assert_eq!(sink.0.last(), Some(&AgentEvent::Phase(Phase::Finished)));
    let runs = r.root.path().join("runtime/agent-runs");
    assert_eq!(std::fs::read_dir(runs).unwrap().count(), 0);
}

#[test]
fn qwen_split_records_are_reassembled() {
    let r = rig();
    r.set("scenario", "split");
    assert!(run(&r, chat_request()).is_ok());
    assert!(run(&r, generate_request()).is_ok());
}

#[test]
fn qwen_unknown_events_are_ignored() {
    let r = rig();
    r.set("scenario", "unknown-events");
    assert!(run(&r, generate_request()).is_ok());
}

#[test]
fn qwen_missing_result_is_invalid_output() {
    let r = rig();
    r.set("scenario", "no-result");
    assert_eq!(code(run(&r, chat_request())), AgentErrorCode::InvalidOutput);
}

#[test]
fn qwen_malformed_structured_output_is_invalid_output() {
    let r = rig();
    for scenario in ["malformed", "unstructured", "invalid-structured"] {
        r.set("scenario", scenario);
        assert_eq!(
            code(run(&r, generate_request())),
            AgentErrorCode::InvalidOutput,
            "{scenario}"
        );
    }
}

#[test]
fn qwen_fenced_output_is_rejected() {
    let r = rig();
    r.set("scenario", "fenced");
    assert_eq!(
        code(run(&r, generate_request())),
        AgentErrorCode::InvalidOutput
    );
}

#[test]
fn qwen_errors_map_to_allowlist() {
    let r = rig();
    for (scenario, want) in [
        ("unauthorized", AgentErrorCode::Unavailable),
        ("endpoint-down", AgentErrorCode::Unavailable),
        ("ratelimit", AgentErrorCode::RateLimited),
        ("context", AgentErrorCode::Context),
        ("otherfail", AgentErrorCode::Failed),
    ] {
        r.set("scenario", scenario);
        assert_eq!(code(run(&r, chat_request())), want, "{scenario}");
    }
}

#[test]
fn qwen_inactivity_becomes_timeout() {
    let r = rig();
    r.set("scenario", "silent-exec");
    assert_eq!(code(run(&r, chat_request())), AgentErrorCode::Timeout);
}

#[test]
fn qwen_late_output_after_cancel_is_dropped() {
    struct Cancelling<'a> {
        run: &'a Run,
        events: Vec<AgentEvent>,
        cancelled_at: Option<usize>,
    }
    impl EventSink for Cancelling<'_> {
        fn event(&mut self, e: AgentEvent) {
            self.events.push(e.clone());
            if e == AgentEvent::Phase(Phase::WaitingForOutput) {
                self.run.request_cancel();
                self.cancelled_at = Some(self.events.len());
            }
        }
    }
    let r = rig();
    r.set("scenario", "late");
    let owner = Run::new();
    let mut sink = Cancelling {
        run: &owner,
        events: vec![],
        cancelled_at: None,
    };
    let result = r.adapter.run(chat_request(), &mut sink, &owner.token());
    assert_eq!(code(result), AgentErrorCode::Cancelled);
    assert_eq!(sink.events.len(), sink.cancelled_at.unwrap());
}

#[test]
fn qwen_cancel_terminates_group() {
    let r = rig();
    r.set("scenario", "silent-exec");
    let owner = Run::new();
    let result = std::thread::scope(|s| {
        s.spawn(|| {
            std::thread::sleep(Duration::from_millis(500));
            owner.request_cancel();
        });
        r.adapter
            .run(chat_request(), &mut Sink::default(), &owner.token())
    });
    assert_eq!(code(result), AgentErrorCode::Cancelled);
    assert_eq!(
        std::fs::read_dir(r.root.path().join("runtime/agent-runs"))
            .unwrap()
            .count(),
        0
    );
}

#[test]
fn qwen_logs_carry_no_content() {
    let r = rig();
    run(&r, generate_request()).unwrap();
    r.set("scenario", "unauthorized");
    let _ = run(&r, generate_request());
    let logs = r.logs.0.lock().unwrap();
    assert_eq!(logs.len(), 2);
    assert!(logs.iter().all(|l| l.code == EventCode::AgentRun));
    let dump = format!("{logs:?}");
    for needle in [SECRET, "Tea was nice", "logged", "home", "runtime"] {
        assert!(!dump.contains(needle), "{needle}");
    }
}

#[test]
fn qwen_unknown_or_foreign_profile_cannot_run() {
    let r = rig();
    let req = chat_request();
    let wrong_kind = AgentRequest::new(
        ProfileHandle {
            alias: "main".into(),
            kind: AgentKind::Codex,
        },
        req.locale().clone(),
        req.mode().clone(),
    );
    assert_eq!(code(run(&r, wrong_kind)), AgentErrorCode::Unavailable);
    let unknown = AgentRequest::new(
        ProfileHandle {
            alias: "nope".into(),
            kind: AgentKind::Qwen,
        },
        req.locale().clone(),
        req.mode().clone(),
    );
    assert_eq!(code(run(&r, unknown)), AgentErrorCode::Unavailable);
}

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/agent/qwen-0.26.4/"
        )
        .to_owned()
            + name,
    )
    .unwrap()
}

#[test]
fn recorded_qwen_stream_parses() {
    use diary_infrastructure::agent::{output::validate_generation, qwen::summarize};
    let ok = summarize(&fixture("success-generation.ndjson"));
    assert!(ok.completed && ok.structured_output && ok.failure.is_none() && !ok.malformed);
    assert_eq!(ok.init, Some(("qwen3.6:35b-a3b".into(), "0.26.4".into())));
    assert_eq!(
        ok.unknown_events, 0,
        "an unknown event kind means the CLI drifted"
    );
    let chat = summarize(&fixture("success-chat-partial.ndjson"));
    assert!(chat.completed && !chat.structured_output && chat.failure.is_none());
    assert_eq!(chat.unknown_events, 0);
    // The recorded structured output passes the validator real runs use.
    let bytes = fixture("success-generation.ndjson");
    let last: serde_json::Value = serde_json::from_slice(
        bytes
            .split(|&b| b == b'\n')
            .rfind(|l| !l.is_empty())
            .unwrap(),
    )
    .unwrap();
    assert!(validate_generation(last["result"].as_str().unwrap().as_bytes(), 1).is_ok());

    let auth = summarize(&fixture("error-no-auth.ndjson"));
    assert_eq!(
        (auth.completed, auth.failure, auth.init),
        (false, Some(AgentErrorCode::Unavailable), None)
    );
    let aborted = summarize(&fixture("error-exit-55-no-result.ndjson"));
    assert!(!aborted.result_seen && !aborted.malformed);
}

#[test]
fn qwen_fixtures_are_redacted() {
    let dir = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/agent/qwen-0.26.4"
    );
    for f in std::fs::read_dir(dir).unwrap().flatten() {
        let t = std::fs::read_to_string(f.path()).unwrap();
        for needle in [
            "/Users/",
            "127.0.0.1",
            "8186",
            "scratchpad",
            "session_id",
            "api_key",
        ] {
            assert!(!t.contains(needle), "{needle} in {:?}", f.file_name());
        }
    }
}

#[test]
fn qwen_exit_55_without_result_is_failed() {
    let r = rig();
    r.set("scenario", "exit-55");
    for req in [chat_request(), generate_request()] {
        assert_eq!(code(run(&r, req)), AgentErrorCode::Failed);
    }
}

#[test]
fn qwen_init_model_mismatch_is_unavailable() {
    let r = rig();
    r.set("scenario", "model-mismatch");
    assert_eq!(code(run(&r, chat_request())), AgentErrorCode::Unavailable);
}

#[test]
fn qwen_init_version_drift_is_unavailable() {
    let r = rig();
    r.set("scenario", "version-drift");
    assert_eq!(code(run(&r, chat_request())), AgentErrorCode::Unavailable);
}

#[test]
fn lease_end_cancels_qwen_runs() {
    let r = rig();
    r.set("scenario", "silent-exec");
    let result = std::thread::scope(|s| {
        s.spawn(|| {
            std::thread::sleep(Duration::from_millis(500));
            r.host.cancel_all();
        });
        run(&r, chat_request())
    });
    assert_eq!(code(result), AgentErrorCode::Cancelled);
    r.set("scenario", "");
    assert!(run(&r, chat_request()).is_ok());
    r.host.shutdown();
    assert_eq!(code(run(&r, chat_request())), AgentErrorCode::Cancelled);
}

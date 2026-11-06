//! Codex adapter against the `fake_cli` example standing in for `codex` (scenario in `$CODEX_HOME`).
use diary_application::{
    agent::*,
    diagnostics::{Diagnostics, EventCode, Record},
};
use diary_domain::authentic::DiaryDate;
mod support;
use diary_infrastructure::agent::{
    codex::{CodexAdapter, GENERATION_SCHEMA},
    host::Host,
    process::Limits,
    profiles::{Entry, ProfileTable},
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
    adapter: CodexAdapter,
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
        kind: AgentKind::Codex,
        home: Some(home),
        executable: Some(exe.unwrap_or_else(fake)),
    }]));
    let logs = Arc::new(Logs::default());
    let host = support::host(root.path(), logs.clone(), limits());
    let adapter = CodexAdapter::new(table, host.clone());
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
        kind: AgentKind::Codex,
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
fn check_maps_each_status() {
    let r = rig();
    assert_eq!(r.adapter.check(&handle()).status, ProfileStatus::Ready);
    r.set("login", "none");
    assert_eq!(
        r.adapter.check(&handle()).status,
        ProfileStatus::Unauthenticated
    );
    r.set("login", "boom");
    assert_eq!(r.adapter.check(&handle()).status, ProfileStatus::Failed);
    r.set("login", "");
    r.set("version", "codex-cli 0.166.0");
    assert_eq!(
        r.adapter.check(&handle()).status,
        ProfileStatus::UnsupportedVersion
    );
    r.set("version", "garbage");
    assert_eq!(r.adapter.check(&handle()).status, ProfileStatus::Failed);
    assert_eq!(
        rig_with(Some("/nonexistent/codex".into()), true)
            .adapter
            .check(&handle())
            .status,
        ProfileStatus::Missing
    );
    assert_eq!(
        rig_with(None, false).adapter.check(&handle()).status,
        ProfileStatus::Unavailable
    );
    let other = ProfileHandle {
        alias: "nope".into(),
        kind: AgentKind::Codex,
    };
    assert_eq!(r.adapter.check(&other).status, ProfileStatus::Unavailable);
}

#[test]
fn unknown_version_is_unsupported() {
    let r = rig();
    for v in [
        "codex-cli 0.167.1",
        "codex-cli 1.0.0",
        "codex-cli 0.167.0-beta",
    ] {
        r.set("version", v);
        assert_eq!(
            r.adapter.check(&handle()).status,
            ProfileStatus::UnsupportedVersion,
            "{v}"
        );
    }
    // A run refuses the same way instead of guessing at an unverified CLI's flags.
    assert_eq!(code(run(&r, chat_request())), AgentErrorCode::Unavailable);
}

#[test]
fn argv_is_fixed_and_contains_no_user_text() {
    let r = rig();
    run(&r, generate_request()).unwrap();
    let argv = std::fs::read_to_string(r.home().join("argv")).unwrap();
    let args: Vec<&str> = argv.split('\n').collect();
    assert!(!argv.contains(SECRET));
    assert_eq!(
        &args[..11],
        [
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
            "-C"
        ]
    );
    let tail: Vec<&str> = args
        .iter()
        .copied()
        .filter(|a| !a.starts_with('/'))
        .skip(11)
        .collect();
    assert_eq!(
        tail,
        [
            "--output-schema",
            "--output-last-message",
            "-m",
            "gpt-5.6-luna",
            "-c",
            "model_reasoning_effort=\"medium\"",
            "-"
        ]
    );
    let stdin = std::fs::read_to_string(r.home().join("stdin")).unwrap();
    assert!(stdin.contains(SECRET), "the prompt travels on stdin");
}

#[test]
fn chat_run_omits_the_schema_and_returns_a_reply() {
    let r = rig();
    assert_eq!(
        run(&r, chat_request()).unwrap(),
        AgentOutput::Reply("A reply.".into())
    );
    assert!(
        !std::fs::read_to_string(r.home().join("argv"))
            .unwrap()
            .contains("--output-schema")
    );
}

#[test]
fn generation_run_returns_a_validated_candidate() {
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
    // No run directory survives the run.
    let runs = r.root.path().join("runtime/agent-runs");
    assert_eq!(std::fs::read_dir(runs).unwrap().count(), 0);
}

#[test]
fn split_ndjson_records_are_reassembled() {
    let r = rig();
    r.set("scenario", "split");
    assert!(run(&r, chat_request()).is_ok());
}

#[test]
fn malformed_final_output_is_invalid_output() {
    let r = rig();
    r.set("scenario", "malformed");
    assert_eq!(
        code(run(&r, generate_request())),
        AgentErrorCode::InvalidOutput
    );
}

#[test]
fn fenced_output_is_rejected() {
    let r = rig();
    r.set("scenario", "fenced");
    assert_eq!(
        code(run(&r, generate_request())),
        AgentErrorCode::InvalidOutput
    );
}

#[test]
fn codex_errors_map_to_allowlist() {
    let r = rig();
    for (scenario, want) in [
        ("unauthorized", AgentErrorCode::Unavailable),
        ("ratelimit", AgentErrorCode::RateLimited),
        ("context", AgentErrorCode::Context),
        ("otherfail", AgentErrorCode::Failed),
    ] {
        r.set("scenario", scenario);
        assert_eq!(code(run(&r, chat_request())), want, "{scenario}");
    }
}

#[test]
fn inactivity_becomes_timeout() {
    let r = rig();
    r.set("scenario", "silent-exec");
    assert_eq!(code(run(&r, chat_request())), AgentErrorCode::Timeout);
}

#[test]
fn late_output_after_cancel_is_dropped() {
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
    // The fake emits a completion record and a last message on TERM; none may surface.
    assert_eq!(sink.events.len(), sink.cancelled_at.unwrap());
}

#[test]
fn logs_carry_no_content() {
    let r = rig();
    run(&r, generate_request()).unwrap();
    r.set("scenario", "unauthorized");
    let _ = run(&r, generate_request());
    let logs = r.logs.0.lock().unwrap();
    assert_eq!(logs.len(), 2);
    assert!(logs.iter().all(|l| l.code == EventCode::AgentRun));
    let dump = format!("{logs:?}");
    for needle in [SECRET, "Tea was nice", "401", "home", "runtime"] {
        assert!(!dump.contains(needle), "{needle}");
    }
}

#[test]
fn unknown_or_foreign_profile_cannot_run() {
    let r = rig();
    let mut req = chat_request();
    req = AgentRequest::new(
        ProfileHandle {
            alias: "nope".into(),
            kind: AgentKind::Codex,
        },
        req.locale().clone(),
        req.mode().clone(),
    );
    assert_eq!(code(run(&r, req)), AgentErrorCode::Unavailable);
}

#[test]
fn schema_matches_the_validator_fields() {
    let v: serde_json::Value = serde_json::from_str(GENERATION_SCHEMA).unwrap();
    let keys = |o: &serde_json::Value| {
        let mut k: Vec<_> = o["properties"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        k.sort();
        k
    };
    assert_eq!(keys(&v), ["claim_support", "draft_markdown", "warnings"]);
    assert_eq!(v["required"].as_array().unwrap().len(), 3);
    let item = &v["properties"]["claim_support"]["items"];
    assert_eq!(keys(item), ["claim", "snippet_ids"]);
    assert_eq!(item["additionalProperties"], false);
    assert_eq!(v["additionalProperties"], false);
}

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/agent/codex-0.167.0/"
        )
        .to_owned()
            + name,
    )
    .unwrap()
}

#[test]
fn recorded_0_167_0_stream_parses() {
    use diary_infrastructure::agent::{codex::summarize, output::validate_generation};
    let ok = summarize(&fixture("success-generation.ndjson"));
    assert!(ok.turn_completed && ok.failure.is_none() && !ok.malformed);
    assert_eq!(
        ok.unknown_events, 0,
        "an unknown event kind means the CLI drifted"
    );
    // The recorded last message passes the same validator real runs use.
    assert!(validate_generation(&fixture("last-message-generation.json"), 1).is_ok());

    let auth = summarize(&fixture("error-unauthorized.ndjson"));
    assert_eq!(
        (auth.turn_completed, auth.failure),
        (false, Some(AgentErrorCode::Unavailable))
    );
    let model = summarize(&fixture("error-model-rejected.ndjson"));
    assert_eq!(model.failure, Some(AgentErrorCode::Failed));
    assert!(!model.malformed);
}

#[test]
fn shutdown_stops_a_running_agent_and_refuses_new_ones() {
    let r = rig();
    r.set("scenario", "silent-exec");
    let result = std::thread::scope(|s| {
        s.spawn(|| {
            std::thread::sleep(Duration::from_millis(500));
            r.host.shutdown();
        });
        run(&r, chat_request())
    });
    assert_eq!(code(result), AgentErrorCode::Cancelled);
    assert_eq!(code(run(&r, chat_request())), AgentErrorCode::Cancelled);
    assert_eq!(
        std::fs::read_dir(r.root.path().join("runtime/agent-runs"))
            .unwrap()
            .count(),
        0
    );
}

#[test]
fn lease_end_cancels_in_flight_work_but_not_later_runs() {
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
    // A new run after the lease ended is not poisoned by the old one.
    r.set("scenario", "");
    assert!(run(&r, chat_request()).is_ok());
}

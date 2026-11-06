//! Apple adapter against a stand-in helper (a shell script whose behavior is a scenario file), the
//! real extraction and hash check, and the closed error mapping. The real helper runs only in
//! `apple_live.rs`.
mod support;
use diary_application::{
    agent::*,
    diagnostics::{Diagnostics, EventCode, Record},
};
use diary_domain::authentic::DiaryDate;
use diary_infrastructure::agent::{
    apple::{AppleAdapter, map_native, parse_availability},
    process::Limits,
    profiles::{Entry, ProfileTable},
};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

const SECRET: &str = "SECRET-DIARY-TEXT";
const GOOD: &str = r#"{\"draft_markdown\":\"Tea was nice.\",\"claim_support\":[{\"claim\":\"Had tea\",\"snippet_ids\":[\"snippet-1\"]}],\"warnings\":[]}"#;

#[derive(Default)]
struct Logs(Mutex<Vec<Record>>);
impl Diagnostics for Logs {
    fn emit(&self, r: Record) {
        self.0.lock().unwrap().push(r);
    }
}

/// A helper whose answers depend on the contents of `<root>/scenario`; argv and stdin are logged.
fn script(root: &Path) -> Vec<u8> {
    let r = root.display();
    format!(
        r#"#!/bin/sh
SC=$(cat "{r}/scenario" 2>/dev/null)
echo "$@" >> "{r}/argv.log"
case "$1" in
availability)
  case "$SC" in
    disabled) echo '{{"state":"unavailable","reason":"apple_intelligence_disabled","os":"x"}}';;
    ineligible) echo '{{"state":"unavailable","reason":"device_ineligible","os":"x"}}';;
    notready) echo '{{"state":"unavailable","reason":"model_not_ready","os":"x"}}';;
    unknown) echo '{{"state":"unavailable","reason":"model_unavailable_unknown","os":"x"}}';;
    novel) echo '{{"state":"unavailable","reason":"something_new","os":"x"}}';;
    garbage) echo 'not json';;
    hang) sleep 30;;
    *) echo '{{"state":"available","os":"Version 26.0","context":4096,"languages":["en","ko"]}}';;
  esac;;
run)
  cat > "{r}/stdin.log"
  case "$SC" in
    context) echo '{{"type":"error","code":"context"}}';;
    unsupported_language|locale_unsupported|refusal|rate_limited|invalid_output|unavailable|busy|timeout) echo "{{\"type\":\"error\",\"code\":\"$SC\"}}";;
    weird) echo '{{"type":"error","code":"brand_new_native_thing"}}';;
    malformed) echo '{{"type":"result","text":"{{}}"}}';;
    runjunk) echo 'not json at all';;
    empty) echo '{{"type":"result","text":"  "}}';;
    noresult) :;;
    exit1) exit 1;;
    slow) sleep 30 & sleep 30;;
    chatreset) echo '{{"type":"delta","text":"Draft one"}}'; echo '{{"type":"reset","text":"Better start"}}'; echo '{{"type":"delta","text":" and more."}}'; echo '{{"type":"result","text":"Better start and more."}}';;
    chat) echo '{{"type":"delta","text":"A "}}'; echo '{{"type":"delta","text":"reply."}}'; echo '{{"type":"result","text":"A reply."}}';;
    *) if grep -q '"mode":"chat"' "{r}/stdin.log"; then echo '{{"type":"result","text":"A reply."}}'; else echo '{{"type":"result","text":"{GOOD}"}}'; fi;;
  esac;;
esac
"#
    )
    .into_bytes()
}

struct Rig {
    root: tempfile::TempDir,
    adapter: AppleAdapter,
    host: Arc<diary_infrastructure::agent::host::Host>,
    logs: Arc<Logs>,
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
fn rig_with(helper: Option<Vec<u8>>, os: u32) -> Rig {
    let root = tempfile::tempdir().unwrap();
    let root_path = root.path().canonicalize().unwrap();
    std::fs::create_dir(root_path.join("runtime")).unwrap();
    let table = Arc::new(ProfileTable::from_entries(vec![Entry {
        alias: "apple".into(),
        kind: AgentKind::AppleFoundationModels,
        home: None,
        executable: None,
    }]));
    let logs = Arc::new(Logs::default());
    let host = support::host(&root_path, logs.clone(), limits());
    let bytes = helper.unwrap_or_else(|| script(&root_path));
    let adapter = AppleAdapter::with_helper(table, host.clone(), bytes, os);
    Rig {
        root,
        adapter,
        host,
        logs,
    }
}
fn rig() -> Rig {
    rig_with(None, 26)
}
impl Rig {
    fn path(&self) -> PathBuf {
        self.root.path().canonicalize().unwrap()
    }
    fn set(&self, scenario: &str) {
        std::fs::write(self.path().join("scenario"), scenario).unwrap();
    }
    fn read(&self, f: &str) -> String {
        std::fs::read_to_string(self.path().join(f)).unwrap_or_default()
    }
    fn check(&self) -> ProfileCheck {
        self.adapter.check(&handle())
    }
    fn run(&self, req: AgentRequest) -> Result<AgentOutput, AgentError> {
        self.adapter
            .run(req, &mut Sink::default(), &CancelToken::default())
    }
    fn reasons(&self) -> Vec<Option<Reason>> {
        self.logs
            .0
            .lock()
            .unwrap()
            .iter()
            .filter_map(|r| r.agent.as_ref().map(|a| a.reason))
            .collect()
    }
    fn leftovers(&self) -> usize {
        std::fs::read_dir(self.path().join("runtime/agent-runs"))
            .map(|d| d.count())
            .unwrap_or(0)
    }
}
#[derive(Default)]
struct Sink(Vec<AgentEvent>);
impl EventSink for Sink {
    fn event(&mut self, e: AgentEvent) {
        self.0.push(e);
    }
}
fn handle() -> ProfileHandle {
    ProfileHandle {
        alias: "apple".into(),
        kind: AgentKind::AppleFoundationModels,
    }
}
fn chat() -> AgentRequest {
    AgentRequest::new(
        handle(),
        Locale::parse("ko-KR").unwrap(),
        Mode::Chat(ChatPacket::new(None, vec![], SECRET.into()).unwrap()),
    )
}
fn generate() -> AgentRequest {
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
fn code(r: Result<AgentOutput, AgentError>) -> AgentErrorCode {
    r.expect_err("expected an error").0
}

#[test]
fn apple_missing_helper_is_unavailable() {
    let r = rig_with(Some(Vec::new()), 26);
    let c = r.check();
    assert_eq!(
        (c.status, c.reason),
        (ProfileStatus::Unavailable, Some(Reason::HelperMissing))
    );
    assert_eq!(code(r.run(chat())), AgentErrorCode::Unavailable);
}

#[test]
fn apple_tampered_helper_is_untrusted() {
    let r = rig();
    assert_eq!(r.check().status, ProfileStatus::Ready);
    let dir = r.path().join("runtime/native");
    let file = std::fs::read_dir(&dir)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    // 0700, owner only.
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
        std::fs::metadata(&file).unwrap().permissions().mode() & 0o777,
        0o700
    );
    let mut bytes = std::fs::read(&file).unwrap();
    bytes.extend_from_slice(b"\n# tampered\n");
    std::fs::write(&file, bytes).unwrap();
    let c = r.check();
    assert_eq!(c.reason, Some(Reason::HelperUntrusted));
    assert_eq!(code(r.run(chat())), AgentErrorCode::Unavailable);
    assert!(
        r.logs
            .0
            .lock()
            .unwrap()
            .iter()
            .any(|l| l.code == EventCode::AgentHelper)
    );
}

#[test]
fn apple_unsupported_os_is_unavailable_without_launching() {
    let r = rig_with(None, 15);
    assert_eq!(r.check().reason, Some(Reason::OsUnsupported));
    assert!(!r.path().join("argv.log").exists(), "helper never launched");
}

#[test]
fn apple_check_maps_each_reason() {
    let r = rig();
    assert_eq!(r.check(), ProfileStatus::Ready.into());
    for (scenario, status, reason) in [
        (
            "disabled",
            ProfileStatus::Unavailable,
            Reason::AppleIntelligenceDisabled,
        ),
        (
            "ineligible",
            ProfileStatus::Unavailable,
            Reason::DeviceIneligible,
        ),
        (
            "notready",
            ProfileStatus::Unavailable,
            Reason::ModelNotReady,
        ),
        (
            "unknown",
            ProfileStatus::Unavailable,
            Reason::ModelUnavailableUnknown,
        ),
        (
            "novel",
            ProfileStatus::Unavailable,
            Reason::ModelUnavailableUnknown,
        ),
        ("garbage", ProfileStatus::Failed, Reason::HelperFailed),
        ("hang", ProfileStatus::Failed, Reason::ProbeTimedOut),
    ] {
        r.set(scenario);
        let c = r.check();
        assert_eq!((c.status, c.reason), (status, Some(reason)), "{scenario}");
    }
}

#[test]
fn apple_availability_parses_metadata() {
    let a = parse_availability(
        "{\"state\":\"available\",\"os\":\"Version 26.7\",\"context\":4096,\"languages\":[\"en\",\"ko\"]}",
    )
    .unwrap();
    assert!(a.available);
    assert_eq!((a.context, a.languages.len()), (Some(4096), 2));
    assert_eq!(parse_availability("x").unwrap_err(), Reason::HelperFailed);
}

#[test]
fn apple_argv_is_fixed_and_contains_no_user_text() {
    let r = rig();
    r.run(generate()).unwrap();
    r.run(chat()).unwrap();
    let argv = r.read("argv.log");
    for line in argv.lines() {
        assert!(line == "availability" || line == "run", "{line}");
    }
    assert!(!argv.contains(SECRET));
    // Stdin is the only input and carries instructions, prompt, locale and mode.
    let stdin = r.read("stdin.log");
    let v: serde_json::Value = serde_json::from_str(&stdin).unwrap();
    assert_eq!(v["mode"], "chat");
    assert_eq!(v["locale"], "ko-KR");
    assert!(v["instructions"].as_str().unwrap().contains("ko-KR"));
    assert!(v["prompt"].as_str().unwrap().contains(SECRET));
    assert!(v.get("model").is_none() && v.get("effort").is_none());
}

#[test]
fn apple_generation_output_is_validated() {
    let r = rig();
    let out = r.run(generate()).unwrap();
    let AgentOutput::Candidate(c) = out else {
        panic!("expected a candidate")
    };
    assert_eq!(c.draft_markdown, "Tea was nice.");
    for scenario in ["malformed", "empty", "runjunk", "noresult"] {
        r.set(scenario);
        assert_eq!(
            code(r.run(generate())),
            AgentErrorCode::InvalidOutput,
            "{scenario}"
        );
    }
    r.set("exit1");
    assert_eq!(code(r.run(generate())), AgentErrorCode::Failed);
}

#[test]
fn apple_chat_reply_streams_and_is_bounded() {
    let r = rig();
    r.set("chat");
    let mut sink = Sink::default();
    let out = r
        .adapter
        .run(chat(), &mut sink, &CancelToken::default())
        .unwrap();
    assert!(matches!(out, AgentOutput::Reply(t) if t == "A reply."));
    let deltas: Vec<_> = sink
        .0
        .iter()
        .filter_map(|e| match e {
            AgentEvent::Delta(t) => Some(t.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(deltas, ["A ", "reply."]);
    r.set("empty");
    assert_eq!(code(r.run(chat())), AgentErrorCode::InvalidOutput);
}

#[test]
fn apple_errors_map_to_allowlist() {
    for (native, want) in [
        ("context", AgentErrorCode::Context),
        ("unsupported_language", AgentErrorCode::UnsupportedLanguage),
        ("locale_unsupported", AgentErrorCode::UnsupportedLanguage),
        ("refusal", AgentErrorCode::Refusal),
        ("rate_limited", AgentErrorCode::RateLimited),
        ("invalid_output", AgentErrorCode::InvalidOutput),
        ("unavailable", AgentErrorCode::Unavailable),
        ("busy", AgentErrorCode::Busy),
        ("timeout", AgentErrorCode::Timeout),
        ("brand_new_native_thing", AgentErrorCode::Failed),
    ] {
        assert_eq!(map_native(native).0, want, "{native}");
    }
    let r = rig();
    for scenario in [
        "context",
        "unsupported_language",
        "locale_unsupported",
        "refusal",
        "rate_limited",
        "invalid_output",
        "busy",
        "timeout",
    ] {
        r.set(scenario);
        assert_eq!(code(r.run(chat())), map_native(scenario).0, "{scenario}");
    }
}

#[test]
fn apple_availability_loss_mid_run_is_unavailable() {
    let r = rig();
    r.set("unavailable");
    assert_eq!(code(r.run(chat())), AgentErrorCode::Unavailable);
    assert_eq!(r.reasons().last(), Some(&Some(Reason::ModelNotReady)));
}

#[test]
fn apple_unknown_native_error_is_failed_and_logged() {
    let r = rig();
    r.set("weird");
    assert_eq!(code(r.run(chat())), AgentErrorCode::Failed);
    assert_eq!(r.reasons().last(), Some(&Some(Reason::NativeErrorUnmapped)));
    let dump = format!("{:?}", r.logs.0.lock().unwrap());
    assert!(
        !dump.contains("brand_new_native_thing"),
        "native text is never logged"
    );
}

#[test]
fn apple_availability_is_rechecked_before_every_run() {
    let r = rig();
    r.run(chat()).unwrap();
    r.set("disabled");
    assert_eq!(code(r.run(chat())), AgentErrorCode::Unavailable);
    assert_eq!(
        r.reasons().last(),
        Some(&Some(Reason::AppleIntelligenceDisabled))
    );
}

#[test]
fn apple_cancel_terminates_group() {
    let r = rig();
    r.set("slow");
    let cancel = CancelToken::default();
    let run = Run::from(cancel.clone());
    let res = std::thread::scope(|s| {
        s.spawn(|| {
            std::thread::sleep(Duration::from_millis(600));
            run.request_cancel();
        });
        r.adapter.run(chat(), &mut Sink::default(), &cancel)
    });
    assert_eq!(code(res), AgentErrorCode::Cancelled);
    assert_eq!(r.leftovers(), 0);
    r.host.shutdown();
    assert_eq!(code(r.run(chat())), AgentErrorCode::Cancelled);
}

#[test]
fn apple_helper_source_never_uses_cloud_or_tools_and_mirrors_the_schema() {
    let src = include_str!("../native/apple-fm/main.swift");
    assert!(!src.contains("PrivateCloudCompute"));
    assert!(!src.contains("tools:"));
    assert!(!src.contains("URLSession"));
    // The guided struct has exactly the schema-1 top-level fields.
    let schema: serde_json::Value =
        serde_json::from_str(diary_infrastructure::agent::codex::GENERATION_SCHEMA).unwrap();
    let mut want: Vec<String> = schema["required"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect();
    want.sort();
    let body = src
        .split("@Generable struct Candidate {")
        .nth(1)
        .unwrap()
        .split('}')
        .next()
        .unwrap();
    let mut have: Vec<String> = body
        .lines()
        .filter_map(|l| l.trim().strip_prefix("var "))
        .map(|l| l.split(':').next().unwrap().to_owned())
        .collect();
    have.sort();
    assert_eq!(have, want);
    let claim = src
        .split("@Generable struct Claim {")
        .nth(1)
        .unwrap()
        .split('}')
        .next()
        .unwrap();
    for f in ["claim", "snippet_ids"] {
        assert!(claim.contains(&format!("var {f}:")), "{f}");
    }
}

fn fixture(name: &str) -> Vec<u8> {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/agent/apple-macos26.7.1-sdk27.2")
        .join(name);
    std::fs::read(p).unwrap()
}

#[test]
fn recorded_macos_26_7_1_streams_parse() {
    use diary_infrastructure::agent::{apple::summarize, output::validate_generation};
    let a = parse_availability(&String::from_utf8(fixture("availability-available.json")).unwrap())
        .unwrap();
    assert!(a.available);
    assert_eq!(a.context, Some(4096));
    assert!(a.languages.iter().any(|l| l == "ko") && a.languages.iter().any(|l| l == "en"));

    let chat = summarize(&fixture("chat-reply.ndjson"));
    assert!(!chat.malformed && chat.failure.is_none());
    assert_eq!((chat.deltas, chat.result.is_some()), (1, true));

    let gen_ = summarize(&fixture("generate-candidate.ndjson"));
    let text = gen_.result.expect("a result line");
    assert!(validate_generation(text.as_bytes(), 1).is_ok());

    let ctx = summarize(&fixture("error-context.ndjson"));
    assert_eq!(ctx.failure, Some((AgentErrorCode::Context, None)));
    let loc = summarize(&fixture("error-locale.ndjson"));
    assert_eq!(
        loc.failure,
        Some((
            AgentErrorCode::UnsupportedLanguage,
            Some(Reason::LocaleUnsupported)
        ))
    );
    assert!(ctx.result.is_none() && loc.result.is_none());
}

#[test]
fn apple_inactivity_becomes_timeout_and_lease_end_cancels_without_poisoning() {
    let r = rig();
    r.set("slow");
    assert_eq!(code(r.run(chat())), AgentErrorCode::Timeout);
    assert_eq!(r.leftovers(), 0);
    let res = std::thread::scope(|s| {
        s.spawn(|| {
            std::thread::sleep(Duration::from_millis(500));
            r.host.cancel_all();
        });
        r.run(chat())
    });
    assert_eq!(code(res), AgentErrorCode::Cancelled);
    assert_eq!(r.reasons().last(), Some(&Some(Reason::LeaseEnded)));
    r.set("");
    assert!(
        r.run(chat()).is_ok(),
        "a later run is not poisoned by the lease end"
    );
    assert_eq!(r.leftovers(), 0);
}

#[test]
fn apple_helper_deleted_after_extraction_is_extracted_again() {
    let r = rig();
    assert_eq!(r.check().status, ProfileStatus::Ready);
    let dir = r.path().join("runtime/native");
    let file = std::fs::read_dir(&dir)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    std::fs::remove_file(&file).unwrap();
    assert_eq!(r.check().status, ProfileStatus::Ready);
    assert!(file.exists());
}

#[test]
fn apple_loose_native_directory_is_tightened_to_0700() {
    use std::os::unix::fs::PermissionsExt;
    let r = rig();
    let dir = r.path().join("runtime/native");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o777)).unwrap();
    assert_eq!(r.check().status, ProfileStatus::Ready);
    assert_eq!(
        std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
        0o700
    );
    // Loosened again later: the next launch tightens it again.
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o775)).unwrap();
    assert_eq!(r.check().status, ProfileStatus::Ready);
    assert_eq!(
        std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
        0o700
    );
}

#[test]
fn apple_revised_stream_text_is_sent_as_a_replace_event() {
    let r = rig();
    r.set("chatreset");
    let mut sink = Sink::default();
    let out = r
        .adapter
        .run(chat(), &mut sink, &CancelToken::default())
        .unwrap();
    assert!(matches!(out, AgentOutput::Reply(t) if t == "Better start and more."));
    let text: Vec<_> = sink
        .0
        .iter()
        .filter_map(|e| match e {
            AgentEvent::Delta(t) => Some(format!("+{t}")),
            AgentEvent::Replace(t) => Some(format!("={t}")),
            _ => None,
        })
        .collect();
    assert_eq!(text, ["+Draft one", "=Better start", "+ and more."]);
    // A consumer applying the events ends with the final reply.
    let mut shown = String::new();
    for e in &sink.0 {
        match e {
            AgentEvent::Delta(t) => shown.push_str(t),
            AgentEvent::Replace(t) => shown = t.clone(),
            _ => {}
        }
    }
    assert_eq!(shown, "Better start and more.");
}

//! Live Qwen Code proof (P1-M19 C09). Ignored by default; turns use the profile's own (local) endpoint. Run by the agent:
//!
//! `DIARY_QWEN_LIVE=1 DIARY_QWEN_LIVE_HOME=<profile_home> cargo test -p diary-infrastructure \
//!    --test qwen_live -- --ignored --nocapture --test-threads=1`
//!
//! Three tests, one real turn each: schema-1 generation (`live_check_and_generation`), a chat reply
//! (`live_chat`) and a cancelled run (`live_cancel`); select with a test-name filter. The profile home
//! is only passed as `QWEN_HOME`; no settings or login file is read or copied. The adapter fails a run whose
//! `system.init` reports another model, so a silently replaced pin is reported, never accepted.
use diary_application::{agent::*, diagnostics::NullDiagnostics};
use diary_domain::authentic::DiaryDate;
use diary_infrastructure::agent::{
    codex::CodexAdapter,
    profiles::{Entry, ProfileTable},
    qwen::QwenAdapter,
};
use std::{path::PathBuf, sync::Arc, time::Instant};

struct Sink(Vec<AgentEvent>);
impl EventSink for Sink {
    fn event(&mut self, e: AgentEvent) {
        self.0.push(e);
    }
}

fn adapter() -> Option<(QwenAdapter, tempfile::TempDir)> {
    if std::env::var_os("DIARY_QWEN_LIVE").is_none_or(|v| v != "1") {
        eprintln!("DIARY_QWEN_LIVE=1 not set; skipping");
        return None;
    }
    let home =
        PathBuf::from(std::env::var_os("DIARY_QWEN_LIVE_HOME").expect("DIARY_QWEN_LIVE_HOME"));
    let root = tempfile::tempdir().unwrap();
    let table = Arc::new(ProfileTable::from_entries(vec![Entry {
        alias: "live".into(),
        kind: AgentKind::Qwen,
        home: Some(home),
        executable: None,
    }]));
    let codex = CodexAdapter::new(
        table.clone(),
        root.path().to_path_buf(),
        std::env::var_os("PATH").unwrap_or_default(),
        std::env::current_dir().unwrap(),
        Arc::new(NullDiagnostics),
    );
    let adapter = QwenAdapter::new(table, codex.host());
    Some((adapter, root))
}
fn handle() -> ProfileHandle {
    ProfileHandle {
        alias: "live".into(),
        kind: AgentKind::Qwen,
    }
}
fn request(chat: bool) -> AgentRequest {
    let mode = if chat {
        Mode::Chat(
            ChatPacket::new(None, vec![], "In one short sentence, say hello.".into()).unwrap(),
        )
    } else {
        let snippet = SnippetItem {
            id: "snippet-1".into(),
            instant_utc: "2026-11-06T01:00:00Z".into(),
            civil_date: "2026-11-06".into(),
            offset: "+00:00".into(),
            body: "I drank a cup of tea in the morning.".into(),
        };
        Mode::Candidate {
            index: 0,
            count: 1,
            packet: GenerationPacket::new(
                Style::Concise,
                DiaryDate::from_ymd(2026, 11, 6).unwrap(),
                vec![snippet],
            )
            .unwrap(),
        }
    };
    AgentRequest::new(handle(), Locale::parse("en").unwrap(), mode)
}
fn cli_version() -> String {
    let o = std::process::Command::new("qwen")
        .arg("--version")
        .output()
        .unwrap();
    String::from_utf8_lossy(&o.stdout).trim().to_owned()
}
fn leftovers(root: &std::path::Path) -> usize {
    std::fs::read_dir(root.join("agent-runs")).map_or(0, |d| d.count())
}

#[test]
#[ignore = "uses the local Qwen endpoint; set DIARY_QWEN_LIVE=1"]
fn live_check_and_generation() {
    let Some((adapter, _root)) = adapter() else {
        return;
    };
    assert_eq!(
        adapter.check(&handle()),
        ProfileStatus::Ready,
        "profile must be ready"
    );
    let before = cli_version();
    let t = Instant::now();
    let mut sink = Sink(vec![]);
    match adapter.run(request(false), &mut sink, &CancelToken::default()) {
        Ok(AgentOutput::Candidate(c)) => eprintln!(
            "generation ok in {:?}: {} bytes, {} claims",
            t.elapsed(),
            c.draft_markdown.len(),
            c.claim_support.len()
        ),
        other => {
            panic!("generation failed (model pin or flags rejected? report to owner): {other:?}")
        }
    }
    // The CLI may self-update; a changed version means the fixtures no longer describe it.
    assert_eq!(cli_version(), before, "qwen changed version during the run");
    eprintln!("qwen version {before}");
}

#[test]
#[ignore = "uses the local Qwen endpoint; set DIARY_QWEN_LIVE=1"]
fn live_chat() {
    let Some((adapter, _root)) = adapter() else {
        return;
    };
    let t = Instant::now();
    match adapter.run(request(true), &mut Sink(vec![]), &CancelToken::default()) {
        Ok(AgentOutput::Reply(r)) => eprintln!("chat ok in {:?}: {} bytes", t.elapsed(), r.len()),
        other => panic!("chat failed: {other:?}"),
    }
}

/// Cancel as soon as the agent reports it is waiting; no run directory or process may remain.
#[test]
#[ignore = "uses the local Qwen endpoint; set DIARY_QWEN_LIVE=1"]
fn live_cancel() {
    let Some((adapter, root)) = adapter() else {
        return;
    };
    let owner = Run::new();
    struct Cancel<'a>(&'a Run);
    impl EventSink for Cancel<'_> {
        fn event(&mut self, e: AgentEvent) {
            if e == AgentEvent::Phase(Phase::WaitingForOutput) {
                self.0.request_cancel();
            }
        }
    }
    let t = Instant::now();
    let r = adapter.run(request(true), &mut Cancel(&owner), &owner.token());
    eprintln!("cancel result {r:?} after {:?}", t.elapsed());
    assert_eq!(r, Err(AgentErrorCode::Cancelled.into()));
    assert_eq!(leftovers(root.path()), 0);
}

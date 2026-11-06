//! Live proof against the real `apple-fm` helper embedded in this build and Apple's on-device model
//! (P1-M20 C18). Ignored by default; run by the agent, never a human gate:
//!
//! `DIARY_APPLE_LIVE=1 cargo test -p diary-infrastructure --test apple_live -- --ignored \
//!    --nocapture --test-threads=1`
//!
//! Four tests: availability (`live_availability`), schema-1 generation (`live_generation`), a chat
//! reply (`live_chat`) and a mid-run cancel (`live_cancel`). Each prints OS, timing and model
//! metadata. Nothing here sends diary content; inputs are synthetic.
mod support;
use diary_application::agent::*;
use diary_domain::authentic::DiaryDate;
use diary_infrastructure::agent::{
    apple::AppleAdapter,
    profiles::{Entry, ProfileTable},
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

fn adapter() -> Option<(AppleAdapter, tempfile::TempDir)> {
    if std::env::var_os("DIARY_APPLE_LIVE").is_none_or(|v| v != "1") {
        eprintln!("DIARY_APPLE_LIVE=1 not set; skipping");
        return None;
    }
    let root = tempfile::tempdir().unwrap();
    let table = Arc::new(ProfileTable::from_entries(vec![Entry {
        alias: "live".into(),
        kind: AgentKind::AppleFoundationModels,
        home: None,
        executable: None,
    }]));
    let adapter = AppleAdapter::new(table, support::live_host(root.path()));
    Some((adapter, root))
}
fn handle() -> ProfileHandle {
    ProfileHandle {
        alias: "live".into(),
        kind: AgentKind::AppleFoundationModels,
    }
}
#[derive(Default)]
struct Sink(Vec<AgentEvent>);
impl EventSink for Sink {
    fn event(&mut self, e: AgentEvent) {
        self.0.push(e);
    }
}
fn chat(locale: &str) -> AgentRequest {
    AgentRequest::new(
        handle(),
        Locale::parse(locale).unwrap(),
        Mode::Chat(
            ChatPacket::new(None, vec![], "In one short sentence, say hello.".into()).unwrap(),
        ),
    )
}
fn leftovers(root: &tempfile::TempDir) -> usize {
    std::fs::read_dir(root.path().join("agent-runs"))
        .map(|d| d.count())
        .unwrap_or(0)
}

#[test]
#[ignore = "needs Apple Intelligence; run with DIARY_APPLE_LIVE=1"]
fn live_availability() {
    let Some((a, _root)) = adapter() else { return };
    let t = Instant::now();
    let c = a.check(&handle());
    eprintln!("availability: {c:?} in {:?}", t.elapsed());
    assert_eq!(c.status, ProfileStatus::Ready, "{c:?}");
}

#[test]
#[ignore = "needs Apple Intelligence; run with DIARY_APPLE_LIVE=1"]
fn live_generation() {
    let Some((a, root)) = adapter() else { return };
    let snip = SnippetItem {
        id: "snippet-1".into(),
        instant_utc: "2026-11-06T01:00:00Z".into(),
        civil_date: "2026-11-06".into(),
        offset: "+09:00".into(),
        body: "I drank tea and read a book.".into(),
    };
    let packet = GenerationPacket::new(
        Style::Standard,
        DiaryDate::from_ymd(2026, 11, 6).unwrap(),
        vec![snip],
    )
    .unwrap();
    let req = AgentRequest::new(
        handle(),
        Locale::parse("en").unwrap(),
        Mode::Candidate {
            index: 0,
            count: 1,
            packet,
        },
    );
    let t = Instant::now();
    let out = a.run(req, &mut Sink::default(), &CancelToken::default());
    eprintln!("generation: {:?}", t.elapsed());
    let Ok(AgentOutput::Candidate(c)) = out else {
        panic!("expected a validated candidate, got {out:?}")
    };
    assert!(!c.draft_markdown.is_empty());
    assert_eq!(leftovers(&root), 0);
}

#[test]
#[ignore = "needs Apple Intelligence; run with DIARY_APPLE_LIVE=1"]
fn live_chat() {
    let Some((a, root)) = adapter() else { return };
    for locale in ["en", "ko"] {
        let mut sink = Sink::default();
        let t = Instant::now();
        let out = a.run(chat(locale), &mut sink, &CancelToken::default());
        eprintln!(
            "chat {locale}: {:?} in {:?}",
            out.as_ref().map(|_| "reply"),
            t.elapsed()
        );
        let Ok(AgentOutput::Reply(text)) = out else {
            panic!("expected a reply for {locale}, got {out:?}")
        };
        assert!(!text.trim().is_empty());
        assert!(
            sink.0
                .iter()
                .any(|e| matches!(e, AgentEvent::Phase(Phase::Finished)))
        );
    }
    assert_eq!(leftovers(&root), 0);
}

#[test]
#[ignore = "needs Apple Intelligence; run with DIARY_APPLE_LIVE=1"]
fn live_cancel() {
    let Some((a, root)) = adapter() else { return };
    let cancel = CancelToken::default();
    let run = Run::from(cancel.clone());
    let req = AgentRequest::new(
        handle(),
        Locale::parse("en").unwrap(),
        Mode::Chat(
            ChatPacket::new(
                None,
                vec![],
                "Write a very long story about a dragon, 800 words.".into(),
            )
            .unwrap(),
        ),
    );
    let t = Instant::now();
    let res = std::thread::scope(|s| {
        s.spawn(|| {
            std::thread::sleep(Duration::from_millis(400));
            run.request_cancel();
        });
        a.run(req, &mut Sink::default(), &cancel)
    });
    eprintln!(
        "cancel: {:?} in {:?}",
        res.as_ref().err().map(|e| e.0),
        t.elapsed()
    );
    match res {
        Err(e) => assert_eq!(e.0, AgentErrorCode::Cancelled),
        // A model that answers before the cancel lands (400 ms) legitimately succeeds; after it, a
        // success would mean the commit-once guard failed.
        Ok(_) => assert!(
            t.elapsed() < Duration::from_millis(400),
            "a cancelled run must not succeed"
        ),
    }
    assert_eq!(leftovers(&root), 0);
}

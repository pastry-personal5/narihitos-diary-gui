use super::*;
use crate::agent::{
    AgentErrorCode, AgentKind, AgentRequest, ChatPacket, Locale, Mode, ProfileCheck, ProfileHandle,
    PromptResolver, PromptSelection, ResolvedPrompt,
};
use crate::{Coordinator, Error, JobState, Limits};
use std::cell::Cell;

fn request() -> PreparedAgentRun {
    let request = AgentRequest::new(
        ProfileHandle {
            alias: "p".into(),
            kind: AgentKind::Codex,
        },
        Locale::parse("en-US").unwrap(),
        Mode::Chat(ChatPacket::new(None, vec![], "hi".into()).unwrap()),
    );
    crate::agent::test_prompts::definition(PromptSelection::for_request(&request))
        .prepare(request)
        .unwrap()
}

/// Replays a script of (clock advance, event), then returns the given result.
struct Script {
    events: Vec<(u64, AgentEvent)>,
    clock: &'static Cell<u64>,
    result: Result<AgentOutput, AgentError>,
    cancel_after: Option<usize>,
    run: Option<Run>,
}
impl PromptResolver for Script {
    fn resolve(&self, selection: PromptSelection) -> Result<ResolvedPrompt, AgentError> {
        Ok(crate::agent::test_prompts::definition(selection))
    }
}
impl AgentRunner for Script {
    fn check(&self, _: &ProfileHandle) -> ProfileCheck {
        unreachable!("not used")
    }
    fn run_prepared(
        &self,
        prepared: PreparedAgentRun,
        sink: &mut dyn EventSink,
        _: &crate::agent::CancelToken,
    ) -> Result<AgentOutput, AgentError> {
        let _ = prepared.into_request();
        for (i, (dt, e)) in self.events.iter().enumerate() {
            self.clock.set(self.clock.get() + dt);
            if self.cancel_after == Some(i) {
                self.run.as_ref().unwrap().request_cancel();
            }
            sink.event(e.clone());
        }
        self.result.clone()
    }
}

fn script(events: Vec<(u64, AgentEvent)>) -> Script {
    Script {
        events,
        clock: Box::leak(Box::new(Cell::new(1_000))),
        result: Ok(AgentOutput::Reply("done".into())),
        cancel_after: None,
        run: None,
    }
}
fn go(script: Script, run: &Run) -> (Result<AgentOutput, AgentError>, Snapshot, u32) {
    let shared: Shared = Default::default();
    let mut n = 0u32;
    let clock = script.clock;
    let r = execute(
        &script,
        request(),
        run,
        &shared,
        &|| clock.get(),
        &mut || n += 1,
    );
    let snap = shared.lock().unwrap().clone();
    (r, snap, n)
}
fn d(s: &str) -> AgentEvent {
    AgentEvent::Delta(s.into())
}

#[test]
fn delta_batching_bounds() {
    // 100 deltas, 1 ms apart: flushed about every 50 ms, never once per delta.
    let (r, snap, n) = go(script((0..100).map(|_| (1, d("a"))).collect()), &Run::new());
    assert!(r.is_ok());
    assert_eq!(snap.text.len(), 100);
    assert!((2..=4).contains(&n), "notifications: {n}");

    // A 16 KiB delta flushes at once, even with no time passing.
    let big = "x".repeat(FLUSH_BYTES);
    let (_, snap, n) = go(script(vec![(0, d(&big))]), &Run::new());
    assert_eq!((snap.text.len(), n), (FLUSH_BYTES, 1));
}

#[test]
fn replace_supersedes() {
    let events = vec![
        (0, d("draft one")),
        (100, d(" more")),
        (0, AgentEvent::Replace("revised".into())),
        (100, d("!")),
    ];
    let (_, snap, _) = go(script(events), &Run::new());
    assert_eq!(snap.text, "revised!");
}

#[test]
fn phase_flushes_pending_text_and_is_recorded() {
    let events = vec![
        (0, d("tail")),
        (0, AgentEvent::Phase(Phase::ValidatingOutput)),
    ];
    let (_, snap, _) = go(script(events), &Run::new());
    assert_eq!(
        (snap.text.as_str(), snap.phase),
        ("tail", Some(Phase::ValidatingOutput))
    );
}

#[test]
fn cancel_wins_over_late_success() {
    let run = Run::new();
    let mut s = script(vec![
        (100, d("seen")),
        (100, d("hidden")),
        (100, AgentEvent::Phase(Phase::Finished)),
    ]);
    s.cancel_after = Some(1);
    s.run = Some(Run::from(run.token()));
    let (r, snap, _) = go(s, &run);
    assert_eq!(r, Err(AgentErrorCode::Cancelled.into()));
    assert_eq!(snap.text, "seen");
    assert_eq!(snap.phase, None);
}

const DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const OTHER: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
fn coordinator() -> Coordinator {
    let mut c = Coordinator::new("i".into(), Limits::default());
    c.open("client", 0).unwrap();
    c
}

#[test]
fn second_run_is_busy() {
    let mut c = coordinator();
    c.admit_agent("client", 0, "r1", DIGEST, "j1", 10).unwrap();
    assert_eq!(
        c.admit_agent("client", 0, "r2", OTHER, "j2", 10),
        Err(Error::Busy)
    );
    // An import-style job is not an agent run and is not blocked.
    c.admit("client", 0, "r3", OTHER, "j3", 10).unwrap();
    c.transition("client", 0, "j1", JobState::Running).unwrap();
    assert_eq!(
        c.admit_agent("client", 0, "r4", OTHER, "j4", 10),
        Err(Error::Busy)
    );
    c.transition("client", 0, "j1", JobState::Succeeded)
        .unwrap();
    c.admit_agent("client", 0, "r5", OTHER, "j5", 10).unwrap();
}

#[test]
fn retry_returns_same_job() {
    let mut c = coordinator();
    let a = c.admit_agent("client", 0, "r1", DIGEST, "j1", 10).unwrap();
    // The busy check never hides a lost reply.
    assert_eq!(c.admit_agent("client", 0, "r1", DIGEST, "j9", 10), Ok(a));
}

#[test]
fn changed_payload_is_mismatch() {
    let mut c = coordinator();
    c.admit_agent("client", 0, "r1", DIGEST, "j1", 10).unwrap();
    assert_eq!(
        c.admit_agent("client", 0, "r1", OTHER, "j2", 10),
        Err(Error::RequestMismatch)
    );
}

#[test]
fn touch_bumps_sequence_until_terminal() {
    let mut c = coordinator();
    c.admit_agent("client", 0, "r1", DIGEST, "j1", 10).unwrap();
    assert_eq!(c.touch("client", 0, "j1").unwrap().sequence, 2);
    c.cancel("client", 0, "j1").unwrap();
    assert_eq!(c.touch("client", 0, "j1"), Err(Error::InvalidTransition));
}

#[test]
fn replace_preview_bound_is_utf8_bytes() {
    let (_, snap, _) = go(
        script(vec![(0, AgentEvent::Replace("한".repeat(MAX_TEXT_BYTES)))]),
        &Run::new(),
    );
    assert!(snap.text.len() <= MAX_TEXT_BYTES);
    assert_eq!(snap.text.len() % 3, 0);
}
#[test]
fn pending_preview_is_not_published_after_cancel() {
    let run = Run::new();
    let mut s = script(vec![(0, d("pending")), (0, d("late"))]);
    s.cancel_after = Some(1);
    s.run = Some(Run::from(run.token()));
    let (result, snap, _) = go(s, &run);
    assert_eq!(result, Err(AgentErrorCode::Cancelled.into()));
    assert!(snap.text.is_empty());
}

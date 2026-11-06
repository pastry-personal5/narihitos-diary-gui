//! One lifecycle matrix over every CLI kind against the `fake_cli` example, run through the
//! registry. A new agent needs a fake scenario set and one registry entry, nothing here.
mod support;
use diary_application::agent::*;
use diary_domain::authentic::DiaryDate;
use diary_infrastructure::agent::{
    host::Host,
    process::Limits,
    profiles::{Entry, ProfileTable},
    registry::Agents,
};
use std::{path::PathBuf, sync::Arc, time::Duration};

const SECRET: &str = "SECRET-DIARY-TEXT";
/// Kinds the fake CLI can stand in for. Apple joins in stage C with its own fake helper.
const KINDS: [AgentKind; 3] = [AgentKind::Codex, AgentKind::Claude, AgentKind::Qwen];

fn fake() -> PathBuf {
    std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples/fake_cli")
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

struct Rig {
    kind: AgentKind,
    root: tempfile::TempDir,
    host: Arc<Host>,
    agents: Agents,
}
impl Rig {
    fn new(kind: AgentKind) -> Self {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("home");
        std::fs::create_dir(&home).unwrap();
        std::fs::create_dir(root.path().join("runtime")).unwrap();
        let table = Arc::new(ProfileTable::from_entries(vec![Entry {
            alias: "main".into(),
            kind,
            home: Some(home),
            executable: Some(fake()),
        }]));
        let host = support::host(
            root.path(),
            Arc::new(diary_application::diagnostics::NullDiagnostics),
            limits(),
        );
        let agents = Agents::standard(table, host.clone());
        Self {
            kind,
            root,
            host,
            agents,
        }
    }
    fn set(&self, scenario: &str) {
        std::fs::write(self.root.path().join("home/scenario"), scenario).unwrap();
    }
    fn handle(&self, kind: AgentKind) -> ProfileHandle {
        ProfileHandle {
            alias: "main".into(),
            kind,
        }
    }
    fn chat(&self) -> AgentRequest {
        AgentRequest::new(
            self.handle(self.kind),
            Locale::parse("en").unwrap(),
            Mode::Chat(ChatPacket::new(None, vec![], SECRET.into()).unwrap()),
        )
    }
    fn generate(&self) -> AgentRequest {
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
            self.handle(self.kind),
            Locale::parse("en").unwrap(),
            Mode::Candidate {
                index: 0,
                count: 1,
                packet,
            },
        )
    }
    fn run(&self, req: AgentRequest) -> (Result<AgentOutput, AgentError>, Vec<AgentEvent>) {
        let mut sink = Sink::default();
        let r = self.agents.run(req, &mut sink, &CancelToken::default());
        (r, sink.0)
    }
    fn code(&self, req: AgentRequest) -> AgentErrorCode {
        self.run(req).0.expect_err("expected an error").0
    }
    fn leftover_run_dirs(&self) -> usize {
        std::fs::read_dir(self.root.path().join("runtime/agent-runs"))
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

fn each(f: impl Fn(&Rig)) {
    for k in KINDS {
        f(&Rig::new(k));
    }
}

#[test]
fn registry_dispatches_every_kind() {
    each(|r| {
        assert!(r.agents.registered(r.kind));
        assert_eq!(
            r.agents.check(&r.handle(r.kind)).status,
            ProfileStatus::Ready
        );
        assert!(r.run(r.chat()).0.is_ok(), "{:?}", r.kind);
    });
}

#[test]
fn phases_arrive_in_order() {
    let rank = |p: Phase| match p {
        Phase::PreparingContext => 0,
        Phase::StartingAgent => 1,
        Phase::WaitingForOutput => 2,
        Phase::ReceivingOutput => 3,
        Phase::ValidatingOutput => 4,
        Phase::BetweenCandidates | Phase::Stopping => 5,
        Phase::Finished => 6,
    };
    each(|r| {
        for req in [r.chat(), r.generate()] {
            let (out, events) = r.run(req);
            assert!(out.is_ok(), "{:?}", r.kind);
            let phases: Vec<u8> = events
                .iter()
                .filter_map(|e| match e {
                    AgentEvent::Phase(p) => Some(rank(*p)),
                    _ => None,
                })
                .collect();
            assert_eq!(phases.first(), Some(&1), "{:?}", r.kind);
            assert_eq!(phases.last(), Some(&6), "{:?}", r.kind);
            assert!(
                phases.windows(2).all(|w| w[0] <= w[1]),
                "{:?} {phases:?}",
                r.kind
            );
            assert!(phases.contains(&4), "{:?}", r.kind);
        }
    });
}

#[test]
fn unknown_alias_and_wrong_kind_are_unavailable() {
    each(|r| {
        let mut req = r.chat();
        req = AgentRequest::new(
            ProfileHandle {
                alias: "nope".into(),
                kind: r.kind,
            },
            Locale::parse("en").unwrap(),
            req.mode().clone(),
        );
        assert_eq!(r.code(req), AgentErrorCode::Unavailable, "{:?}", r.kind);
        let other = KINDS.into_iter().find(|k| *k != r.kind).unwrap();
        let req = AgentRequest::new(
            r.handle(other),
            Locale::parse("en").unwrap(),
            r.chat().mode().clone(),
        );
        assert_eq!(r.code(req), AgentErrorCode::Unavailable, "{:?}", r.kind);
        assert_eq!(
            r.agents
                .check(&ProfileHandle {
                    alias: "nope".into(),
                    kind: r.kind
                })
                .status,
            ProfileStatus::Unavailable
        );
    });
}

#[test]
fn failure_scenarios_map_to_the_allowlist() {
    each(|r| {
        for (scenario, want) in [
            ("unauthorized", AgentErrorCode::Unavailable),
            ("ratelimit", AgentErrorCode::RateLimited),
            ("context", AgentErrorCode::Context),
            ("otherfail", AgentErrorCode::Failed),
            ("silent-exec", AgentErrorCode::Timeout),
        ] {
            r.set(scenario);
            assert_eq!(r.code(r.chat()), want, "{:?} {scenario}", r.kind);
        }
        for scenario in ["malformed", "fenced"] {
            r.set(scenario);
            assert_eq!(
                r.code(r.generate()),
                AgentErrorCode::InvalidOutput,
                "{:?} {scenario}",
                r.kind
            );
        }
        assert_eq!(r.leftover_run_dirs(), 0, "{:?}", r.kind);
    });
}

#[test]
fn errors_carry_no_content() {
    each(|r| {
        for scenario in ["unauthorized", "otherfail", "malformed"] {
            r.set(scenario);
            let dump = format!("{:?}", r.run(r.generate()).0);
            for needle in [SECRET, "Tea was nice", "401"] {
                assert!(!dump.contains(needle), "{:?} {needle}", r.kind);
            }
        }
    });
}

#[test]
fn cancel_token_stops_a_running_agent() {
    each(|r| {
        r.set("silent-exec");
        let cancel = CancelToken::default();
        let run = Run::from(cancel.clone());
        let result = std::thread::scope(|s| {
            s.spawn(|| {
                std::thread::sleep(Duration::from_millis(500));
                run.request_cancel();
            });
            r.agents.run(r.chat(), &mut Sink::default(), &cancel)
        });
        assert_eq!(
            result.unwrap_err().0,
            AgentErrorCode::Cancelled,
            "{:?}",
            r.kind
        );
        assert_eq!(r.leftover_run_dirs(), 0);
    });
}

#[test]
fn shutdown_and_lease_end_behave_alike() {
    each(|r| {
        r.set("silent-exec");
        let result = std::thread::scope(|s| {
            s.spawn(|| {
                std::thread::sleep(Duration::from_millis(500));
                r.host.cancel_all();
            });
            r.run(r.chat()).0
        });
        assert_eq!(
            result.unwrap_err().0,
            AgentErrorCode::Cancelled,
            "{:?}",
            r.kind
        );
        r.set("");
        assert!(
            r.run(r.chat()).0.is_ok(),
            "lease end must not poison later runs"
        );
        r.host.shutdown();
        assert_eq!(r.code(r.chat()), AgentErrorCode::Cancelled, "{:?}", r.kind);
        assert_eq!(r.leftover_run_dirs(), 0);
    });
}

#[test]
fn missing_home_or_executable_is_unavailable() {
    each(|r| {
        std::fs::remove_dir_all(r.root.path().join("home")).unwrap();
        assert_eq!(
            r.agents.check(&r.handle(r.kind)).status,
            ProfileStatus::Unavailable,
            "{:?}",
            r.kind
        );
        assert_eq!(r.code(r.chat()), AgentErrorCode::Unavailable);
    });
}

#[test]
fn every_agent_kind_has_a_protocol() {
    // Apple joins in stage C; until then it is the one deliberately unregistered kind.
    let r = Rig::new(AgentKind::Codex);
    for k in AgentKind::ALL {
        assert_eq!(
            r.agents.registered(k),
            k != AgentKind::AppleFoundationModels,
            "{k:?}"
        );
    }
}

#[test]
fn registry_rejects_unregistered_kind() {
    let r = Rig::new(AgentKind::Codex);
    let h = r.handle(AgentKind::AppleFoundationModels);
    assert_eq!(r.agents.check(&h).status, ProfileStatus::Unavailable);
    let req = AgentRequest::new(h, Locale::parse("en").unwrap(), r.chat().mode().clone());
    assert_eq!(r.code(req), AgentErrorCode::Unavailable);
}

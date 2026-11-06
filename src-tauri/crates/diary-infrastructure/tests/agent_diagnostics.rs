//! Every refusal, failure, stop and check emits exactly one typed record with the expected tags,
//! for each CLI kind; no record carries content, paths or executable names.
mod support;
use diary_application::{
    agent::*,
    diagnostics::{
        ActionTag, AgentDetail, Diagnostics, EventCode, ExitTag, Level, PhaseTag, Record,
        StatusTag, StopTag,
    },
};
use diary_domain::authentic::DiaryDate;
use diary_infrastructure::agent::{
    host::Host,
    process::Limits,
    profiles::{Entry, ProfileTable},
    registry::Agents,
};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

const SECRET: &str = "SECRET-DIARY-TEXT";
const KINDS: [AgentKind; 3] = [AgentKind::Codex, AgentKind::Claude, AgentKind::Qwen];

#[derive(Default)]
struct Logs(Mutex<Vec<Record>>);
impl Diagnostics for Logs {
    fn emit(&self, r: Record) {
        self.0.lock().unwrap().push(r);
    }
}
impl Logs {
    fn take(&self) -> Vec<Record> {
        std::mem::take(&mut *self.0.lock().unwrap())
    }
    fn one(&self) -> (EventCode, Level, AgentDetail) {
        let mut v = self.take();
        assert_eq!(v.len(), 1, "exactly one record, got {v:?}");
        let r = v.pop().unwrap();
        (r.code, r.level, r.agent.expect("agent detail"))
    }
}

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
    logs: Arc<Logs>,
}
impl Rig {
    fn new(kind: AgentKind) -> Self {
        Self::with(kind, true, Some(fake()))
    }
    fn with(kind: AgentKind, make_home: bool, exe: Option<PathBuf>) -> Self {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("home");
        if make_home {
            std::fs::create_dir(&home).unwrap();
        }
        std::fs::create_dir(root.path().join("runtime")).unwrap();
        let table = Arc::new(ProfileTable::from_entries(vec![Entry {
            alias: "main".into(),
            kind,
            home: Some(home),
            executable: exe,
        }]));
        let logs = Arc::new(Logs::default());
        let host = support::host(root.path(), logs.clone(), limits());
        let agents = Agents::standard(table, host.clone());
        Self {
            kind,
            root,
            host,
            agents,
            logs,
        }
    }
    fn set(&self, name: &str, v: &str) {
        std::fs::write(self.root.path().join("home").join(name), v).unwrap();
    }
    fn handle(&self) -> ProfileHandle {
        ProfileHandle {
            alias: "main".into(),
            kind: self.kind,
        }
    }
    fn chat_for(&self, h: ProfileHandle) -> AgentRequest {
        AgentRequest::new(
            h,
            Locale::parse("en").unwrap(),
            Mode::Chat(ChatPacket::new(None, vec![], SECRET.into()).unwrap()),
        )
    }
    fn chat(&self) -> AgentRequest {
        self.chat_for(self.handle())
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
            self.handle(),
            Locale::parse("en").unwrap(),
            Mode::Candidate {
                index: 0,
                count: 1,
                packet,
            },
        )
    }
    fn run(&self, req: AgentRequest) -> Result<AgentOutput, AgentError> {
        self.agents.run(req, &mut Sink, &CancelToken::default())
    }
    fn unsupported_version(&self) -> &'static str {
        match self.kind {
            AgentKind::Codex => "codex-cli 9.0.0",
            AgentKind::Claude => "9.0.0 (Claude Code)",
            _ => "9.0.0",
        }
    }
}
struct Sink;
impl EventSink for Sink {
    fn event(&mut self, _: AgentEvent) {}
}

fn each(f: impl Fn(&Rig)) {
    for k in KINDS {
        f(&Rig::new(k));
    }
}

/// Asserts a refusal record: blocked, with the reason, code and kind.
fn blocked(r: &Rig, res: Result<AgentOutput, AgentError>, reason: Reason, code: AgentErrorCode) {
    assert_eq!(res.unwrap_err().0, code, "{:?} {reason:?}", r.kind);
    let (ev, _, d) = r.logs.one();
    assert_eq!(ev, EventCode::AgentBlocked, "{:?} {reason:?}", r.kind);
    assert_eq!(d.kind, r.kind);
    assert_eq!(d.reason, Some(reason), "{:?}", r.kind);
    assert_eq!(d.code, Some(code));
    assert_eq!(d.exit, None, "no process was started");
}

#[test]
fn refusals_before_a_process_each_emit_one_blocked_record() {
    each(|r| {
        // Unknown alias and a handle for another kind.
        let h = ProfileHandle {
            alias: "nope".into(),
            kind: r.kind,
        };
        blocked(
            r,
            r.run(r.chat_for(h)),
            Reason::ProfileUnknown,
            AgentErrorCode::Unavailable,
        );
        // A handle of another kind reaches that kind's adapter, which refuses the entry.
        let res = r.run(r.chat_for(ProfileHandle {
            alias: "main".into(),
            kind: AgentKind::AppleFoundationModels,
        }));
        assert_eq!(res.unwrap_err().0, AgentErrorCode::Unavailable);
        let (ev, _, d) = r.logs.one();
        assert_eq!(ev, EventCode::AgentBlocked);
        assert_eq!(d.kind, AgentKind::AppleFoundationModels);
        assert_eq!(d.reason, Some(Reason::KindMismatch));
        // Cancelled before start.
        let cancel = CancelToken::default();
        let run = Run::from(cancel.clone());
        run.request_cancel();
        let res = r.agents.run(r.chat(), &mut Sink, &cancel);
        blocked(
            r,
            res,
            Reason::CancelledBeforeStart,
            AgentErrorCode::Cancelled,
        );
        // Version problems.
        r.set("version", r.unsupported_version());
        blocked(
            r,
            r.run(r.chat()),
            Reason::VersionUnsupported,
            AgentErrorCode::Unavailable,
        );
        r.set("version", "garbage");
        blocked(
            r,
            r.run(r.chat()),
            Reason::VersionUnreadable,
            AgentErrorCode::Unavailable,
        );
        // Shutdown last: it refuses everything after it.
        r.host.shutdown();
        blocked(
            r,
            r.run(r.chat()),
            Reason::HostShuttingDown,
            AgentErrorCode::Cancelled,
        );
    });
}

#[test]
fn missing_home_directory_executable_are_blocked_with_their_reason() {
    for k in KINDS {
        let r = Rig::with(k, false, Some(fake()));
        blocked(
            &r,
            r.run(r.chat()),
            Reason::ProfileHomeMissing,
            AgentErrorCode::Unavailable,
        );
        std::fs::write(r.root.path().join("home"), b"x").unwrap();
        blocked(
            &r,
            r.run(r.chat()),
            Reason::ProfileHomeNotDirectory,
            AgentErrorCode::Unavailable,
        );
        let r = Rig::with(k, true, Some("/nonexistent/agent".into()));
        blocked(
            &r,
            r.run(r.chat()),
            Reason::ExecutableMissing,
            AgentErrorCode::Unavailable,
        );
    }
}

#[test]
fn runs_that_start_a_process_emit_one_run_record() {
    each(|r| {
        // Success.
        r.run(r.generate()).unwrap();
        let (ev, level, d) = r.logs.one();
        assert_eq!((ev, level), (EventCode::AgentRun, Level::Info));
        assert_eq!(d.action, Some(ActionTag::Generate));
        assert_eq!(d.phase, Some(PhaseTag::ValidatingOutput));
        assert_eq!(
            (d.exit, d.reason, d.code),
            (Some(ExitTag::Success), None, None)
        );
        assert!(d.input_bytes.unwrap() > 0 && d.output_bytes.unwrap() > 0);
        assert!(d.events.unwrap() >= 3);
        // Failure classes.
        for (scenario, code) in [
            ("unauthorized", AgentErrorCode::Unavailable),
            ("ratelimit", AgentErrorCode::RateLimited),
            ("context", AgentErrorCode::Context),
            ("otherfail", AgentErrorCode::Failed),
        ] {
            r.set("scenario", scenario);
            assert_eq!(r.run(r.chat()).unwrap_err().0, code);
            let (ev, level, d) = r.logs.one();
            assert_eq!(
                (ev, level),
                (EventCode::AgentRun, Level::Warn),
                "{scenario}"
            );
            assert_eq!(d.code, Some(code));
            assert_eq!(d.action, Some(ActionTag::Chat));
        }
        // Inactivity.
        r.set("scenario", "silent-exec");
        assert_eq!(r.run(r.chat()).unwrap_err().0, AgentErrorCode::Timeout);
        let (_, _, d) = r.logs.one();
        assert_eq!(d.stop, Some(StopTag::Inactivity));
        assert_eq!(d.code, Some(AgentErrorCode::Timeout));
        // Malformed output.
        r.set("scenario", "malformed");
        assert_eq!(
            r.run(r.generate()).unwrap_err().0,
            AgentErrorCode::InvalidOutput
        );
        let (_, _, d) = r.logs.one();
        assert_eq!(d.code, Some(AgentErrorCode::InvalidOutput));
        assert_eq!(d.phase, Some(PhaseTag::ValidatingOutput));
    });
}

#[test]
fn cancel_and_lease_end_are_distinguished() {
    each(|r| {
        r.set("scenario", "silent-exec");
        let cancel = CancelToken::default();
        let run = Run::from(cancel.clone());
        let res = std::thread::scope(|s| {
            s.spawn(|| {
                std::thread::sleep(Duration::from_millis(500));
                run.request_cancel();
            });
            r.agents.run(r.chat(), &mut Sink, &cancel)
        });
        assert_eq!(res.unwrap_err().0, AgentErrorCode::Cancelled);
        let (ev, level, d) = r.logs.one();
        assert_eq!((ev, level), (EventCode::AgentRun, Level::Info));
        assert_eq!((d.stop, d.reason), (Some(StopTag::Cancelled), None));

        let res = std::thread::scope(|s| {
            s.spawn(|| {
                std::thread::sleep(Duration::from_millis(500));
                r.host.cancel_all();
            });
            r.run(r.chat())
        });
        assert_eq!(res.unwrap_err().0, AgentErrorCode::Cancelled);
        let (_, _, d) = r.logs.one();
        assert_eq!(d.reason, Some(Reason::LeaseEnded));
        assert_eq!(d.stop, Some(StopTag::Cancelled));
    });
}

#[test]
fn checks_log_status_reason_and_whether_it_changed() {
    each(|r| {
        assert_eq!(r.agents.check(&r.handle()).status, ProfileStatus::Ready);
        let (ev, level, d) = r.logs.one();
        assert_eq!((ev, level), (EventCode::AgentCheck, Level::Info));
        assert_eq!(d.status, Some(StatusTag::Ready));
        assert_eq!((d.reason, d.changed), (None, Some(true)));
        r.agents.check(&r.handle());
        assert_eq!(r.logs.one().2.changed, Some(false));

        r.set("version", r.unsupported_version());
        let c = r.agents.check(&r.handle());
        assert_eq!(
            (c.status, c.reason),
            (
                ProfileStatus::UnsupportedVersion,
                Some(Reason::VersionUnsupported)
            )
        );
        let (_, level, d) = r.logs.one();
        assert_eq!(level, Level::Warn);
        assert_eq!(d.reason, Some(Reason::VersionUnsupported));
        assert_eq!(d.changed, Some(true));

        let c = r.agents.check(&ProfileHandle {
            alias: "nope".into(),
            kind: r.kind,
        });
        assert_eq!(c.reason, Some(Reason::ProfileUnknown));
        r.logs.one();
    });
}

#[test]
fn no_record_carries_content_paths_or_names() {
    each(|r| {
        r.set("scenario", "unauthorized");
        let _ = r.run(r.generate());
        r.set("scenario", "");
        let _ = r.run(r.generate());
        r.agents.check(&r.handle());
        let dump = format!("{:?}", r.logs.take());
        let root = r.root.path().to_string_lossy().into_owned();
        for needle in [
            SECRET,
            "Tea was nice",
            "401",
            root.as_str(),
            "fake_cli",
            "home",
            "runtime",
            "PATH",
        ] {
            assert!(!dump.contains(needle), "{:?} leaked {needle}", r.kind);
        }
    });
}

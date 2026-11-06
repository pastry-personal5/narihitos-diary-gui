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
    assets::{EMBEDDED, PromptAssets},
    codex::CodexAdapter,
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
            // The child failed after starting: either it exited non-zero or its result record was bad.
            assert!(
                matches!(d.reason, Some(Reason::ExitNonzero | Reason::ResultMissing)),
                "{scenario}: {:?}",
                d.reason
            );
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

// ---- Remaining reasons, limits and statuses -------------------------------------------------

/// A Codex adapter with chosen limits and prompt assets, logging into the returned sink.
fn codex_with(
    limits: Limits,
    assets: Option<PromptAssets>,
    runtime_is_a_file: bool,
) -> (tempfile::TempDir, CodexAdapter, Arc<Logs>) {
    let root = tempfile::tempdir().unwrap();
    let home = root.path().join("home");
    std::fs::create_dir(&home).unwrap();
    if runtime_is_a_file {
        std::fs::write(root.path().join("runtime"), b"x").unwrap();
    } else {
        std::fs::create_dir(root.path().join("runtime")).unwrap();
    }
    let table = Arc::new(ProfileTable::from_entries(vec![Entry {
        alias: "main".into(),
        kind: AgentKind::Codex,
        home: Some(home),
        executable: Some(fake()),
    }]));
    let logs = Arc::new(Logs::default());
    // Probes keep ordinary limits so a tiny run limit cannot break the version check.
    let host = support::host_with_probe(root.path(), logs.clone(), limits, self::limits());
    let adapter = match assets {
        Some(a) => CodexAdapter::with_prompt_assets(table, host, a),
        None => CodexAdapter::new(table, host),
    };
    (root, adapter, logs)
}
fn codex_chat() -> AgentRequest {
    AgentRequest::new(
        ProfileHandle {
            alias: "main".into(),
            kind: AgentKind::Codex,
        },
        Locale::parse("en").unwrap(),
        Mode::Chat(ChatPacket::new(None, vec![], SECRET.into()).unwrap()),
    )
}
fn run_codex(a: &CodexAdapter, req: AgentRequest) -> Result<AgentOutput, AgentError> {
    a.run(req, &mut Sink, &CancelToken::default())
}
fn only(logs: &Logs) -> (EventCode, Level, AgentDetail) {
    logs.one()
}
/// The embedded table with every Codex asset passed through `f`.
fn codex_assets(f: impl Fn(&'static str, &str) -> Option<String>) -> PromptAssets {
    let table: Vec<(AgentKind, &str, String)> = EMBEDDED
        .iter()
        .filter_map(|(k, n, t)| {
            if *k == AgentKind::Codex {
                f(n, t).map(|t| (*k, *n, t))
            } else {
                Some((*k, *n, (*t).to_owned()))
            }
        })
        .collect();
    let refs: Vec<(AgentKind, &str, &str)> =
        table.iter().map(|(k, n, t)| (*k, *n, t.as_str())).collect();
    PromptAssets::from_table(&refs)
}

#[test]
fn asset_and_pin_problems_are_blocked_with_their_reason() {
    for (assets, reason) in [
        (codex_assets(|_, _| None), Reason::AssetsMissing),
        (
            codex_assets(|n, t| {
                Some(if n == "chat" {
                    "garbage".into()
                } else {
                    t.to_owned()
                })
            }),
            Reason::AssetsInvalid,
        ),
        (
            codex_assets(|n, t| {
                Some(if n == "chat" {
                    t.replacen("model: ", "model: bad\"", 1)
                } else {
                    t.to_owned()
                })
            }),
            Reason::PromptPinRejected,
        ),
    ] {
        let (_root, a, logs) = codex_with(limits(), Some(assets), false);
        assert_eq!(
            run_codex(&a, codex_chat()).unwrap_err().0,
            AgentErrorCode::AssetsUnavailable,
            "{reason:?}"
        );
        let (ev, _, d) = only(&logs);
        assert_eq!((ev, d.reason), (EventCode::AgentBlocked, Some(reason)));
        assert_eq!(d.exit, None);
    }
}

#[test]
fn an_oversized_packet_is_blocked_before_any_process() {
    let (_root, a, logs) = codex_with(limits(), None, false);
    let doc = ChatDocument {
        kind: DocumentKind::Authentic,
        date: DiaryDate::from_ymd(2026, 11, 6).unwrap(),
        unsaved: false,
        body: "x".repeat(MAX_PACKET_BYTES + 1),
    };
    let req = AgentRequest::new(
        ProfileHandle {
            alias: "main".into(),
            kind: AgentKind::Codex,
        },
        Locale::parse("en").unwrap(),
        Mode::Chat(ChatPacket::new(Some(doc), vec![], "hi".into()).unwrap()),
    );
    assert!(run_codex(&a, req).is_err());
    let (ev, _, d) = only(&logs);
    assert_eq!(
        (ev, d.reason),
        (EventCode::AgentBlocked, Some(Reason::PacketTooLarge))
    );
    assert!(d.input_bytes.is_none() || d.input_bytes == Some(0));
}

#[test]
fn an_unusable_runtime_directory_is_blocked() {
    let (_root, a, logs) = codex_with(limits(), None, true);
    assert_eq!(
        run_codex(&a, codex_chat()).unwrap_err().0,
        AgentErrorCode::Failed
    );
    let (ev, _, d) = only(&logs);
    assert_eq!(
        (ev, d.reason),
        (EventCode::AgentBlocked, Some(Reason::RunDirUnavailable))
    );
    let c = a.check(&ProfileHandle {
        alias: "main".into(),
        kind: AgentKind::Codex,
    });
    assert_eq!(
        (c.status, c.reason),
        (ProfileStatus::Failed, Some(Reason::RunDirUnavailable))
    );
}

#[test]
fn process_limits_and_escapees_are_recorded_as_stop_causes() {
    let write = |root: &tempfile::TempDir, s: &str| {
        std::fs::write(root.path().join("home/scenario"), s).unwrap()
    };
    // Wall clock.
    let (root, a, logs) = codex_with(
        Limits {
            inactivity: Duration::from_secs(20),
            wall: Duration::from_millis(400),
            ..limits()
        },
        None,
        false,
    );
    write(&root, "silent-exec");
    assert_eq!(
        run_codex(&a, codex_chat()).unwrap_err().0,
        AgentErrorCode::Timeout
    );
    let (_, _, d) = only(&logs);
    assert_eq!(
        (d.stop, d.code),
        (Some(StopTag::Wall), Some(AgentErrorCode::Timeout))
    );
    // One record longer than the record limit, then a total over the output limit.
    for (l, stop, reason) in [
        (
            Limits {
                record: 8,
                ..limits()
            },
            StopTag::RecordTooLarge,
            Reason::RecordTooLarge,
        ),
        (
            Limits {
                total: 20,
                ..limits()
            },
            StopTag::OutputTooLarge,
            Reason::OutputTooLarge,
        ),
    ] {
        let (root, a, logs) = codex_with(l, None, false);
        write(&root, "");
        assert_eq!(
            run_codex(&a, codex_chat()).unwrap_err().0,
            AgentErrorCode::InvalidOutput
        );
        let (_, _, d) = only(&logs);
        assert_eq!((d.stop, d.reason), (Some(stop), Some(reason)));
    }
    // A descendant that left the process group.
    let (root, a, logs) = codex_with(limits(), None, false);
    write(&root, "escaped");
    assert_eq!(
        run_codex(&a, codex_chat()).unwrap_err().0,
        AgentErrorCode::Failed
    );
    let (_, level, d) = only(&logs);
    assert_eq!(level, Level::Error);
    assert_eq!(
        (d.reason, d.escaped),
        (Some(Reason::EscapedDescendants), Some(1))
    );
}

#[test]
fn qwen_reporting_another_model_is_recorded_as_init_mismatch() {
    let r = Rig::new(AgentKind::Qwen);
    r.set("scenario", "model-mismatch");
    assert_eq!(r.run(r.chat()).unwrap_err().0, AgentErrorCode::Unavailable);
    let (_, _, d) = r.logs.one();
    assert_eq!(d.reason, Some(Reason::InitMismatch));
}

#[test]
fn every_profile_status_is_logged_with_its_tag() {
    // One check per reachable status, on Codex. `available_auth_unknown` is a vocabulary value no
    // adapter produces today.
    let r = Rig::new(AgentKind::Codex);
    let tag = |r: &Rig| {
        r.agents.check(&r.handle());
        r.logs.one().2.status.unwrap()
    };
    assert_eq!(tag(&r), StatusTag::Ready);
    r.set("login", "none");
    assert_eq!(tag(&r), StatusTag::Unauthenticated);
    r.set("login", "boom");
    assert_eq!(tag(&r), StatusTag::Failed);
    r.set("login", "");
    r.set("version", "codex-cli 9.0.0");
    assert_eq!(tag(&r), StatusTag::UnsupportedVersion);
    let gone = Rig::with(AgentKind::Codex, true, Some("/nonexistent/agent".into()));
    gone.agents.check(&gone.handle());
    assert_eq!(gone.logs.one().2.status, Some(StatusTag::Missing));
    let nohome = Rig::with(AgentKind::Codex, false, Some(fake()));
    nohome.agents.check(&nohome.handle());
    assert_eq!(nohome.logs.one().2.status, Some(StatusTag::Unavailable));
    // Busy: a check made while a run is active.
    let busy = Rig::new(AgentKind::Codex);
    busy.set("scenario", "silent-exec");
    let status = std::thread::scope(|s| {
        s.spawn(|| {
            let _ = busy.run(busy.chat());
        });
        std::thread::sleep(Duration::from_millis(400));
        let c = busy.agents.check(&busy.handle());
        busy.host.cancel_all();
        c
    });
    assert_eq!(
        (status.status, status.reason),
        (ProfileStatus::Busy, Some(Reason::HostBusy))
    );
}

/// Adding a `Reason` fails to compile here until the new value names the test that proves its
/// record, so no reason can be added without log coverage.
fn proven_by(r: Reason) -> &'static str {
    use Reason::*;
    match r {
        ProfileUnknown | KindMismatch | CancelledBeforeStart | HostShuttingDown
        | VersionUnsupported | VersionUnreadable => {
            "refusals_before_a_process_each_emit_one_blocked_record"
        }
        ProfileHomeMissing | ProfileHomeNotDirectory | ExecutableMissing => {
            "missing_home_directory_executable_are_blocked_with_their_reason"
        }
        HostBusy => "every_profile_status_is_logged_with_its_tag",
        LeaseEnded => "cancel_and_lease_end_are_distinguished",
        RunDirUnavailable => "an_unusable_runtime_directory_is_blocked",
        AssetsMissing | AssetsInvalid | PromptPinRejected => {
            "asset_and_pin_problems_are_blocked_with_their_reason"
        }
        PacketTooLarge => "an_oversized_packet_is_blocked_before_any_process",
        RecordTooLarge | OutputTooLarge | EscapedDescendants => {
            "process_limits_and_escapees_are_recorded_as_stop_causes"
        }
        InitMismatch => "qwen_reporting_another_model_is_recorded_as_init_mismatch",
        ProbeTimedOut
        | ProbeFailed
        | SpawnFailed
        | OsUnsupported
        | DeviceIneligible
        | AppleIntelligenceDisabled
        | ModelNotReady
        | ModelUnavailableUnknown
        | LocaleUnsupported
        | NativeSessionBusy
        | NativeErrorUnmapped
        | HelperMissing
        | HelperUntrusted
        | HelperFailed => "agent_apple.rs (Apple helper and availability paths)",
        ExitNonzero | ResultMissing => "runs_that_start_a_process_emit_one_run_record",
    }
}

#[test]
fn every_reason_names_the_test_that_proves_its_record() {
    for r in Reason::ALL {
        assert!(!proven_by(r).is_empty(), "{r:?}");
    }
}

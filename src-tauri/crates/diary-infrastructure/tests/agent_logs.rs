//! Agent diagnostics through the real log worker and store: failing, refused and cancelled runs of
//! every CLI kind write only typed lines, a seeded-string scan of the log directory finds no content,
//! and the trace reader shows the refused, failed and cancelled runs and `recent` warnings.
mod support;
use diary_application::diagnostics::{EventCode, Role};
use diary_application::{agent::*, diagnostics::Diagnostics};
use diary_infrastructure::agent::{
    process::Limits,
    profiles::{Entry, ProfileTable},
    registry::Agents,
};
use diary_logging::{
    clock::SystemClock,
    codec::parse_line,
    private_fs::PrivateDir,
    store::LogStore,
    trace::{Selector, collect},
    worker::LogWorker,
};
use std::{os::unix::fs::PermissionsExt, path::PathBuf, sync::Arc, time::Duration};

const SECRET: &str = "SECRET-DIARY-TEXT-93f1";
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
struct Sink;
impl EventSink for Sink {
    fn event(&mut self, _: AgentEvent) {}
}

#[test]
fn real_log_files_hold_typed_agent_lines_and_no_content() {
    let base = tempfile::tempdir().unwrap();
    let base_path = base.path().canonicalize().unwrap();
    std::fs::set_permissions(&base_path, std::fs::Permissions::from_mode(0o700)).unwrap();
    let ws = PrivateDir::open(&base_path).unwrap();
    let clock = Arc::new(SystemClock);
    let store = LogStore::open(&ws, Role::Server, clock.clone()).unwrap();
    let worker = LogWorker::start(
        Some(store),
        Role::Server,
        None,
        clock,
        Box::new(std::io::sink()),
    );

    let work = tempfile::tempdir().unwrap();
    let work_path = work.path().canonicalize().unwrap();
    std::fs::create_dir(work_path.join("runtime")).unwrap();
    let mut entries = Vec::new();
    for k in KINDS {
        let home = work_path.join(format!("home-{}", k.asset_dir()));
        std::fs::create_dir(&home).unwrap();
        entries.push(Entry {
            alias: k.asset_dir().into(),
            kind: k,
            home: Some(home),
            executable: Some(fake()),
        });
    }
    let table = Arc::new(ProfileTable::from_entries(entries));
    let diag: Arc<dyn Diagnostics> = worker.clone();
    let host = support::host(&work_path, diag, limits());
    let agents = Agents::standard(table, host.clone());

    let request = |k: AgentKind| {
        AgentRequest::new(
            ProfileHandle {
                alias: k.asset_dir().into(),
                kind: k,
            },
            Locale::parse("en").unwrap(),
            Mode::Chat(ChatPacket::new(None, vec![], SECRET.into()).unwrap()),
        )
    };
    let set = |k: AgentKind, s: &str| {
        std::fs::write(
            work_path
                .join(format!("home-{}", k.asset_dir()))
                .join("scenario"),
            s,
        )
        .unwrap()
    };
    for k in KINDS {
        let h = ProfileHandle {
            alias: k.asset_dir().into(),
            kind: k,
        };
        agents.check(&h); // ready (info)
        set(k, "unauthorized");
        let _ = agents.run(request(k), &mut Sink, &CancelToken::default()); // failed
        set(k, "silent-exec");
        let cancel = CancelToken::default();
        let run = Run::from(cancel.clone());
        std::thread::scope(|s| {
            s.spawn(|| {
                std::thread::sleep(Duration::from_millis(500));
                run.request_cancel();
            });
            let _ = agents.run(request(k), &mut Sink, &cancel); // cancelled
        });
        set(k, "");
        let _ = agents.run(request(k), &mut Sink, &CancelToken::default()); // succeeds
    }
    // A refused run: an alias that is not configured.
    let _ = agents.run(
        AgentRequest::new(
            ProfileHandle {
                alias: "nope".into(),
                kind: AgentKind::Codex,
            },
            Locale::parse("en").unwrap(),
            Mode::Chat(ChatPacket::new(None, vec![], SECRET.into()).unwrap()),
        ),
        &mut Sink,
        &CancelToken::default(),
    );
    worker.shutdown(Duration::from_secs(5));

    // Seeded-string scan of every byte in the log directory.
    let mut all = String::new();
    let mut files = 0;
    for entry in walk(&base_path) {
        files += 1;
        all.push_str(&String::from_utf8_lossy(&std::fs::read(&entry).unwrap()));
    }
    assert!(files > 0, "the worker wrote a log file");
    let work_str = work_path.to_string_lossy().into_owned();
    for needle in [
        SECRET,
        "Tea was nice",
        "401",
        "Unauthorized",
        work_str.as_str(),
        "fake_cli",
        "home-",
        "scenario",
        "CODEX_HOME",
        "CLAUDE_CONFIG_DIR",
        "QWEN_HOME",
        "/usr/bin",
    ] {
        assert!(!all.contains(needle), "log directory leaked {needle:?}");
    }

    // Every line parses strictly; agent lines are present for each event code and kind.
    let lines: Vec<_> = all.lines().filter_map(parse_line).collect();
    assert_eq!(
        lines.len(),
        all.lines().count(),
        "every line is a valid typed line"
    );
    for code in [
        EventCode::AgentCheck,
        EventCode::AgentRun,
        EventCode::AgentBlocked,
    ] {
        assert!(lines.iter().any(|l| l.record.code == code), "{code:?}");
    }
    for k in KINDS {
        assert!(
            lines
                .iter()
                .any(|l| l.record.agent.as_ref().is_some_and(|a| a.kind == k)),
            "{k:?}"
        );
    }

    // The trace reader shows the refused and failed runs under `recent` (warn and above) and the
    // cancelled runs by their stop tag, re-rendered from typed values.
    let report = collect(&ws, &Selector::Recent(50), 200);
    assert_eq!(report.invalid, 0);
    let text = report.lines.concat();
    assert!(text.contains(r#""code":"agent.blocked""#), "{text}");
    assert!(text.contains(r#""reason":"profile_unknown""#), "{text}");
    assert!(text.contains(r#""code":"agent.run""#), "{text}");
    assert!(text.contains(r#""code":"unavailable""#), "{text}");
    assert!(!text.contains(SECRET));
    let cancelled = lines
        .iter()
        .filter(|l| {
            l.record
                .agent
                .as_ref()
                .is_some_and(|a| a.stop == Some(diary_application::diagnostics::StopTag::Cancelled))
        })
        .count();
    assert_eq!(cancelled, KINDS.len(), "one cancelled run per kind");
}

fn walk(dir: &std::path::Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(walk(&p));
        } else {
            out.push(p);
        }
    }
    out
}

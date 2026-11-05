use super::*;
use diary_application::diagnostics::{Component, ErrorTag, EventCode, Level, Record, Role};
use diary_logging::{clock::SystemClock, codec::parse_line, store::LogStore};
use serde_json::json;
use std::{
    os::unix::fs::PermissionsExt,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

#[derive(Clone, Default)]
struct Sink(Arc<Mutex<Vec<u8>>>);
impl Write for Sink {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
impl Sink {
    fn lines(&self) -> usize {
        String::from_utf8_lossy(&self.0.lock().unwrap())
            .lines()
            .count()
    }
}
fn wait_until(what: &str, mut cond: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(10);
    while !cond() {
        assert!(Instant::now() < end, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(5));
    }
}
fn test_log() -> (Arc<ClientLog>, Sink, Arc<AtomicU64>) {
    let sink = Sink::default();
    let ms = Arc::new(AtomicU64::new(0));
    let (s, m) = (sink.clone(), ms.clone());
    let log = ClientLog::with(
        Arc::new(SystemClock),
        Arc::new(move || m.load(Ordering::Relaxed)),
        Arc::new(move || Box::new(s.clone())),
    );
    (log, sink, ms)
}
struct Ws {
    _t: tempfile::TempDir,
    base: std::path::PathBuf,
}
impl Ws {
    fn new() -> Self {
        let t = tempfile::tempdir().unwrap();
        let base = t.path().canonicalize().unwrap();
        std::fs::set_permissions(&base, std::fs::Permissions::from_mode(0o700)).unwrap();
        Ws { _t: t, base }
    }
    fn dir(&self) -> PrivateDir {
        PrivateDir::open(&self.base).unwrap()
    }
    fn records(&self) -> Vec<diary_logging::codec::ParsedLine> {
        let dir = self.base.join("logs/client");
        let mut out = Vec::new();
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|e| e == "log") {
                for line in std::fs::read_to_string(path).unwrap().lines() {
                    out.push(parse_line(line).expect("every line parses"));
                }
            }
        }
        out
    }
}
fn ev(code: &str) -> ClientEvent {
    serde_json::from_value(json!({"code": code, "level": "warn"})).unwrap()
}
fn numbered(n: u32) -> Record {
    Record::new(EventCode::UiBridgeError, Level::Warn).count(n)
}

#[test]
fn unknown_code_extra_key_oversize_detail_and_server_only_code_rejected() {
    let (log, _sink, _ms) = test_log();
    assert!(
        serde_json::from_value::<ClientEvent>(
            json!({"code": "ui.bridge_error", "level": "warn", "extra": 1})
        )
        .is_err()
    );
    for code in ["server.started", "ignore previous instructions", ""] {
        assert_eq!(log.client_event(ev(code)), Err(Reject::Invalid), "{code}");
    }
    let with = |f: &dyn Fn(&mut ClientEvent)| {
        let mut e = ev("ui.bridge_error");
        f(&mut e);
        e
    };
    for bad in [
        with(&|e| e.level = "debug".into()),
        with(&|e| e.trace_id = Some("not-a-trace".into())),
        with(&|e| e.detail = Some(json!({"component": "editor", "note": "x"}))),
        with(&|e| e.detail = Some(json!({"component": "nowhere"}))),
        with(&|e| e.detail = Some(json!({"component": "editor", "pad": "x".repeat(600)}))),
    ] {
        assert_eq!(log.client_event(bad), Err(Reject::Invalid));
    }
    let good = with(&|e| {
        e.trace_id = Some("a".repeat(32));
        e.detail = Some(json!({"component": "editor", "count": 3, "error": "internal"}));
    });
    assert_eq!(log.client_event(good), Ok(()));
}

#[test]
fn twenty_one_events_in_one_second_drops_the_21st() {
    let (log, _sink, ms) = test_log();
    for _ in 0..20 {
        assert_eq!(log.client_event(ev("ui.render_error")), Ok(()));
    }
    ms.store(999, Ordering::Relaxed);
    assert_eq!(log.client_event(ev("ui.render_error")), Err(Reject::Rate));
    ms.store(1000, Ordering::Relaxed);
    assert_eq!(log.client_event(ev("ui.render_error")), Ok(()));
}

#[test]
fn buffer_keeps_first_256_lines_drops_newer_and_attach_flushes_in_order() {
    let (log, _sink, _ms) = test_log();
    for n in 0..300 {
        log.log(numbered(n));
    }
    assert_eq!(log.buffered(), 256);
    assert_eq!(log.mode(), Mode::Buffering);
    let ws = Ws::new();
    log.attach(&ws.dir());
    assert_eq!(log.mode(), Mode::File);
    log.shutdown(Duration::from_secs(5));
    let records = ws.records();
    let ui: Vec<u32> = records
        .iter()
        .filter(|r| r.record.code == EventCode::UiBridgeError)
        .filter_map(|r| r.record.count)
        .collect();
    assert_eq!(ui, (0..256).collect::<Vec<_>>());
    let dropped: Vec<u32> = records
        .iter()
        .filter(|r| r.record.code == EventCode::LogDropped)
        .filter_map(|r| r.record.count)
        .collect();
    assert_eq!(dropped, [44]);
    assert!(records.iter().all(|r| r.role == Role::Client));
}

#[test]
fn attach_falls_back_to_stderr_when_the_log_is_locked() {
    let ws = Ws::new();
    let _other_client = LogStore::open(&ws.dir(), Role::Client, Arc::new(SystemClock)).unwrap();
    let (log, sink, _ms) = test_log();
    log.log(numbered(1));
    log.attach(&ws.dir());
    assert_eq!(log.mode(), Mode::Stderr);
    log.log(numbered(2));
    wait_until("both lines on stderr", || sink.lines() == 2);
}

#[test]
fn accepted_event_is_logged_with_trace_and_component_and_rejection_is_logged_too() {
    let (log, _sink, _ms) = test_log();
    let ws = Ws::new();
    log.attach(&ws.dir());
    let mut good = ev("ui.bridge_error");
    good.trace_id = Some("b".repeat(32));
    good.detail = Some(json!({"component": "editor", "count": 3, "error": "internal"}));
    assert_eq!(log.client_event(good), Ok(()));
    assert_eq!(log.client_event(ev("server.started")), Err(Reject::Invalid));
    log.shutdown(Duration::from_secs(5));
    let records = ws.records();
    let ok = records
        .iter()
        .find(|r| r.record.code == EventCode::UiBridgeError)
        .unwrap();
    assert_eq!(ok.level, Level::Warn);
    assert_eq!(ok.record.trace.as_ref().unwrap().as_str(), "b".repeat(32));
    assert_eq!(ok.record.component, Some(Component::Editor));
    assert_eq!(ok.record.count, Some(3));
    assert_eq!(ok.record.error, Some(ErrorTag::Internal));
    assert!(
        records
            .iter()
            .any(|r| r.record.code == EventCode::ClientEventRejected)
    );
}

#[test]
fn concurrent_attach_calls_open_the_log_once_and_stay_on_file() {
    let ws = Ws::new();
    let (log, _sink, _ms) = test_log();
    let dir = std::sync::Arc::new(ws.dir());
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let (log, dir) = (log.clone(), dir.clone());
            std::thread::spawn(move || log.attach(&dir))
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }
    assert_eq!(log.mode(), Mode::File);
}

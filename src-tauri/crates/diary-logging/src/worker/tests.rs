use super::*;
use crate::clock::FakeClock;
use crate::codec::parse_line;
use crate::private_fs::PrivateDir;
use std::os::unix::fs::PermissionsExt;
use std::sync::Mutex;
use std::time::Instant;

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
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().unwrap()).into_owned()
    }
}

/// A writer that blocks while the test holds `gate`.
struct Gated {
    gate: Arc<Mutex<()>>,
    sink: Sink,
}
impl Write for Gated {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        let _g = self.gate.lock().unwrap();
        self.sink.write(b)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn wait_until(what: &str, mut cond: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(10);
    while !cond() {
        assert!(Instant::now() < end, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(5));
    }
}
fn record() -> Record {
    Record::new(EventCode::ServerStarted, Level::Info)
}
fn cfg() -> WorkerConfig {
    WorkerConfig {
        tick: Duration::from_millis(10),
        ..WorkerConfig::default()
    }
}
fn clock() -> Arc<FakeClock> {
    FakeClock::at("2026-11-05T09:00:00.000+09:00")
}
fn stalled() -> (Arc<LogWorker>, Arc<Mutex<()>>, Sink) {
    let gate = Arc::new(Mutex::new(()));
    let sink = Sink::default();
    let w = LogWorker::start_with(
        None,
        Role::Server,
        None,
        clock(),
        Box::new(Gated {
            gate: gate.clone(),
            sink: sink.clone(),
        }),
        cfg(),
    );
    (w, gate, sink)
}
fn dropped_counts(text: &str) -> Vec<u32> {
    text.lines()
        .filter_map(parse_line)
        .filter(|p| p.record.code == EventCode::LogDropped)
        .filter_map(|p| p.record.count)
        .collect()
}

#[test]
fn emit_never_blocks_when_writer_is_stalled_and_overflow_is_counted() {
    let (w, gate, sink) = stalled();
    let held = gate.lock().unwrap();
    let start = Instant::now();
    for _ in 0..10_000 {
        w.emit(record());
    }
    assert!(start.elapsed() < Duration::from_secs(1));
    drop(held);
    w.shutdown(Duration::from_secs(5));
    let counts = dropped_counts(&sink.text());
    assert_eq!(counts.len(), 1, "{}", sink.text().len());
    assert!(counts[0] > 9_000 && counts[0] <= 10_000 - 256, "{counts:?}");
}

#[test]
fn queue_holds_256_records_so_300_emits_drop_43_or_44() {
    let (w, gate, sink) = stalled();
    let held = gate.lock().unwrap();
    for _ in 0..300 {
        w.emit(record());
    }
    drop(held);
    w.shutdown(Duration::from_secs(5));
    let counts = dropped_counts(&sink.text());
    assert!(matches!(counts.as_slice(), [43] | [44]), "{counts:?}");
}

#[test]
fn shutdown_drain_is_bounded() {
    let (w, gate, _sink) = stalled();
    let held = gate.lock().unwrap();
    for _ in 0..10 {
        w.emit(record());
    }
    let start = Instant::now();
    w.shutdown(Duration::from_millis(100));
    assert!(start.elapsed() < Duration::from_secs(2));
    drop(held); // lets the abandoned thread finish
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
    fn store(&self, clock: &Arc<FakeClock>) -> LogStore {
        let dir = PrivateDir::open(&self.base).unwrap();
        LogStore::open(&dir, Role::Server, clock.clone()).unwrap()
    }
}

#[test]
fn at_most_one_stderr_notice_per_minute() {
    let ws = Ws::new();
    let clock = clock();
    let sink = Sink::default();
    let w = LogWorker::start_with(
        Some(ws.store(&clock)),
        Role::Server,
        None,
        clock.clone(),
        Box::new(sink.clone()),
        cfg(),
    );
    let notices = || sink.text().matches(NOTICE.trim_end()).count();
    w.emit(record());
    wait_until("first record", || w.processed() == 1);
    assert_eq!(notices(), 0);
    // Break the directory, then force a rotation so every write fails.
    std::fs::remove_dir_all(ws.base.join("logs/server")).unwrap();
    clock.advance(86_400);
    for _ in 0..5 {
        w.emit(record());
    }
    wait_until("failures", || w.processed() == 6);
    assert_eq!(notices(), 1);
    clock.advance(59);
    w.emit(record());
    wait_until("59s", || w.processed() == 7);
    assert_eq!(notices(), 1);
    clock.advance(2);
    w.emit(record());
    wait_until("61s", || w.processed() == 8);
    assert_eq!(notices(), 2);
    w.shutdown(Duration::from_secs(1));
}

#[test]
fn hourly_prune_runs_while_idle() {
    let ws = Ws::new();
    let clock = clock();
    let w = LogWorker::start_with(
        Some(ws.store(&clock)),
        Role::Server,
        None,
        clock.clone(),
        Box::new(Sink::default()),
        cfg(),
    );
    let stale = ws.base.join("logs/server/server-2026-10-01.log");
    std::fs::write(&stale, "old\n").unwrap();
    std::fs::set_permissions(&stale, std::fs::Permissions::from_mode(0o600)).unwrap();
    let old = std::time::SystemTime::UNIX_EPOCH
        + Duration::from_secs((clock.now().epoch_secs - 100 * 3600) as u64);
    std::fs::File::options()
        .write(true)
        .open(&stale)
        .unwrap()
        .set_modified(old)
        .unwrap();
    std::thread::sleep(Duration::from_millis(50));
    assert!(stale.exists(), "must not prune before an hour passes");
    clock.advance(3601);
    wait_until("idle prune", || !stale.exists());
    w.shutdown(Duration::from_secs(1));
}

#[test]
fn records_reach_the_store_as_parsable_lines() {
    let ws = Ws::new();
    let clock = clock();
    let w = LogWorker::start_with(
        Some(ws.store(&clock)),
        Role::Server,
        None,
        clock.clone(),
        Box::new(Sink::default()),
        cfg(),
    );
    w.emit(record());
    wait_until("write", || w.processed() == 1);
    let text = std::fs::read_to_string(ws.base.join("logs/server/server-2026-11-05.log")).unwrap();
    let parsed = parse_line(text.lines().next().unwrap()).unwrap();
    assert_eq!(parsed.record.code, EventCode::ServerStarted);
    assert_eq!(parsed.role, Role::Server);
    w.shutdown(Duration::from_secs(1));
}

#[test]
fn emit_line_writes_a_prerendered_line() {
    let sink = Sink::default();
    let w = LogWorker::start_with(
        None,
        Role::Server,
        None,
        clock(),
        Box::new(sink.clone()),
        cfg(),
    );
    w.emit_line("{\"x\":1}\n".into());
    wait_until("line", || sink.text() == "{\"x\":1}\n");
    w.shutdown(Duration::from_secs(1));
}

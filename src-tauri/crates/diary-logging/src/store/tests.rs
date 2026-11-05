use super::*;
use crate::clock::FakeClock;
use std::os::unix::fs::PermissionsExt;
use std::time::{Duration, SystemTime};

const LINE: &str = "aaaaaaaaaaaaaaaaaaa\n"; // 20 bytes

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
    fn logs(&self) -> std::path::PathBuf {
        self.base.join("logs/server")
    }
    fn names(&self) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(self.logs())
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .filter(|n| n != ".lock")
            .collect();
        v.sort();
        v
    }
    fn read(&self, name: &str) -> String {
        std::fs::read_to_string(self.logs().join(name)).unwrap()
    }
    fn store(&self, clock: &Arc<FakeClock>, limits: Limits) -> LogStore {
        LogStore::open_with(&self.dir(), Role::Server, clock.clone(), limits).unwrap()
    }
    /// Creates a role-owned file with an explicit age in seconds relative to `clock`.
    fn aged(&self, name: &str, clock: &FakeClock, age_secs: i64) {
        std::fs::create_dir_all(self.logs()).unwrap();
        for d in [self.base.join("logs"), self.logs()] {
            std::fs::set_permissions(d, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        let path = self.logs().join(name);
        std::fs::write(&path, "old\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let t = clock.now().epoch_secs - age_secs;
        std::fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(t as u64))
            .unwrap();
    }
}
fn small() -> Limits {
    Limits {
        file_bytes: 50,
        max_files: 8,
        total_bytes: 10_000,
    }
}
fn at(s: &str) -> Arc<FakeClock> {
    FakeClock::at(s)
}
fn mode(p: &std::path::Path) -> u32 {
    std::fs::metadata(p).unwrap().permissions().mode() & 0o7777
}

#[test]
fn open_creates_0700_dirs_and_0600_file() {
    let ws = Ws::new();
    let clock = at("2026-11-05T09:00:00.000+09:00");
    let mut s = ws.store(&clock, Limits::default());
    s.append(LINE).unwrap();
    assert_eq!(mode(&ws.base.join("logs")), 0o700);
    assert_eq!(mode(&ws.logs()), 0o700);
    assert_eq!(mode(&ws.logs().join("server-2026-11-05.log")), 0o600);
    assert_eq!(ws.read("server-2026-11-05.log"), LINE);
}

#[test]
fn second_open_is_locked() {
    let ws = Ws::new();
    let clock = at("2026-11-05T09:00:00.000+09:00");
    let _first = ws.store(&clock, Limits::default());
    let second = LogStore::open_with(&ws.dir(), Role::Server, clock.clone(), Limits::default());
    assert_eq!(second.err(), Some(LogError::Locked));
}

#[test]
fn rolls_over_at_local_midnight_including_dst() {
    let ws = Ws::new();
    let clock = at("2026-11-05T23:59:59.000+09:00");
    let mut s = ws.store(&clock, Limits::default());
    s.append(LINE).unwrap();
    clock.advance(1);
    s.append(LINE).unwrap();
    assert_eq!(
        ws.names(),
        ["server-2026-11-05.log", "server-2026-11-06.log"]
    );

    // US fall-back day: 01:30 happens twice but the local date does not change.
    let ws = Ws::new();
    let clock = at("2026-11-01T01:30:00.000-04:00");
    let mut s = ws.store(&clock, Limits::default());
    s.append(LINE).unwrap();
    clock.advance(3600); // now 01:30 -05:00
    s.append(LINE).unwrap();
    assert_eq!(ws.names(), ["server-2026-11-01.log"]);
    assert_eq!(ws.read("server-2026-11-01.log").lines().count(), 2);
    clock.advance(22 * 3600 + 30 * 60); // 00:00 next local day, 25 hours after start of day
    s.append(LINE).unwrap();
    assert_eq!(
        ws.names(),
        ["server-2026-11-01.log", "server-2026-11-02.log"]
    );
}

#[test]
fn size_rotation_uses_next_free_suffix_and_never_truncates_after_restart() {
    let ws = Ws::new();
    let clock = at("2026-11-05T09:00:00.000+09:00");
    let mut s = ws.store(&clock, small());
    for _ in 0..3 {
        s.append(LINE).unwrap();
    }
    assert_eq!(
        ws.names(),
        ["server-2026-11-05.1.log", "server-2026-11-05.log"]
    );
    drop(s); // restart
    let mut s = ws.store(&clock, small());
    s.append(LINE).unwrap();
    s.append(LINE).unwrap();
    assert_eq!(
        ws.names(),
        [
            "server-2026-11-05.1.log",
            "server-2026-11-05.2.log",
            "server-2026-11-05.log"
        ]
    );
    assert_eq!(ws.read("server-2026-11-05.log").len(), 40);
    assert_eq!(ws.read("server-2026-11-05.1.log").len(), 40);
    assert_eq!(ws.read("server-2026-11-05.2.log").len(), 20);
}

#[test]
fn backward_clock_change_appends_to_existing_date_file_without_truncation() {
    let ws = Ws::new();
    let clock = at("2026-11-05T09:00:00.000+09:00");
    let mut s = ws.store(&clock, Limits::default());
    s.append(LINE).unwrap();
    clock.advance(86_400);
    s.append(LINE).unwrap();
    clock.advance(-86_400);
    s.append(LINE).unwrap();
    assert_eq!(ws.read("server-2026-11-05.log").lines().count(), 2);
    assert_eq!(ws.read("server-2026-11-06.log").lines().count(), 1);
}

#[test]
fn caps_files_and_total_bytes_drop_without_deleting_younger_files() {
    let ws = Ws::new();
    let clock = at("2026-11-05T09:00:00.000+09:00");
    let limits = Limits {
        file_bytes: 50,
        max_files: 3,
        total_bytes: 10_000,
    };
    let mut s = ws.store(&clock, limits);
    for _ in 0..3 {
        s.append(LINE).unwrap();
        clock.advance(86_400);
    }
    assert_eq!(s.append(LINE), Err(LogError::Capped));
    assert_eq!(ws.names().len(), 3);
    assert_eq!(ws.read("server-2026-11-05.log"), LINE);
    // Age pruning frees capacity; the next record is accepted again.
    ws.aged("server-2026-11-05.log", &clock, 73 * 3600);
    s.append(LINE).unwrap();
    assert_eq!(ws.names().len(), 3);
    assert!(!ws.names().contains(&"server-2026-11-05.log".to_string()));

    let ws = Ws::new();
    let limits = Limits {
        file_bytes: 50,
        max_files: 8,
        total_bytes: 40,
    };
    let mut s = ws.store(&clock, limits);
    s.append(LINE).unwrap();
    s.append(LINE).unwrap();
    assert_eq!(s.append(LINE), Err(LogError::Capped));
}

#[test]
fn prune_keeps_exactly_72h_and_deletes_72h_plus_one_second_on_open() {
    let ws = Ws::new();
    let clock = at("2026-11-05T09:00:00.000+09:00");
    ws.aged("server-2026-11-02.log", &clock, 72 * 3600);
    ws.aged("server-2026-11-01.log", &clock, 72 * 3600 + 1);
    let _s = ws.store(&clock, Limits::default());
    assert_eq!(ws.names(), ["server-2026-11-02.log"]);
}

#[test]
fn prune_runs_on_rotation_in_a_long_running_process() {
    let ws = Ws::new();
    let clock = at("2026-11-05T09:00:00.000+09:00");
    let mut s = ws.store(&clock, Limits::default());
    s.append(LINE).unwrap();
    ws.aged("server-2026-10-01.log", &clock, 100 * 3600);
    assert!(ws.names().contains(&"server-2026-10-01.log".to_string()));
    clock.advance(86_400);
    s.append(LINE).unwrap();
    assert!(!ws.names().contains(&"server-2026-10-01.log".to_string()));
}

#[test]
fn prune_never_deletes_active_unrelated_other_role_or_symlinked_names() {
    let ws = Ws::new();
    let clock = at("2026-11-05T09:00:00.000+09:00");
    let mut s = ws.store(&clock, Limits::default());
    s.append(LINE).unwrap();
    ws.aged("server-2026-11-05.log", &clock, 200 * 3600); // active file, ancient mtime
    ws.aged("notes.txt", &clock, 200 * 3600);
    ws.aged("client-2026-01-01.log", &clock, 200 * 3600);
    ws.aged("server-2026-01-02.log.bak", &clock, 200 * 3600);
    ws.aged("server-2026-01-03.01.log", &clock, 200 * 3600); // non-canonical suffix
    std::os::unix::fs::symlink(
        ws.logs().join("notes.txt"),
        ws.logs().join("server-2026-01-01.log"),
    )
    .unwrap();
    let before = ws.names();
    assert_eq!(s.prune(), Ok(0));
    assert_eq!(ws.names(), before);
}

#[test]
fn append_rejects_line_over_2kib() {
    let ws = Ws::new();
    let clock = at("2026-11-05T09:00:00.000+09:00");
    let mut s = ws.store(&clock, Limits::default());
    assert_eq!(s.append(&"x".repeat(2049)), Err(LogError::Oversize));
}

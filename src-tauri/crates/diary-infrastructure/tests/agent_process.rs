//! Supervisor behavior against the `fake_cli` example (built by `cargo test`, not by `--test` alone).
use diary_application::agent::Run;
use diary_infrastructure::agent::process::*;
use std::{
    ffi::OsString,
    path::PathBuf,
    time::{Duration, Instant},
};

fn fake() -> PathBuf {
    let p = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples/fake_cli");
    assert!(
        p.is_file(),
        "build examples first: cargo test builds fake_cli"
    );
    p
}
fn quick() -> Limits {
    Limits {
        inactivity: Duration::from_secs(20),
        wall: Duration::from_secs(30),
        record: 1024,
        total: 64 * 1024,
        grace: Duration::from_millis(400),
        drain: Duration::from_millis(500),
    }
}
fn spec(dir: &std::path::Path, args: &[&str], stdin: &str, limits: Limits) -> Spec {
    Spec {
        executable: fake(),
        argv: args.iter().map(OsString::from).collect(),
        env: vec![
            ("HOME".into(), "/h".into()),
            ("PATH".into(), "/usr/bin".into()),
        ],
        stdin: stdin.as_bytes().to_vec(),
        cwd: dir.to_path_buf(),
        limits,
        shutdown: None,
    }
}
fn lines(recs: &[Vec<u8>]) -> Vec<String> {
    recs.iter()
        .map(|r| String::from_utf8_lossy(r).into_owned())
        .collect()
}
/// Runs to completion; `cancel_on` cancels when a record starts with that prefix.
fn go(
    args: &[&str],
    stdin: &str,
    limits: Limits,
    cancel_on: Option<&str>,
) -> (Outcome, Vec<String>) {
    let dir = tempfile::tempdir().unwrap();
    let run = Run::new();
    let mut recs = Vec::new();
    let o = Supervisor::run(
        spec(dir.path(), args, stdin, limits),
        &run.token(),
        &mut |r| {
            recs.push(r.to_vec());
            if cancel_on.is_some_and(|p| r.starts_with(p.as_bytes())) {
                run.request_cancel();
            }
        },
    )
    .unwrap();
    (o, lines(&recs))
}
fn find(recs: &[String], key: &str) -> String {
    recs.iter()
        .find_map(|l| l.strip_prefix(key).map(|v| v.trim().to_owned()))
        .unwrap_or_else(|| panic!("no {key} in {recs:?}"))
}
fn dead(pid: i32) -> bool {
    for _ in 0..40 {
        // SAFETY: signal 0 only probes existence.
        if unsafe { libc::kill(pid, 0) } != 0 {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

#[test]
fn child_has_own_group_and_private_cwd() {
    let dir = tempfile::tempdir().unwrap();
    let run = Run::new();
    let mut recs = Vec::new();
    Supervisor::run(
        spec(dir.path(), &["env"], "", quick()),
        &run.token(),
        &mut |r| recs.push(r.to_vec()),
    )
    .unwrap();
    let recs = lines(&recs);
    let (pid, pgid) = (find(&recs, "pid "), find(&recs, "pgid "));
    assert_eq!(pid, pgid, "child leads its own group");
    // SAFETY: plain syscall.
    assert_ne!(pgid, unsafe { libc::getpgrp() }.to_string());
    assert_eq!(
        PathBuf::from(find(&recs, "cwd ")).canonicalize().unwrap(),
        dir.path().canonicalize().unwrap()
    );
}

#[test]
fn environment_is_allowlisted() {
    let (_, recs) = go(&["env"], "", quick(), None);
    let mut keys: Vec<_> = recs
        .iter()
        .filter_map(|l| l.strip_prefix("env "))
        .map(|kv| kv.split('=').next().unwrap().to_owned())
        .filter(|k| k != "__CF_USER_TEXT_ENCODING") // injected by macOS for any process
        .collect();
    keys.sort();
    assert_eq!(keys, ["HOME", "PATH"]);
}

#[test]
fn stdin_carries_the_prompt_and_is_closed() {
    let (o, recs) = go(&["echo-stdin"], "a\nb\nlast", quick(), None);
    assert_eq!(recs, ["a", "b", "last"]);
    assert_eq!((o.exit, o.stop), (Exit::Success, None));
}

#[test]
fn flooding_stderr_does_not_deadlock() {
    let t = Instant::now();
    let mut l = quick();
    l.total = 8 * 1024 * 1024;
    let (o, recs) = go(&["flood-stderr"], "", l, None);
    assert_eq!((o.exit, o.stop), (Exit::Success, None));
    assert_eq!(recs, ["done"]);
    assert_eq!(o.stderr_tail.len(), 2048);
    assert!(t.elapsed() < Duration::from_secs(10));
}

#[test]
fn record_over_1mib_fails() {
    let (o, _) = go(&["big-record"], "", quick(), None); // quick() scales 1 MiB down to 1 KiB
    assert_eq!(o.stop, Some(Stop::RecordTooLarge));
}

#[test]
fn total_output_over_10mib_fails() {
    let (o, _) = go(&["flood-stdout"], "", quick(), None); // scaled to 64 KiB
    assert_eq!(o.stop, Some(Stop::OutputTooLarge));
}

#[test]
fn inactivity_deadline_times_out() {
    let mut l = quick();
    l.inactivity = Duration::from_millis(400);
    let (o, _) = go(&["silent"], "", l, None);
    assert_eq!(o.stop, Some(Stop::Inactivity));
}

#[test]
fn wall_deadline_times_out() {
    let mut l = quick();
    l.wall = Duration::from_millis(600); // output keeps flowing, so only wall time can fire
    let (o, _) = go(&["drip"], "", l, None);
    assert_eq!(o.stop, Some(Stop::Wall));
}

#[test]
fn cancel_terminates_group() {
    let (o, recs) = go(&["grandchild"], "", quick(), Some("grandchild"));
    assert_eq!(o.stop, Some(Stop::Cancelled));
    assert!(dead(find(&recs, "grandchild ").parse().unwrap()));
}

#[test]
fn term_ignoring_child_is_killed_after_grace() {
    let t = Instant::now();
    let (o, recs) = go(&["ignore-term"], "", quick(), Some("ready"));
    assert_eq!(o.stop, Some(Stop::Cancelled));
    assert_eq!(o.exit, Exit::Signal(libc::SIGKILL));
    assert!(
        t.elapsed() >= Duration::from_millis(400),
        "waited out the grace"
    );
    assert!(dead(find(&recs, "pid ").parse().unwrap()));
}

#[test]
fn terminal_only_after_reap() {
    let (_, recs) = go(&["ignore-term"], "", quick(), Some("ready"));
    // A zombie would still answer signal 0; the leader must be fully reaped on return.
    let pid: i32 = find(&recs, "pid ").parse().unwrap();
    // SAFETY: signal 0 only probes existence.
    assert_eq!(unsafe { libc::kill(pid, 0) }, -1);
}

#[test]
fn grandchild_in_group_is_killed() {
    let (o, recs) = go(&["grandchild", "exit"], "", quick(), None);
    assert_eq!((o.exit, o.stop, o.escaped), (Exit::Success, None, 0));
    assert!(dead(find(&recs, "grandchild ").parse().unwrap()));
}

#[test]
fn escaped_descendant_is_detected() {
    let (o, recs) = go(&["escape"], "", quick(), None);
    assert_eq!(o.escaped, 1);
    assert!(
        dead(find(&recs, "escapee ").parse().unwrap()),
        "best-effort kill"
    );
}

#[test]
fn no_signal_to_recycled_pid() {
    let mut c = std::process::Command::new("/bin/sleep")
        .arg("30")
        .spawn()
        .unwrap();
    let id = identify(c.id() as i32).expect("live child");
    let stale = Ident {
        start: (id.start.0, id.start.1 + 1),
        ..id
    };
    assert!(
        !kill_if_same(stale),
        "a different start time means a different process"
    );
    assert!(c.try_wait().unwrap().is_none());
    assert!(kill_if_same(id));
    c.wait().unwrap();
}

#[test]
fn shutdown_flag_terminates_group() {
    let dir = tempfile::tempdir().unwrap();
    let flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut s = spec(dir.path(), &["grandchild"], "", quick());
    s.shutdown = Some(flag.clone());
    let mut recs = Vec::new();
    let o = Supervisor::run(s, &Run::new().token(), &mut |r| {
        recs.push(r.to_vec());
        flag.store(true, std::sync::atomic::Ordering::SeqCst);
    })
    .unwrap();
    assert_eq!(o.stop, Some(Stop::Cancelled));
    assert!(dead(find(&lines(&recs), "grandchild ").parse().unwrap()));
}

#[test]
fn unwinding_callback_does_not_leak_the_group() {
    let dir = tempfile::tempdir().unwrap();
    let mut pid = String::new();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = Supervisor::run(
            spec(dir.path(), &["grandchild"], "", quick()),
            &Run::new().token(),
            &mut |r| {
                pid = String::from_utf8_lossy(r)
                    .trim_start_matches("grandchild ")
                    .to_owned();
                panic!("callback failed");
            },
        );
    }));
    assert!(result.is_err());
    assert!(
        dead(pid.parse().unwrap()),
        "the guard killed the whole group"
    );
}

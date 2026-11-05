use super::*;

#[test]
fn closure_runs_once_and_its_value_is_returned() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("f.md");
    let mut runs = 0;
    let value = write(&path, || {
        runs += 1;
        std::fs::write(&path, b"x").unwrap();
        7
    });
    assert_eq!(value, Ok(7));
    assert_eq!(runs, 1);
    assert_eq!(
        read(&path, || std::fs::read(&path).unwrap()),
        Ok(b"x".to_vec())
    );
}

#[test]
fn absent_files_can_be_coordinated() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(read(&dir.path().join("none.md"), || 1), Ok(1));
}

#[test]
fn errors_inside_the_closure_propagate() {
    let dir = tempfile::tempdir().unwrap();
    let r: Result<Result<(), &str>, _> = write(&dir.path().join("f"), || Err("boom"));
    assert_eq!(r, Ok(Err("boom")));
}

#[test]
fn two_threads_writing_one_path_are_serialized() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("f.md");
    let inside = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|s| {
        for _ in 0..2 {
            s.spawn(|| {
                write(&path, || {
                    assert_eq!(inside.fetch_add(1, std::sync::atomic::Ordering::SeqCst), 0);
                    std::thread::sleep(std::time::Duration::from_millis(150));
                    inside.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                })
                .unwrap();
            });
        }
    });
}

/// Re-executed by the test below as a separate process that holds a coordinated write.
#[test]
fn child_holder() {
    if let Ok(path) = std::env::var("DIARY_COORD_HOLD") {
        let path = std::path::PathBuf::from(path);
        write(&path, || {
            std::fs::write(path.with_extension("held"), b"1").unwrap();
            std::thread::sleep(std::time::Duration::from_millis(2500));
        })
        .unwrap();
    }
}

#[test]
fn a_writer_in_another_process_serializes_with_us() {
    use std::time::{Duration, Instant};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("f.md");
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "coordination::tests::child_holder",
            "--test-threads=1",
        ])
        .env("DIARY_COORD_HOLD", &path)
        .stdout(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let held = path.with_extension("held");
    let started = Instant::now();
    while !held.exists() {
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "child never took the coordination"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    let waited = Instant::now();
    write(&path, || ()).unwrap();
    let blocked_for = waited.elapsed();
    assert!(
        blocked_for >= Duration::from_millis(1000),
        "second process's write ran after only {blocked_for:?}; coordination did not serialize"
    );
    assert!(child.wait().unwrap().success());
}

//! Opt-in standalone proof through the production registry and command adapter.
use diary_application::{agent::Run, image::*};
use diary_infrastructure::image::CommandEngine;
use std::{
    collections::BTreeSet,
    fs,
    io::Read,
    path::PathBuf,
    process::{Child, Command},
    sync::Arc,
    time::Duration,
};

struct Sentinel(Child);
impl Drop for Sentinel {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn descendants() -> BTreeSet<u32> {
    let output = Command::new("/bin/ps")
        .args(["-axo", "pid=,ppid="])
        .output()
        .unwrap();
    assert!(output.status.success());
    let rows: Vec<(u32, u32)> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| {
            let mut fields = line.split_whitespace();
            (
                fields.next().unwrap().parse().unwrap(),
                fields.next().unwrap().parse().unwrap(),
            )
        })
        .collect();
    let mut owned = BTreeSet::from([std::process::id()]);
    loop {
        let before = owned.len();
        for (pid, parent) in &rows {
            if owned.contains(parent) {
                owned.insert(*pid);
            }
        }
        if owned.len() == before {
            break;
        }
    }
    owned.remove(&std::process::id());
    owned
}

fn main() {
    assert_eq!(std::env::var("DIARY_IMAGE_LIVE").as_deref(), Ok("1"));
    let evidence = PathBuf::from(std::env::var_os("DIARY_IMAGE_EVIDENCE").unwrap());
    let program = PathBuf::from(std::env::var_os("DIARY_IMAGE_LIVE_PROGRAM").unwrap());
    assert!(evidence.is_absolute() && program.is_absolute());
    let render_run = Run::new();
    let run = Run::new();
    let stop_runs = [Run::from(render_run.token()), Run::from(run.token())];
    // The harness keeps stdin open. EOF or a byte requests cancellation through
    // the real supervisor, including its separately owned renderer group.
    std::thread::spawn(move || {
        let _ = std::io::stdin().read(&mut [0u8]);
        for run in stop_runs {
            run.request_cancel();
        }
    });
    let root = tempfile::tempdir_in(&evidence).unwrap();
    let registry = Registry::default();
    registry
        .replace(vec![(
            "krea2".into(),
            "Krea 2 (local)".into(),
            Arc::new(CommandEngine {
                shutdown: registry.shutdown_signal(),
                program,
                root: root.path().into(),
                timeout: Duration::from_secs(1800),
            }),
        )])
        .unwrap();
    assert_eq!(
        registry.check("krea2").unwrap().unwrap().status,
        Availability::Ready
    );
    let request = RenderRequest {
        prompt: "A blue ceramic cup on a plain wooden table, soft daylight, still life photograph"
            .into(),
        seed: 42,
        cancel: render_run.token(),
    };
    let start = std::time::Instant::now();
    let result = registry
        .render("krea2", &request)
        .expect("real Krea render");
    assert_eq!((result.width, result.height, result.seed), (624, 832, 42));
    fs::copy(&result.path, evidence.join("image.png")).unwrap();
    let render_seconds = start.elapsed().as_secs_f64();
    let bytes = result.bytes;
    drop(result);
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
    // A live, unrelated process must survive the adapter's process-group cleanup.
    let mut sentinel = Sentinel(Command::new("/bin/sleep").arg("3600").spawn().unwrap());
    let sentinel_pid = sentinel.0.id();
    let cancel = run.token();
    let thread = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(10));
        let mut owned = descendants();
        owned.remove(&sentinel_pid);
        // Exclude the ps process used to take this snapshot (already reaped).
        owned.retain(|pid| unsafe { libc::kill(*pid as i32, 0) == 0 });
        run.request_cancel();
        owned
    });
    let start = std::time::Instant::now();
    let cancelled = registry
        .render("krea2", &RenderRequest { cancel, ..request })
        .err();
    let owned = thread.join().unwrap();
    let sentinel_survived = sentinel.0.try_wait().unwrap().is_none();
    drop(sentinel);
    assert_eq!(cancelled, Some(EngineError::Cancelled));
    assert!(
        owned.len() >= 2,
        "wrapper and real renderer must be in flight"
    );
    // Group termination is asynchronous for grandchildren; give launchd a
    // bounded opportunity to reap exited orphans before checking PID absence.
    let cleanup_deadline = std::time::Instant::now() + Duration::from_secs(5);
    while owned
        .iter()
        .any(|pid| unsafe { libc::kill(*pid as i32, 0) == 0 })
        && std::time::Instant::now() < cleanup_deadline
    {
        std::thread::sleep(Duration::from_millis(50));
    }
    for pid in &owned {
        assert_eq!(
            unsafe { libc::kill(*pid as i32, 0) },
            -1,
            "owned child survived: {pid}"
        );
    }
    assert!(sentinel_survived);
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
    fs::write(
        evidence.join("adapter.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "render_seconds": render_seconds, "bytes": bytes, "width": 624, "height": 832,
            "seed": 42, "cancel_seconds": start.elapsed().as_secs_f64(),
            "cancelled_processes": owned, "sentinel_survived": sentinel_survived,
            "staging_empty_after_success_and_cancel": true,
            "png_validation": "production CRC, bounds and ImageIO pixel decode"
        }))
        .unwrap(),
    )
    .unwrap();
    println!("image engine live: render, cancel, process cleanup and sentinel PASS");
}

use super::*;
use std::time::Instant;

#[test]
fn a_burst_of_writes_is_one_signal_without_paths() {
    let dir = tempfile::tempdir().unwrap();
    let tree = dir.path().canonicalize().unwrap();
    let w = watch(&tree, Duration::from_millis(300)).unwrap();
    std::thread::sleep(Duration::from_millis(300));
    for i in 0..5 {
        std::fs::write(tree.join(format!("{i}.md")), b"x").unwrap();
    }
    w.signals
        .recv_timeout(Duration::from_secs(5))
        .expect("one signal");
    let started = Instant::now();
    assert!(
        w.signals.recv_timeout(Duration::from_millis(900)).is_err(),
        "burst must coalesce into a single signal (waited {:?})",
        started.elapsed()
    );
}

#[test]
fn a_quiet_tree_sends_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let w = watch(dir.path(), Duration::from_millis(50)).unwrap();
    assert!(w.signals.recv_timeout(Duration::from_millis(400)).is_err());
}

#[test]
fn errors_and_changes_invalidate_but_plain_reads_do_not() {
    use notify::event::{AccessKind, CreateKind};
    let change = Event::new(EventKind::Create(CreateKind::File));
    let read = Event::new(EventKind::Access(AccessKind::Read));
    assert!(invalidates(&Ok(change)));
    assert!(!invalidates(&Ok(read)));
    assert!(invalidates(&Err(notify::Error::generic("overflow"))));
}

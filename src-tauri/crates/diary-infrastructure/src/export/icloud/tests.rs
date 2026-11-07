use super::*;
use std::sync::atomic::AtomicBool;

fn d() -> DiaryDate {
    DiaryDate::parse("2026-11-05").unwrap()
}
fn adapter() -> (tempfile::TempDir, Icloud) {
    let t = tempfile::tempdir().unwrap();
    let a = Icloud::new(t.path()).unwrap();
    (t, a)
}
fn put(a: &Icloud, text: &str) -> PathBuf {
    let p = a.path(&d());
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(&p, text).unwrap();
    p
}

// T01: exact bytes under YYYY/MM, directories created on first write.
#[test]
fn creates_exact_bytes_under_year_and_month() {
    let (t, a) = adapter();
    let seen = a.observe(&d());
    assert!(seen.available && !seen.present);
    assert_eq!(
        a.apply(&d(), b"hello\n", Write::Create, &seen),
        Applied::Done
    );
    let p = t
        .path()
        .canonicalize()
        .unwrap()
        .join("2026/11/2026-11-05.md");
    assert_eq!(std::fs::read(&p).unwrap(), b"hello\n");
    let back = a.read_back(&d());
    assert!(back.present && back.digest == hex_sha(b"hello\n"));
    assert_eq!(back.ids, "2026/11/2026-11-05.md");
    assert!(!Icloud::stage(&p).exists());
}

// T01: a symlinked year directory is refused, never followed.
#[test]
fn symlinked_parents_are_rejected() {
    let (t, a) = adapter();
    let outside = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(outside.path(), t.path().join("2026")).unwrap();
    let seen = a.observe(&d());
    assert_eq!(seen.unavailable, Some(Unavailable::Unreadable));
    assert_eq!(
        a.apply(&d(), b"x", Write::Create, &Observation::absent()),
        Applied::Failed
    );
    assert!(std::fs::read_dir(outside.path()).unwrap().next().is_none());
}

// T02: create never clobbers; replace needs the authorized fingerprint.
#[test]
fn create_is_no_clobber_and_replace_is_guarded() {
    let (_t, a) = adapter();
    let p = put(&a, "occupant");
    assert_eq!(
        a.apply(&d(), b"new", Write::Create, &Observation::absent()),
        Applied::Failed
    );
    assert_eq!(std::fs::read_to_string(&p).unwrap(), "occupant");
    let seen = a.observe(&d());
    std::fs::write(&p, "changed after preview").unwrap();
    assert_eq!(
        a.apply(&d(), b"new", Write::Replace, &seen),
        Applied::Failed
    );
    assert_eq!(
        std::fs::read_to_string(&p).unwrap(),
        "changed after preview"
    );
    let seen = a.observe(&d());
    assert_eq!(a.apply(&d(), b"new", Write::Replace, &seen), Applied::Done);
    assert_eq!(std::fs::read_to_string(&p).unwrap(), "new");
    assert!(!Icloud::stage(&p).exists());
}

// T02: the exclusive rename itself refuses an existing target.
#[test]
fn exclusive_rename_refuses_existing_target() {
    let t = tempfile::tempdir().unwrap();
    let (a, b) = (t.path().join("a"), t.path().join("b"));
    std::fs::write(&a, "a").unwrap();
    std::fs::write(&b, "b").unwrap();
    assert!(!rename_excl(&a, &b));
    assert_eq!(std::fs::read_to_string(&b).unwrap(), "b");
}

// T03: provider states are unavailability, never absence.
#[test]
fn download_needed_is_unavailable_not_absent() {
    let (_t, a) = adapter();
    put(&a, "x");
    let a = a.with(
        Arc::new(|_| Provider::NotDownloaded),
        OBSERVE_TIMEOUT,
        APPLY_TIMEOUT,
    );
    let seen = a.observe(&d());
    assert!(!seen.available && !seen.present);
    assert_eq!(seen.unavailable, Some(Unavailable::DownloadNeeded));
}

#[test]
fn legacy_placeholder_means_download_needed() {
    let (_t, a) = adapter();
    let p = a.path(&d());
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p.with_file_name(".2026-11-05.md.icloud"), "").unwrap();
    assert_eq!(
        a.observe(&d()).unavailable,
        Some(Unavailable::DownloadNeeded)
    );
}

// C7: a permission denial is its own reason.
#[test]
fn permission_denied_is_distinct() {
    use std::os::unix::fs::PermissionsExt;
    let (_t, a) = adapter();
    let p = put(&a, "x");
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o000)).unwrap();
    let seen = a.observe(&d());
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(seen.unavailable, Some(Unavailable::PermissionDenied));
}

// T04 / C8: a late accessor never writes; a crash leftover is cleaned at the next observe.
#[test]
fn late_accessor_never_writes_and_leftovers_are_cleaned() {
    let (_t, a) = adapter();
    let p = a.path(&d());
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    let flag = AtomicBool::new(true);
    assert_eq!(
        a.publish(&p, b"x", Write::Create, &Observation::absent(), &flag),
        Applied::Failed
    );
    assert!(!p.exists());
    std::fs::write(Icloud::stage(&p), "leftover").unwrap();
    a.observe(&d());
    assert!(!Icloud::stage(&p).exists());
}

#[test]
fn a_missing_root_registers_nothing() {
    assert!(Icloud::new(Path::new("/definitely/not/here")).is_none());
}

#[test]
fn staging_symlink_never_truncates_another_file() {
    let (_t, a) = adapter();
    let p = put(&a, "original");
    let outside = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(outside.path(), "private data").unwrap();
    std::os::unix::fs::symlink(outside.path(), Icloud::stage(&p)).unwrap();
    let expected = a.look_file(&p);
    assert_eq!(
        a.apply(&d(), b"new", Write::Replace, &expected),
        Applied::Failed
    );
    assert_eq!(std::fs::read(outside.path()).unwrap(), b"private data");
    assert_eq!(std::fs::read(&p).unwrap(), b"original");
}

#[test]
fn target_symlinks_are_unreadable_even_when_their_contents_match() {
    let (_t, a) = adapter();
    let p = put(&a, "original");
    let outside = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(outside.path(), "original").unwrap();
    let expected = a.observe(&d());
    std::fs::remove_file(&p).unwrap();
    std::os::unix::fs::symlink(outside.path(), &p).unwrap();
    assert_eq!(a.observe(&d()).unavailable, Some(Unavailable::Unreadable));
    assert_eq!(
        a.apply(&d(), b"new", Write::Replace, &expected),
        Applied::Failed
    );
    assert_eq!(std::fs::read(outside.path()).unwrap(), b"original");
}

#[test]
fn oversized_remote_files_are_unreadable() {
    let (_t, a) = adapter();
    let p = put(&a, "");
    std::fs::OpenOptions::new()
        .write(true)
        .open(p)
        .unwrap()
        .set_len(MAX_SNAPSHOT_BYTES as u64 + 1)
        .unwrap();
    assert_eq!(a.observe(&d()).unavailable, Some(Unavailable::Unreadable));
}

#[test]
fn observation_does_not_clean_up_a_stage_while_publication_holds_access() {
    let (_t, a) = adapter();
    let p = put(&a, "old");
    let a = a.with(
        Arc::new(system_probe),
        Duration::from_millis(20),
        APPLY_TIMEOUT,
    );
    let guard = a.access.lock().unwrap();
    std::fs::write(Icloud::stage(&p), "pending publication").unwrap();
    assert_eq!(
        a.observe(&d()).unavailable,
        Some(Unavailable::CoordinationFailed)
    );
    assert!(Icloud::stage(&p).exists());
    drop(guard);
    // The expired worker must not clean up after it eventually acquires access.
    drop(a.access.lock().unwrap());
    assert!(Icloud::stage(&p).exists());
}

#[test]
fn timeout_or_drift_during_staging_prevents_publication() {
    let (_t, a) = adapter();
    let p = put(&a, "original");
    let before = a.look_file(&p);
    let stage = Icloud::stage(&p);
    std::fs::write(&stage, "new").unwrap();
    assert_eq!(
        a.publish_staged(&p, &stage, Write::Replace, &before, &AtomicBool::new(true)),
        Applied::Failed
    );
    assert_eq!(std::fs::read(&p).unwrap(), b"original");
    assert!(!stage.exists());

    std::fs::write(&stage, "new").unwrap();
    std::fs::write(&p, "external edit during staging").unwrap();
    assert_eq!(
        a.publish_staged(&p, &stage, Write::Replace, &before, &AtomicBool::new(false)),
        Applied::Failed
    );
    assert_eq!(std::fs::read(&p).unwrap(), b"external edit during staging");
    assert!(!stage.exists());
}

#[test]
fn timed_out_workers_bound_subsequent_native_calls() {
    let active = Arc::new(AtomicBool::new(false));
    let (release, blocked) = mpsc::channel();
    let (finished, done) = mpsc::channel();
    assert_eq!(
        timed(Duration::from_millis(20), active.clone(), move |_| {
            blocked.recv().unwrap();
            finished.send(()).unwrap();
            1
        }),
        None
    );
    assert!(active.load(Ordering::SeqCst));
    assert_eq!(
        timed(Duration::from_secs(1), active.clone(), |_| panic!(
            "second worker started"
        )),
        None::<()>
    );
    release.send(()).unwrap();
    done.recv_timeout(Duration::from_secs(1)).unwrap();
}

use super::*;
use crate::{lifecycle::WorkspaceLock, workspace::Workspace};
use diary_application::{
    authentic::{ConflictKind, CreateRequest, SaveRequest},
    operations::{NewOperation, OperationState, ResourceKey},
};
use diary_domain::authentic::{AuthenticHeader, render};
use std::fs;

struct Env {
    _temp: tempfile::TempDir,
    root: PathBuf,
}
fn env() -> Env {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap().join("ws");
    fs::create_dir(&root).unwrap();
    Env { _temp: temp, root }
}
struct Open {
    store: Arc<Store>,
    ws: Workspace,
    _lock: WorkspaceLock,
}
fn open(root: &Path) -> Open {
    let lock = WorkspaceLock::acquire(root).unwrap();
    let ws = Workspace::open(&lock).unwrap();
    let (store, _) = ws.open_authentic().unwrap().unwrap();
    Open {
        store,
        ws,
        _lock: lock,
    }
}
fn op(n: u8) -> OperationId {
    OperationId::parse(&format!("00000000-0000-4000-8000-0000000000{n:02x}")).unwrap()
}
fn d(text: &str) -> DiaryDate {
    DiaryDate::parse(text).unwrap()
}
fn save(
    op: OperationId,
    date: &str,
    expected: Option<&AuthenticRevision>,
    body: &str,
) -> SaveRequest {
    SaveRequest {
        operation_id: op,
        date: d(date),
        expected: expected.cloned(),
        body: body.into(),
        buffer_generation: 7,
    }
}
fn create(op: OperationId, date: &str) -> CreateRequest {
    CreateRequest {
        operation_id: op,
        date: d(date),
    }
}
fn path(root: &Path, date: &str) -> PathBuf {
    root.join(format!(
        "assets/diary/authentic/{}/{}/{date}.md",
        &date[..4],
        &date[5..7]
    ))
}
fn bytes(root: &Path, date: &str) -> Vec<u8> {
    fs::read(path(root, date)).unwrap()
}
fn names(dir: &Path) -> Vec<String> {
    fs::read_dir(dir)
        .map(|e| {
            e.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default()
}
fn clean(o: &Open, date: &str) {
    assert_eq!(
        names(&o.store.pending()),
        Vec::<String>::new(),
        "stages left"
    );
    let month = path(&o.store.root, date).parent().unwrap().to_path_buf();
    assert!(
        names(&month).iter().all(|n| !n.ends_with(".tmp")),
        "temp left"
    );
    assert!(o.ws.ledger().unwrap().unfinished().unwrap().is_empty());
}

const POINTS: [Step; 9] = [
    Step::StageWrite,
    Step::StageSync,
    Step::Intent,
    Step::Publish,
    Step::FileSync,
    Step::DirSync,
    Step::Verify,
    Step::Receipt,
    Step::Ack,
];

#[test]
fn create_empty_makes_revision_one_with_an_empty_body_and_survives_restart() {
    let e = env();
    let o = open(&e.root);
    let out = o.store.create_empty(create(op(1), "2026-11-05")).unwrap();
    assert_eq!(out.revision.sequence, 1);
    let file = bytes(&e.root, "2026-11-05");
    let (header, body) = parse(&file, &d("2026-11-05")).unwrap();
    assert_eq!((header.revision, body.as_str()), (1, ""));
    assert_eq!(out.revision.sha256, sha_hex(&file));
    assert!(!header.timezone.is_empty() && !header.timezone_database.is_empty());
    drop(o);
    let o = open(&e.root);
    let page = o.store.list(None, 10).unwrap();
    assert_eq!(page.rows.len(), 1);
    assert_eq!(page.rows[0].revision.as_ref(), Some(&out.revision));
    assert!(matches!(
        o.store.load(&d("2026-11-05")),
        Ok(Loaded::File { .. })
    ));
    assert!(matches!(
        o.store.load(&d("2026-11-06")),
        Ok(Loaded::Virtual(_))
    ));
    clean(&o, "2026-11-05");
}

#[test]
fn create_empty_never_clobbers_and_a_repeated_id_returns_the_recorded_outcome() {
    let e = env();
    let o = open(&e.root);
    let first = o.store.create_empty(create(op(1), "2026-11-05")).unwrap();
    let before = bytes(&e.root, "2026-11-05");
    assert_eq!(
        o.store.create_empty(create(op(2), "2026-11-05")),
        Err(StoreError::Conflict(ConflictKind::Exists))
    );
    assert_eq!(bytes(&e.root, "2026-11-05"), before);
    assert_eq!(o.store.create_empty(create(op(1), "2026-11-05")), Ok(first));
    assert_eq!(bytes(&e.root, "2026-11-05"), before);
}

#[test]
fn saves_are_guarded_by_the_exact_revision() {
    let e = env();
    let o = open(&e.root);
    let one = o
        .store
        .save(save(op(1), "2026-11-05", None, "one"))
        .unwrap();
    assert_eq!(
        o.store.save(save(op(2), "2026-11-05", None, "again")),
        Err(StoreError::Conflict(ConflictKind::Exists))
    );
    let two = o
        .store
        .save(save(op(3), "2026-11-05", Some(&one.revision), "two"))
        .unwrap();
    assert_eq!((two.revision.sequence, two.buffer_generation), (2, 7));
    let latest = bytes(&e.root, "2026-11-05");
    assert_eq!(
        o.store
            .save(save(op(4), "2026-11-05", Some(&one.revision), "stale")),
        Err(StoreError::Conflict(ConflictKind::Stale))
    );
    // ABA: same sequence, different bytes.
    let wrong = AuthenticRevision {
        sequence: 2,
        sha256: sha_hex(b"other"),
    };
    assert_eq!(
        o.store.save(save(op(5), "2026-11-05", Some(&wrong), "x")),
        Err(StoreError::Conflict(ConflictKind::Stale))
    );
    assert_eq!(
        o.store
            .save(save(op(6), "2026-11-06", Some(&one.revision), "x")),
        Err(StoreError::Conflict(ConflictKind::Stale))
    );
    assert_eq!(bytes(&e.root, "2026-11-05"), latest);
    assert!(!path(&e.root, "2026-11-06").exists());
    // Same ID with different input is a mismatch, and emptying a body keeps the file.
    assert_eq!(
        o.store
            .save(save(op(3), "2026-11-05", Some(&one.revision), "different")),
        Err(StoreError::Mismatch)
    );
    let empty = o
        .store
        .save(save(op(7), "2026-11-05", Some(&two.revision), ""))
        .unwrap();
    assert_eq!(empty.revision.sequence, 3);
    assert!(
        matches!(o.store.load(&d("2026-11-05")), Ok(Loaded::File { body, .. }) if body.is_empty())
    );
}

#[test]
fn a_clock_regression_never_lowers_the_revision() {
    let e = env();
    let o = open(&e.root);
    o.store
        .clock
        .store(2_000_000_000, std::sync::atomic::Ordering::SeqCst);
    let one = o.store.save(save(op(1), "2026-11-05", None, "a")).unwrap();
    o.store
        .clock
        .store(1_000_000_000, std::sync::atomic::Ordering::SeqCst);
    let two = o
        .store
        .save(save(op(2), "2026-11-05", Some(&one.revision), "b"))
        .unwrap();
    assert_eq!(two.revision.sequence, 2);
}

#[test]
fn bodies_are_capped_at_one_mebibyte_without_side_effects() {
    let e = env();
    let o = open(&e.root);
    let big = "a".repeat(MAX_BODY_BYTES);
    assert!(o.store.save(save(op(1), "2026-11-05", None, &big)).is_ok());
    let rev = match o.store.load(&d("2026-11-05")).unwrap() {
        Loaded::File { revision, .. } => revision,
        _ => unreachable!(),
    };
    let before = bytes(&e.root, "2026-11-05");
    let r = o
        .store
        .save(save(op(2), "2026-11-05", Some(&rev), &format!("{big}a")));
    assert!(matches!(r, Err(StoreError::Format(_))));
    assert_eq!(bytes(&e.root, "2026-11-05"), before);
    assert!(o.store.operation(&op(2)).unwrap().is_none());
    clean(&o, "2026-11-05");
}

fn crash_case(create_mode: bool, step: Step, when: When) {
    let label = format!("create={create_mode} {step:?} {when:?}");
    let e = env();
    let date = "2026-11-05";
    let o = open(&e.root);
    let base = (!create_mode).then(|| o.store.save(save(op(1), date, None, "one")).unwrap());
    o.store.inject(Fault {
        step,
        when,
        kind: FaultKind::Crash,
    });
    let result = match &base {
        None => o.store.create_empty(create(op(2), date)),
        Some(b) => o.store.save(save(op(2), date, Some(&b.revision), "two")),
    };
    assert!(result.is_err(), "{label}: a crash cannot acknowledge");
    drop(o);

    let o = open(&e.root); // restart runs recovery
    if let Ok(file) = fs::read(path(&e.root, date)) {
        assert!(
            parse(&file, &d(date)).is_ok(),
            "{label}: file must never be partial"
        );
    }
    let retried = match &base {
        None => o.store.create_empty(create(op(2), date)),
        Some(b) => o.store.save(save(op(2), date, Some(&b.revision), "two")),
    }
    .unwrap_or_else(|e| panic!("{label}: retry of the original ID failed: {e:?}"));
    let (want_seq, want_body) = if create_mode { (1, "") } else { (2, "two") };
    assert_eq!(retried.revision.sequence, want_seq, "{label}");
    let (header, body) = parse(&bytes(&e.root, date), &d(date)).unwrap();
    assert_eq!(
        (header.revision, body.as_str()),
        (want_seq, want_body),
        "{label}"
    );
    assert_eq!(
        o.store.operation(&op(2)).unwrap().unwrap().state,
        OperationState::Succeeded,
        "{label}"
    );
    drop(o);
    let o = open(&e.root);
    clean(&o, date);
}

#[test]
fn every_crash_boundary_reconciles_to_one_complete_file() {
    for create_mode in [false, true] {
        for step in POINTS {
            for when in [When::Before, When::After] {
                crash_case(create_mode, step, when);
            }
        }
    }
}

#[test]
fn failing_system_calls_fail_cleanly_before_publication_and_are_uncertain_after() {
    for step in POINTS {
        for when in [When::Before, When::After] {
            let label = format!("{step:?} {when:?}");
            let e = env();
            let date = "2026-11-05";
            let o = open(&e.root);
            let one = o.store.save(save(op(1), date, None, "one")).unwrap();
            let old = bytes(&e.root, date);
            o.store.inject(Fault {
                step,
                when,
                kind: FaultKind::Io,
            });
            let r = o.store.save(save(op(2), date, Some(&one.revision), "two"));
            assert!(r.is_err(), "{label}");
            let published = matches!(
                (step, when),
                (Step::Publish, When::After)
                    | (
                        Step::FileSync | Step::DirSync | Step::Verify | Step::Receipt | Step::Ack,
                        _
                    )
            );
            if published {
                assert_eq!(r, Err(StoreError::Uncertain), "{label}");
                assert_eq!(
                    o.store
                        .save(save(op(2), date, Some(&one.revision), "two"))
                        .unwrap()
                        .revision
                        .sequence,
                    2,
                    "{label}"
                );
            } else {
                assert_eq!(bytes(&e.root, date), old, "{label}: old file must survive");
                assert!(
                    o.ws.ledger().unwrap().unfinished().unwrap().is_empty(),
                    "{label}"
                );
                assert_eq!(
                    o.store
                        .save(save(op(3), date, Some(&one.revision), "two"))
                        .unwrap()
                        .revision
                        .sequence,
                    2,
                    "{label}"
                );
            }
            drop(o);
            clean(&open(&e.root), date);
        }
    }
}

fn crash_after_intent(e: &Env) -> AuthenticRevision {
    let o = open(&e.root);
    let one = o
        .store
        .save(save(op(1), "2026-11-05", None, "one"))
        .unwrap();
    o.store.inject(Fault {
        step: Step::Intent,
        when: When::After,
        kind: FaultKind::Crash,
    });
    assert!(
        o.store
            .save(save(op(2), "2026-11-05", Some(&one.revision), "two"))
            .is_err()
    );
    one.revision
}

#[test]
fn a_missing_or_corrupt_stage_with_an_unchanged_file_is_a_failed_save() {
    for corrupt in [false, true] {
        let e = env();
        let one = crash_after_intent(&e);
        let stage = e.root.join(format!(".diary/pending/{}.md", op(2).as_str()));
        if corrupt {
            fs::write(&stage, b"garbage").unwrap();
        } else {
            fs::remove_file(&stage).unwrap();
        }
        let old = bytes(&e.root, "2026-11-05");
        let o = open(&e.root);
        assert_eq!(bytes(&e.root, "2026-11-05"), old);
        assert_eq!(
            o.store.save(save(op(2), "2026-11-05", Some(&one), "two")),
            Err(StoreError::Failed("stage_missing".into()))
        );
        assert!(o.ws.ledger().unwrap().unfinished().unwrap().is_empty());
    }
}

#[test]
fn an_external_edit_during_a_prepared_save_is_preserved_never_restored_over() {
    let e = env();
    let one = crash_after_intent(&e);
    let external = b"someone else's edit".to_vec();
    fs::write(path(&e.root, "2026-11-05"), &external).unwrap();
    let o = open(&e.root);
    assert_eq!(bytes(&e.root, "2026-11-05"), external);
    let stage = e.root.join(format!(".diary/pending/{}.md", op(2).as_str()));
    assert!(
        fs::read(&stage).unwrap().ends_with(b"two"),
        "staged content preserved"
    );
    assert_eq!(
        o.store.save(save(op(2), "2026-11-05", Some(&one), "two")),
        Err(StoreError::Conflict(ConflictKind::External))
    );
    assert!(o.ws.ledger().unwrap().unfinished().unwrap().is_empty());
    // The corrupt file blocks saving that date with a readable diagnostic.
    assert!(matches!(
        o.store.save(save(op(3), "2026-11-05", Some(&one), "x")),
        Err(StoreError::Blocked(_))
    ));
}

#[test]
fn startup_removes_only_app_owned_orphans() {
    let e = env();
    let o = open(&e.root);
    o.store
        .save(save(op(1), "2026-11-05", None, "one"))
        .unwrap();
    drop(o);
    let month = path(&e.root, "2026-11-05").parent().unwrap().to_path_buf();
    let stray = e
        .root
        .join(".diary/pending")
        .join(format!("{}.md", op(9).as_str()));
    fs::write(&stray, b"orphan").unwrap();
    let tmp = month.join(format!(".2026-11-05.md.{}.tmp", op(9).as_str()));
    fs::write(&tmp, b"orphan").unwrap();
    let user = month.join("notes.md");
    fs::write(&user, b"mine").unwrap();
    let other = path(&e.root, "2026-11-04");
    fs::write(&other, b"not a diary file").unwrap();
    let o = open(&e.root);
    assert!(!stray.exists() && !tmp.exists());
    assert_eq!(fs::read(&user).unwrap(), b"mine");
    assert_eq!(fs::read(&other).unwrap(), b"not a diary file");
    clean(&o, "2026-11-05");
}

#[test]
fn a_quarantined_operation_blocks_only_its_own_date() {
    let e = env();
    let o = open(&e.root);
    let held = NewOperation {
        id: op(50),
        kind: "other_kind".into(),
        hash_version: 1,
        input_hash: sha_hex(b"x"),
        resources: vec![ResourceKey::parse("authentic:2026-11-04").unwrap()],
        identity: o.ws.identity.clone(),
        first_step: None,
    };
    o.ws.ledger().unwrap().admit(held).unwrap();
    drop(o);
    let o = open(&e.root); // classified NoReconciler, stays unfinished
    assert_eq!(o.ws.ledger().unwrap().unfinished().unwrap().len(), 1);
    assert_eq!(
        o.store.save(save(op(1), "2026-11-04", None, "x")),
        Err(StoreError::Reserved)
    );
    assert!(o.store.save(save(op(2), "2026-11-05", None, "x")).is_ok());
}

fn file_with_id(date: &str, id: &str) -> Vec<u8> {
    render(
        &AuthenticHeader {
            id: id.into(),
            date: d(date),
            timezone: "UTC".into(),
            timezone_database: "2026e".into(),
            created_at: "2026-11-05T10:00:00Z".into(),
            updated_at: "2026-11-05T10:00:00Z".into(),
            revision: 1,
            extensions: vec![],
        },
        "x",
    )
    .unwrap()
}

#[test]
fn rebuild_restores_the_catalog_and_flags_duplicates_without_touching_operations() {
    let e = env();
    let o = open(&e.root);
    for (n, date) in [(1, "2026-11-03"), (2, "2026-11-04"), (3, "2026-11-05")] {
        o.store.save(save(op(n), date, None, "x")).unwrap();
    }
    let operations = |o: &Open| o.ws.ledger().unwrap().get(&op(2)).unwrap();
    let before = operations(&o);
    o.store
        .db
        .with(|c| Ok(c.execute("DELETE FROM catalog_authentic", []).unwrap()))
        .unwrap();
    assert!(o.store.list(None, 10).unwrap().rows.is_empty());
    let report = o.store.refresh().unwrap();
    assert_eq!((report.files, report.problems), (3, 0));
    assert_eq!(operations(&o), before);

    // Two dates claiming one ID, and a file whose header date disagrees with its path.
    let id = "11111111-1111-4111-8111-111111111111";
    fs::create_dir_all(path(&e.root, "2026-12-01").parent().unwrap()).unwrap();
    fs::write(path(&e.root, "2026-12-01"), file_with_id("2026-12-01", id)).unwrap();
    fs::write(path(&e.root, "2026-12-02"), file_with_id("2026-12-02", id)).unwrap();
    fs::write(path(&e.root, "2026-12-03"), file_with_id("2026-12-09", id)).unwrap();
    let report = o.store.refresh().unwrap();
    assert_eq!((report.files, report.problems), (6, 3));
    let rows = o.store.list(None, 10).unwrap().rows;
    assert!(rows.iter().filter(|r| r.problem.is_some()).count() == 3);
    assert!(matches!(
        o.store.save(save(op(9), "2026-12-01", None, "x")),
        Err(StoreError::Conflict(ConflictKind::Exists))
    ));
}

#[test]
fn listing_is_bounded_and_cursor_stable() {
    let e = env();
    let o = open(&e.root);
    for (n, date) in [
        (1, "2026-01-01"),
        (2, "2026-01-02"),
        (3, "2026-01-03"),
        (4, "2026-02-01"),
        (5, "2025-12-31"),
    ] {
        o.store.save(save(op(n), date, None, "x")).unwrap();
    }
    let first = o.store.list(None, 2).unwrap();
    assert_eq!(
        first
            .rows
            .iter()
            .map(|r| r.date.to_string())
            .collect::<Vec<_>>(),
        ["2025-12-31", "2026-01-01"]
    );
    // An insert before the cursor does not shift later pages.
    o.store.save(save(op(6), "2025-01-01", None, "x")).unwrap();
    let second = o.store.list(first.next, 2).unwrap();
    assert_eq!(
        second
            .rows
            .iter()
            .map(|r| r.date.to_string())
            .collect::<Vec<_>>(),
        ["2026-01-02", "2026-01-03"]
    );
    let third = o.store.list(second.next, 2).unwrap();
    assert_eq!((third.rows.len(), third.next), (1, None));
    assert_eq!(o.store.list(None, 10_000).unwrap().rows.len(), 6);
}

#[test]
fn rescans_see_added_edited_and_deleted_files() {
    let e = env();
    let o = open(&e.root);
    let one = o
        .store
        .save(save(op(1), "2026-11-05", None, "one"))
        .unwrap();
    fs::create_dir_all(path(&e.root, "2026-11-06").parent().unwrap()).unwrap();
    fs::write(
        path(&e.root, "2026-11-06"),
        file_with_id("2026-11-06", "22222222-2222-4222-8222-222222222222"),
    )
    .unwrap();
    assert_eq!(o.store.refresh().unwrap().files, 2);
    // An external edit that keeps the sequence but changes the bytes is a different revision.
    let mut edited = bytes(&e.root, "2026-11-05");
    edited.extend_from_slice(b"!");
    fs::write(path(&e.root, "2026-11-05"), &edited).unwrap();
    let Ok(Loaded::File { revision, .. }) = o.store.load(&d("2026-11-05")) else {
        panic!()
    };
    assert_eq!(revision.sequence, one.revision.sequence);
    assert_ne!(revision.sha256, one.revision.sha256);
    assert_eq!(
        o.store
            .save(save(op(2), "2026-11-05", Some(&one.revision), "mine")),
        Err(StoreError::Conflict(ConflictKind::Stale))
    );
    fs::remove_file(path(&e.root, "2026-11-06")).unwrap();
    assert_eq!(o.store.refresh().unwrap().files, 1);
}

#[test]
fn recovery_mode_keeps_files_readable_and_refuses_every_mutation() {
    let e = env();
    let o = open(&e.root);
    o.store
        .save(save(op(1), "2026-11-05", None, "kept"))
        .unwrap();
    drop(o);
    let db = e.root.join(".diary/state.sqlite3");
    fs::remove_file(&db).unwrap();
    let lock = WorkspaceLock::acquire(&e.root).unwrap();
    let ws = Workspace::open(&lock).unwrap();
    assert!(
        ws.open_authentic().is_none(),
        "no ledger, no writable store"
    );
    let ro = ws.read_only_authentic().unwrap();
    assert!(matches!(ro.load(&d("2026-11-05")), Ok(Loaded::File { body, .. }) if body == "kept"));
    assert_eq!(ro.list(None, 10).unwrap().rows.len(), 1);
    assert_eq!(
        ro.save(save(op(2), "2026-11-05", None, "x")),
        Err(StoreError::Unavailable)
    );
    assert_eq!(
        ro.create_empty(create(op(3), "2026-11-06")),
        Err(StoreError::Unavailable)
    );
    assert!(!db.exists(), "no empty ledger is created");
}

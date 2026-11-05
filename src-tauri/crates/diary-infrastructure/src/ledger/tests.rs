use super::*;
use crate::sqlite::DB_FILE;
use diary_application::operations::{StepId, classify};
use diary_logging::private_fs::PrivateDir;
use std::{os::unix::fs::PermissionsExt, path::PathBuf, sync::Arc};

struct Fixture {
    _temp: tempfile::TempDir,
    dir: PrivateDir,
    path: PathBuf,
    db: Arc<Db>,
}
fn fixture() -> Fixture {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().canonicalize().unwrap().join(".diary");
    std::fs::create_dir(&path).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    let dir = PrivateDir::open(&path).unwrap();
    let db = Arc::new(Db::create(&dir, &path.join(DB_FILE)).unwrap());
    Fixture {
        _temp: temp,
        dir,
        path,
        db,
    }
}
impl Fixture {
    fn reopen(self) -> Self {
        let Self {
            _temp,
            dir,
            path,
            db,
        } = self;
        drop(db);
        let db = Arc::new(Db::open_existing(&dir, &path.join(DB_FILE), false).unwrap());
        Self {
            _temp,
            dir,
            path,
            db,
        }
    }
}

fn uuid(n: u8) -> String {
    format!("07249746-e280-4d51-a86a-cbcb198ee1{n:02x}")
}
fn identity() -> Identity {
    Identity {
        workspace_id: "w".into(),
        root: "/r".into(),
        native: "1:2".into(),
        adapter_version: 1,
    }
}
fn step(n: u8, checkpoint: u64, state: Option<OperationState>) -> NewStep {
    NewStep {
        step_id: StepId::parse(&uuid(n)).unwrap(),
        input_hash: hash_input("step", 1, &[("n", &n.to_string())]),
        expected_checkpoint: checkpoint,
        outcome: format!("outcome {n}"),
        state,
    }
}
fn op(n: u8, keys: &[&str]) -> NewOperation {
    NewOperation {
        id: OperationId::parse(&uuid(n)).unwrap(),
        kind: "save".into(),
        hash_version: 1,
        input_hash: hash_input("save", 1, &[("n", &n.to_string())]),
        resources: keys
            .iter()
            .map(|k| ResourceKey::parse(k).unwrap())
            .collect(),
        identity: identity(),
        first_step: None,
    }
}

#[test]
fn hash_ignores_field_order_and_changes_with_every_part() {
    let base = hash_input("save", 1, &[("a", "1"), ("b", "2")]);
    assert_eq!(base, hash_input("save", 1, &[("b", "2"), ("a", "1")]));
    for other in [
        hash_input("export", 1, &[("a", "1"), ("b", "2")]),
        hash_input("save", 2, &[("a", "1"), ("b", "2")]),
        hash_input("save", 1, &[("a", "1"), ("b", "3")]),
        hash_input("save", 1, &[("a", "1"), ("c", "2")]),
        hash_input("save", 1, &[("a", "1")]),
        hash_input("save", 1, &[("a", "1b"), ("b", "2")]),
        hash_input("save", 1, &[("a", "1"), ("b2", "")]),
    ] {
        assert_ne!(base, other);
    }
}

#[test]
fn admit_new_existing_and_mismatch() {
    let f = fixture();
    let Admission::New(rec) = f.db.admit(op(1, &["authentic:2026-11-05"])).unwrap() else {
        panic!()
    };
    assert_eq!((rec.state, rec.checkpoint), (OperationState::Running, 0));
    // A retry returns the record, even though its resource is now reserved by itself.
    let again = f.db.admit(op(1, &["authentic:2026-11-05"])).unwrap();
    assert_eq!(again, Admission::Existing(rec));
    let mut changed = op(1, &["authentic:2026-11-05"]);
    changed.input_hash = hash_input("save", 1, &[("n", "other")]);
    assert_eq!(f.db.admit(changed).unwrap(), Admission::Mismatch);
    let mut versioned = op(1, &[]);
    versioned.hash_version = 2;
    assert_eq!(f.db.admit(versioned).unwrap(), Admission::Mismatch);
}

#[test]
fn invalid_admissions_are_rejected_before_any_write() {
    let f = fixture();
    let mut bad_kind = op(1, &[]);
    bad_kind.kind = "Save".into();
    let mut bad_hash = op(2, &[]);
    bad_hash.input_hash = "x".into();
    let mut many = op(3, &[]);
    many.resources = (0..65)
        .map(|i| ResourceKey::parse(&format!("k{i}")).unwrap())
        .collect();
    let mut late_step = op(4, &[]);
    late_step.first_step = Some(step(9, 1, None));
    for bad in [bad_kind, bad_hash, many, late_step] {
        assert_eq!(f.db.admit(bad), Err(LedgerError::Invalid));
    }
    assert!(f.db.unfinished().unwrap().is_empty());
}

#[test]
fn tombstone_and_unfinished_state_survive_restart() {
    let f = fixture();
    f.db.admit(op(1, &["a"])).unwrap();
    f.db.admit(op(2, &["b"])).unwrap();
    f.db.finish(
        &OperationId::parse(&uuid(1)).unwrap(),
        OperationState::Succeeded,
        "rev 3",
    )
    .unwrap();
    let f = f.reopen();
    let done =
        f.db.get(&OperationId::parse(&uuid(1)).unwrap())
            .unwrap()
            .unwrap();
    assert_eq!(
        (done.state, done.result.as_deref()),
        (OperationState::Succeeded, Some("rev 3"))
    );
    assert_eq!(f.db.unfinished().unwrap().len(), 1);
    // The finished operation still answers its retry, and does not hold "a" any longer.
    assert!(matches!(
        f.db.admit(op(1, &["a"])).unwrap(),
        Admission::Existing(_)
    ));
    assert!(matches!(
        f.db.admit(op(3, &["a"])).unwrap(),
        Admission::New(_)
    ));
    assert_eq!(
        f.db.get(&OperationId::parse(&uuid(9)).unwrap()).unwrap(),
        None
    );
}

#[test]
fn finish_is_idempotent_only_for_the_same_outcome() {
    let f = fixture();
    let id = OperationId::parse(&uuid(1)).unwrap();
    f.db.admit(op(1, &[])).unwrap();
    assert_eq!(
        f.db.finish(&id, OperationState::Paused, ""),
        Err(LedgerError::Invalid)
    );
    assert_eq!(
        f.db.finish(&id, OperationState::Failed, &"x".repeat(4097)),
        Err(LedgerError::Invalid)
    );
    f.db.finish(&id, OperationState::Failed, "disk full")
        .unwrap();
    f.db.finish(&id, OperationState::Failed, "disk full")
        .unwrap();
    assert_eq!(
        f.db.finish(&id, OperationState::Succeeded, "ok"),
        Err(LedgerError::Terminal)
    );
    let missing = OperationId::parse(&uuid(8)).unwrap();
    assert_eq!(
        f.db.finish(&missing, OperationState::Failed, ""),
        Err(LedgerError::NotFound)
    );
}

#[test]
fn steps_advance_dedupe_and_reject_stale_checkpoints() {
    let f = fixture();
    let id = OperationId::parse(&uuid(1)).unwrap();
    f.db.admit(op(1, &["a"])).unwrap();
    let StepOutcome::Applied(rec) =
        f.db.advance(&id, step(10, 0, Some(OperationState::Paused)))
            .unwrap()
    else {
        panic!()
    };
    assert_eq!((rec.checkpoint, rec.state), (1, OperationState::Paused));
    // The duplicate returns what was recorded, though the checkpoint has moved on.
    assert_eq!(
        f.db.advance(&id, step(10, 0, Some(OperationState::Paused)))
            .unwrap(),
        StepOutcome::Recorded {
            outcome: "outcome 10".into(),
            checkpoint: 1
        }
    );
    // Same step ID, different input.
    let mut other = step(10, 0, None);
    other.input_hash = hash_input("step", 1, &[("n", "different")]);
    assert_eq!(f.db.advance(&id, other), Err(LedgerError::StepMismatch));
    // A new step must name the current checkpoint.
    assert_eq!(
        f.db.advance(&id, step(11, 0, None)),
        Err(LedgerError::CheckpointMismatch { current: 1 })
    );
    f.db.advance(&id, step(11, 1, None)).unwrap();
    // Terminal states only through finish.
    assert_eq!(
        f.db.advance(&id, step(12, 2, Some(OperationState::Failed))),
        Err(LedgerError::Invalid)
    );
    f.db.finish(&id, OperationState::Cancelled, "").unwrap();
    assert_eq!(
        f.db.advance(&id, step(13, 2, None)),
        Err(LedgerError::Terminal)
    );
    assert!(
        matches!(
            f.db.advance(&id, step(10, 0, None)),
            Ok(StepOutcome::Recorded { .. })
        ),
        "duplicates answer even when terminal"
    );
    assert_eq!(
        f.db.advance(&OperationId::parse(&uuid(7)).unwrap(), step(14, 0, None)),
        Err(LedgerError::NotFound)
    );
}

#[test]
fn only_one_of_two_concurrent_steps_advances_a_checkpoint() {
    let f = fixture();
    let id = OperationId::parse(&uuid(1)).unwrap();
    f.db.admit(op(1, &[])).unwrap();
    let results: Vec<_> = (20..28u8)
        .map(|n| {
            let (db, id) = (f.db.clone(), id.clone());
            std::thread::spawn(move || db.advance(&id, step(n, 0, None)))
        })
        .collect::<Vec<_>>()
        .into_iter()
        .map(|h| h.join().unwrap())
        .collect();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert!(
        results
            .iter()
            .filter(|r| r.is_err())
            .all(|r| *r == Err(LedgerError::CheckpointMismatch { current: 1 }))
    );
}

#[test]
fn first_step_is_recorded_with_the_operation_atomically() {
    let f = fixture();
    let mut with_step = op(1, &["a"]);
    with_step.first_step = Some(step(10, 0, Some(OperationState::Paused)));
    let Admission::New(rec) = f.db.admit(with_step).unwrap() else {
        panic!()
    };
    assert_eq!((rec.checkpoint, rec.state), (1, OperationState::Paused));
    // A failure inside a transaction leaves nothing behind (admit uses the same wrapper).
    let before = f.db.unfinished().unwrap().len();
    let err =
        f.db.transact(|tx| -> Result<(), DbError> {
            tx.execute("DELETE FROM operation_resources", [])
                .map_err(|_| DbError::Storage)?;
            Err(DbError::Storage)
        })
        .err();
    assert_eq!(err, Some(DbError::Storage));
    assert_eq!(f.db.unfinished().unwrap().len(), before);
    assert!(
        matches!(f.db.admit(op(2, &["a"])), Err(LedgerError::Reserved(_))),
        "the reservation survived the rollback"
    );
}

#[test]
fn overlapping_reservations_are_refused_and_released_on_finish() {
    let f = fixture();
    f.db.admit(op(1, &["x", "y"])).unwrap();
    let holder = OperationId::parse(&uuid(1)).unwrap();
    assert_eq!(
        f.db.admit(op(2, &["z", "y"])),
        Err(LedgerError::Reserved(holder.clone()))
    );
    assert_eq!(
        f.db.get(&OperationId::parse(&uuid(2)).unwrap()).unwrap(),
        None,
        "nothing partial"
    );
    // Opposite order and disjoint keys never wait or deadlock.
    assert!(matches!(
        f.db.admit(op(3, &["w"])).unwrap(),
        Admission::New(_)
    ));
    f.db.finish(&holder, OperationState::Failed, "").unwrap();
    assert!(matches!(
        f.db.admit(op(2, &["y", "z"])).unwrap(),
        Admission::New(_)
    ));
}

#[test]
fn paused_and_uncertain_work_keeps_its_reservation_across_restart() {
    let f = fixture();
    f.db.admit(op(1, &["a"])).unwrap();
    f.db.admit(op(2, &["b"])).unwrap();
    f.db.advance(
        &OperationId::parse(&uuid(1)).unwrap(),
        step(10, 0, Some(OperationState::Paused)),
    )
    .unwrap();
    f.db.advance(
        &OperationId::parse(&uuid(2)).unwrap(),
        step(11, 0, Some(OperationState::Uncertain)),
    )
    .unwrap();
    let f = f.reopen();
    assert!(matches!(
        f.db.admit(op(3, &["a", "q"])),
        Err(LedgerError::Reserved(_))
    ));
    assert!(matches!(
        f.db.admit(op(4, &["b"])),
        Err(LedgerError::Reserved(_))
    ));
    assert!(matches!(
        f.db.admit(op(5, &["unrelated"])).unwrap(),
        Admission::New(_)
    ));
}

#[test]
fn duplicate_resource_keys_in_one_request_are_one_reservation() {
    let f = fixture();
    assert!(matches!(
        f.db.admit(op(1, &["a", "a"])).unwrap(),
        Admission::New(_)
    ));
}

#[test]
fn restart_classification_blocks_a_moved_root_without_changing_rows() {
    let f = fixture();
    f.db.admit(op(1, &["a"])).unwrap();
    let before = f.db.unfinished().unwrap();
    let mut moved = identity();
    moved.root = "/elsewhere".into();
    let classes = classify(&before, &moved, &["save"]);
    assert!(matches!(
        classes[0].class,
        diary_application::operations::Class::Blocked(_)
    ));
    assert_eq!(f.db.unfinished().unwrap(), before);
    assert!(
        matches!(f.db.admit(op(2, &["a"])), Err(LedgerError::Reserved(_))),
        "blocked work stays reserved"
    );
    let _ = (&f.dir, &f.path);
}

use super::*;
use crate::{
    config::{Source, SourceKind},
    source::{
        narihito_write::tests_support::{REL, golden},
        tests::Tree,
        trash::prepare_pair,
    },
    sqlite::{DB_FILE, MIGRATIONS, migrate},
};
use diary_application::operations::{Identity, NewStep, OperationId, OperationLedger, StepId};
use diary_logging::private_fs::PrivateDir;
use std::{fs, os::unix::fs::PermissionsExt};

struct Env {
    _temp: tempfile::TempDir,
    dir: PrivateDir,
    path: PathBuf,
    db: Db,
    plan: StoredPlan,
    op: NewOperation,
}
fn env(audio: bool) -> Env {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().canonicalize().unwrap().join(".diary");
    fs::create_dir(&path).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    let dir = PrivateDir::open(&path).unwrap();
    let db = Db::create(&dir, &path.join(DB_FILE)).unwrap();
    let tree = Tree::new();
    tree.write(REL, &golden("2024-02-29T01:02:03Z", ""));
    if audio {
        tree.write(
            &format!(
                "{}.assets/0b1d2c3e-4f50-4a6b-8c7d-9e0f1a2b3c4d.m4a",
                REL.strip_suffix(".md").unwrap()
            ),
            b"audio",
        );
    }
    let pair = prepare_pair(
        &[Source {
            alias: "test".into(),
            kind: SourceKind::Narihito,
            root: tree.temp.path().into(),
        }],
        0,
        REL,
        &|| false,
    )
    .unwrap();
    let id = source_id(&pair.root, &normalize(REL, pair.root.insensitive).unwrap());
    let plan = StoredPlan::from_pair(&id, &pair);
    let hash = plan.initial_hash().unwrap();
    let op = NewOperation {
        id: OperationId::parse("00000000-0000-4000-8000-000000000001").unwrap(),
        kind: "source_trash".into(),
        hash_version: 1,
        input_hash: hash.clone(),
        resources: plan.resources().unwrap(),
        identity: Identity {
            workspace_id: "test".into(),
            root: "/workspace".into(),
            native: "1:2".into(),
            adapter_version: 1,
        },
        first_step: Some(NewStep {
            step_id: StepId::parse("00000000-0000-4000-8000-000000000002").unwrap(),
            input_hash: hash,
            expected_checkpoint: 0,
            outcome: "admitted:1".into(),
            state: None,
        }),
    };
    Env {
        _temp: temp,
        dir,
        path,
        db,
        plan,
        op,
    }
}

#[test]
fn admission_survives_reopen_and_reserves_even_absent_audio() {
    let e = env(false);
    assert!(matches!(
        e.db.admit_trash(&e.op, &e.plan, &"a".repeat(64)),
        Ok(Admission::New(_))
    ));
    drop(e.db);
    let db = Db::open_existing(&e.dir, &e.path.join(DB_FILE), false).unwrap();
    let (plan, members) = db.trash_plan(&e.op.id).unwrap().unwrap();
    assert_eq!(plan.hash(), e.plan.hash());
    assert_eq!(members[0].status, MemberStatus::Pending);
    assert_eq!(members[1].status, MemberStatus::Missing);
    assert!(matches!(
        db.admit_trash(&e.op, &e.plan, &"b".repeat(64)),
        Ok(Admission::Existing(_))
    ));
    let mut other = e.op.clone();
    other.id = OperationId::parse("00000000-0000-4000-8000-000000000003").unwrap();
    assert_eq!(
        db.admit_trash(&other, &e.plan, &"a".repeat(64)),
        Err(StoreError::TokenUsed)
    );
    assert!(matches!(
        db.admit_trash(&other, &e.plan, &"b".repeat(64)),
        Err(StoreError::Ledger(LedgerError::Reserved(_)))
    ));
    assert!(db.get(&other.id).unwrap().is_none());
}

#[test]
fn member_insertion_failure_rolls_back_ledger_token_and_reservations() {
    let e = env(true);
    e.db.with(|c|c.execute_batch("CREATE TRIGGER fail_trash_member BEFORE INSERT ON trash_members BEGIN SELECT RAISE(ABORT,'test'); END;").map_err(|_|DbError::Storage)).unwrap();
    assert_eq!(
        e.db.admit_trash(&e.op, &e.plan, &"a".repeat(64)),
        Err(StoreError::Storage)
    );
    assert!(e.db.get(&e.op.id).unwrap().is_none());
    assert!(e.db.trash_plan(&e.op.id).unwrap().is_none());
    e.db.with(|c| {
        c.execute_batch("DROP TRIGGER fail_trash_member;")
            .map_err(|_| DbError::Storage)
    })
    .unwrap();
    assert!(matches!(
        e.db.admit_trash(&e.op, &e.plan, &"a".repeat(64)),
        Ok(Admission::New(_))
    ));
}

#[test]
fn authoritative_corruption_keeps_reservations_and_cannot_become_a_plan() {
    let e = env(true);
    e.db.admit_trash(&e.op, &e.plan, &"a".repeat(64)).unwrap();
    e.db.with(|c| {
        c.execute("DELETE FROM trash_members WHERE ordinal=1", [])
            .map_err(|_| DbError::Storage)
    })
    .unwrap();
    assert_eq!(e.db.trash_plan(&e.op.id).unwrap_err(), StoreError::Corrupt);
    assert_eq!(e.db.unfinished().unwrap().len(), 1);
    let mut other = e.op.clone();
    other.id = OperationId::parse("00000000-0000-4000-8000-000000000003").unwrap();
    assert!(matches!(
        e.db.admit_trash(&other, &e.plan, &"b".repeat(64)),
        Err(StoreError::Ledger(LedgerError::Reserved(_)))
    ));
}

#[test]
fn actual_pre_trash_database_upgrades_and_preserves_existing_rows() {
    let e = env(false);
    drop(e.db);
    fs::remove_file(e.path.join(DB_FILE)).unwrap();
    e.dir.create(DB_FILE).unwrap();
    let mut conn = rusqlite::Connection::open(e.path.join(DB_FILE)).unwrap();
    migrate(&mut conn, &MIGRATIONS[..4]).unwrap();
    conn.execute("INSERT INTO workspace_meta VALUES (1,'pre-trash')", [])
        .unwrap();
    drop(conn);
    let db = Db::open_existing(&e.dir, &e.path.join(DB_FILE), false).unwrap();
    assert!(db.unfinished().unwrap().is_empty());
    assert!(matches!(
        db.admit_trash(&e.op, &e.plan, &"a".repeat(64)),
        Ok(Admission::New(_))
    ));
    db.with(|c| {
        let id: String = c
            .query_row("SELECT workspace_id FROM workspace_meta", [], |r| r.get(0))
            .map_err(|_| DbError::Storage)?;
        assert_eq!(id, "pre-trash");
        Ok(())
    })
    .unwrap();
}

#[test]
fn simultaneous_token_reuse_admits_only_one_operation() {
    let e = env(true);
    let mut other = e.op.clone();
    other.id = OperationId::parse("00000000-0000-4000-8000-000000000003").unwrap();
    std::thread::scope(|scope| {
        let first = scope.spawn(|| e.db.admit_trash(&e.op, &e.plan, &"a".repeat(64)));
        let second = scope.spawn(|| e.db.admit_trash(&other, &e.plan, &"a".repeat(64)));
        let results = [first.join().unwrap(), second.join().unwrap()];
        assert_eq!(
            results
                .iter()
                .filter(|r| matches!(r, Ok(Admission::New(_))))
                .count(),
            1
        );
        assert_eq!(
            results
                .iter()
                .filter(|r| **r == Err(StoreError::TokenUsed))
                .count(),
            1
        );
    });
    assert_eq!(e.db.unfinished().unwrap().len(), 1);
}

#[test]
fn changed_plan_or_new_step_cannot_masquerade_as_an_initial_duplicate() {
    let e = env(true);
    e.db.admit_trash(&e.op, &e.plan, &"a".repeat(64)).unwrap();
    let mut other = e.op.clone();
    other.first_step.as_mut().unwrap().step_id =
        StepId::parse("00000000-0000-4000-8000-000000000003").unwrap();
    assert_eq!(
        e.db.admit_trash(&other, &e.plan, &"b".repeat(64)),
        Ok(Admission::Mismatch)
    );
    let mut plan = e.plan.clone();
    plan.markdown.sha256 = [3; 32];
    let hash = plan.initial_hash().unwrap();
    let mut other = e.op.clone();
    other.input_hash = hash.clone();
    other.first_step.as_mut().unwrap().input_hash = hash;
    assert_eq!(
        e.db.admit_trash(&other, &plan, &"b".repeat(64)),
        Ok(Admission::Mismatch)
    );
    assert_eq!(e.db.get(&e.op.id).unwrap().unwrap().checkpoint, 1);
}

#[test]
fn every_admission_insertion_boundary_rolls_back_everything() {
    for table in [
        "operations",
        "operation_steps",
        "operation_resources",
        "trash_plans",
        "trash_members",
        "trash_authorizations",
    ] {
        let e = env(true);
        e.db.with(|c|c.execute_batch(&format!("CREATE TRIGGER fail_admission BEFORE INSERT ON {table} BEGIN SELECT RAISE(ABORT,'test'); END;")).map_err(|_|DbError::Storage)).unwrap();
        assert_eq!(
            e.db.admit_trash(&e.op, &e.plan, &"a".repeat(64)),
            Err(StoreError::Storage),
            "{table}"
        );
        assert!(e.db.get(&e.op.id).unwrap().is_none());
        assert!(e.db.trash_plan(&e.op.id).unwrap().is_none());
        e.db.with(|c| {
            c.execute_batch("DROP TRIGGER fail_admission;")
                .map_err(|_| DbError::Storage)
        })
        .unwrap();
        assert!(matches!(
            e.db.admit_trash(&e.op, &e.plan, &"a".repeat(64)),
            Ok(Admission::New(_))
        ));
    }
}

fn continuation(e: &Env, n: u8, checkpoint: u64, ordinals: &[u8]) -> NewStep {
    NewStep {
        step_id: StepId::parse(&format!("00000000-0000-4000-8000-0000000000{n:02x}")).unwrap(),
        input_hash: e
            .plan
            .confirmation_hash(false, checkpoint, ordinals)
            .unwrap(),
        expected_checkpoint: checkpoint,
        outcome: format!("admitted:{}", checkpoint + 1),
        state: Some(diary_application::operations::OperationState::Running),
    }
}
fn simulated_receipt(d: &FileDigest) -> Receipt {
    Receipt {
        path: "/private/tmp/diary-simulated-native-receipt".into(),
        dev: d.dev,
        ino: d.ino,
    }
}

#[test]
fn markdown_retry_authorizes_original_pending_audio_and_never_repeats_completed_members() {
    use super::progress::continuation_ordinals;
    let e = env(true);
    e.db.admit_trash(&e.op, &e.plan, &"a".repeat(64)).unwrap();
    assert_eq!(
        e.db.intend_trash_member(&e.op.id, 1, 1),
        Err(StoreError::Invalid)
    );
    e.db.intend_trash_member(&e.op.id, 0, 1).unwrap();
    e.db.record_trash_member(
        &e.op.id,
        0,
        1,
        MemberStatus::Failed,
        None,
        Some("unavailable"),
    )
    .unwrap();
    let (_, members) = e.db.trash_plan(&e.op.id).unwrap().unwrap();
    assert_eq!(members[1].status, MemberStatus::Pending);
    assert_eq!(continuation_ordinals(&members).unwrap(), vec![0, 1]);
    let step = continuation(&e, 9, 1, &[0, 1]);
    assert!(matches!(
        e.db.continue_trash(&e.op.id, &step, &e.plan, &[0, 1], &"b".repeat(64)),
        Ok(diary_application::operations::StepOutcome::Applied(_))
    ));
    e.db.intend_trash_member(&e.op.id, 0, 2).unwrap();
    e.db.record_trash_member(
        &e.op.id,
        0,
        2,
        MemberStatus::Moved,
        Some(&simulated_receipt(&e.plan.markdown)),
        None,
    )
    .unwrap();
    e.db.intend_trash_member(&e.op.id, 1, 2).unwrap();
    e.db.record_trash_member(
        &e.op.id,
        1,
        2,
        MemberStatus::Failed,
        None,
        Some("unavailable"),
    )
    .unwrap();
    let (_, members) = e.db.trash_plan(&e.op.id).unwrap().unwrap();
    assert_eq!(continuation_ordinals(&members).unwrap(), vec![1]);
    let audio_step = continuation(&e, 10, 2, &[1]);
    e.db.continue_trash(&e.op.id, &audio_step, &e.plan, &[1], &"c".repeat(64))
        .unwrap();
    assert_eq!(
        e.db.intend_trash_member(&e.op.id, 0, 3),
        Err(StoreError::Invalid)
    );
    e.db.intend_trash_member(&e.op.id, 1, 3).unwrap();
    e.db.record_trash_member(
        &e.op.id,
        1,
        3,
        MemberStatus::Moved,
        Some(&simulated_receipt(e.plan.audio.as_ref().unwrap())),
        None,
    )
    .unwrap();
    assert!(e.db.get(&e.op.id).unwrap().unwrap().state.is_terminal());
    assert!(matches!(
        e.db.continue_trash(&e.op.id, &step, &e.plan, &[0, 1], &"b".repeat(64)),
        Ok(diary_application::operations::StepOutcome::Recorded { checkpoint: 2, .. })
    ));
    let mut competing = e.op.clone();
    competing.id = OperationId::parse("00000000-0000-4000-8000-000000000003").unwrap();
    assert!(matches!(
        e.db.admit_trash(&competing, &e.plan, &"d".repeat(64)),
        Ok(Admission::New(_))
    ));
}

#[test]
fn receipt_commit_failure_quarantines_intent_and_stops_audio_after_reopen() {
    let e = env(true);
    e.db.admit_trash(&e.op, &e.plan, &"a".repeat(64)).unwrap();
    e.db.intend_trash_member(&e.op.id, 0, 1).unwrap();
    e.db.with(|c|c.execute_batch("CREATE TRIGGER fail_receipt BEFORE UPDATE OF receipt ON trash_members BEGIN SELECT RAISE(ABORT,'test'); END;").map_err(|_|DbError::Storage)).unwrap();
    assert_eq!(
        e.db.record_trash_member(
            &e.op.id,
            0,
            1,
            MemberStatus::Moved,
            Some(&simulated_receipt(&e.plan.markdown)),
            None
        ),
        Err(StoreError::Storage)
    );
    assert_eq!(
        e.db.intend_trash_member(&e.op.id, 1, 1),
        Err(StoreError::Invalid)
    );
    let step = continuation(&e, 9, 1, &[0, 1]);
    drop(e.db);
    let db = Db::open_existing(&e.dir, &e.path.join(DB_FILE), false).unwrap();
    db.recover_trash(&e.op.id).unwrap();
    let (_, members) = db.trash_plan(&e.op.id).unwrap().unwrap();
    assert_eq!(members[0].status, MemberStatus::Uncertain);
    assert_eq!(members[1].status, MemberStatus::Pending);
    assert_eq!(
        db.get(&e.op.id).unwrap().unwrap().state,
        diary_application::operations::OperationState::Uncertain
    );
    assert_eq!(
        db.continue_trash(&e.op.id, &step, &e.plan, &[0, 1], &"b".repeat(64)),
        Err(StoreError::Invalid)
    );
}

#[test]
fn recovery_before_any_native_intent_requires_fresh_confirmation() {
    let e = env(false);
    e.db.admit_trash(&e.op, &e.plan, &"a".repeat(64)).unwrap();
    e.db.recover_trash(&e.op.id).unwrap();
    let (_, members) = e.db.trash_plan(&e.op.id).unwrap().unwrap();
    assert_eq!(members[0].status, MemberStatus::Failed);
    assert_eq!(members[1].status, MemberStatus::Missing);
    let step = continuation(&e, 9, 1, &[0]);
    e.db.continue_trash(&e.op.id, &step, &e.plan, &[0], &"b".repeat(64))
        .unwrap();
    e.db.intend_trash_member(&e.op.id, 0, 2).unwrap();
    e.db.record_trash_member(
        &e.op.id,
        0,
        2,
        MemberStatus::Moved,
        Some(&simulated_receipt(&e.plan.markdown)),
        None,
    )
    .unwrap();
    assert_eq!(
        e.db.get(&e.op.id).unwrap().unwrap().state,
        diary_application::operations::OperationState::Succeeded
    );
}

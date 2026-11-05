use super::*;
use crate::ledger::hash_input;
use diary_application::operations::{
    Admission, Blocker, Class, NewOperation, OperationId, ResourceKey,
};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

fn base() -> (tempfile::TempDir, PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap().join("ws");
    fs::create_dir(&root).unwrap();
    (temp, root)
}
fn open(root: &Path, fault: Option<Fault>) -> Result<Workspace, WorkspaceError> {
    let lock = WorkspaceLock::acquire(root).unwrap();
    open_with(&lock, fault)
}
fn diary(root: &Path, name: &str) -> PathBuf {
    root.join(".diary").join(name)
}
fn listing(root: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    for dir in [root.to_path_buf(), root.join(".diary")] {
        for entry in fs::read_dir(dir).unwrap().flatten() {
            if entry.path().is_file() && !entry.file_name().to_string_lossy().ends_with("-shm") {
                let name = entry
                    .path()
                    .strip_prefix(root)
                    .unwrap()
                    .display()
                    .to_string();
                out.push((name, fs::read(entry.path()).unwrap()));
            }
        }
    }
    out.sort();
    out
}
fn operation(ws: &Workspace, n: u8) -> NewOperation {
    NewOperation {
        id: OperationId::parse(&format!("07249746-e280-4d51-a86a-cbcb198ee1{n:02x}")).unwrap(),
        kind: "save".into(),
        hash_version: 1,
        input_hash: hash_input("save", 1, &[("n", &n.to_string())]),
        resources: vec![ResourceKey::parse("authentic:2026-11-05").unwrap()],
        identity: ws.identity.clone(),
        first_step: None,
    }
}

#[test]
fn utc_formatting_is_correct() {
    assert_eq!(utc(0), "1970-01-01T00:00:00Z");
    assert_eq!(utc(1_700_000_000), "2023-11-14T22:13:20Z");
    assert_eq!(utc(1_709_164_800 + 86_399), "2024-02-29T23:59:59Z");
}

#[test]
fn empty_directory_becomes_a_private_ready_workspace_and_reopens() {
    let (_t, root) = base();
    let ws = open(&root, None).unwrap();
    assert_eq!((ws.readiness, ws.reason), (Readiness::Ready, None));
    let id = ws.identity.workspace_id.clone();
    assert_eq!(id.len(), 36);
    let marker: serde_json::Value =
        serde_json::from_slice(&fs::read(diary(&root, MARKER)).unwrap()).unwrap();
    assert_eq!(marker["schema_version"], 1);
    assert_eq!(marker["workspace_id"], id);
    assert!(marker["created_at"].as_str().unwrap().ends_with('Z'));
    assert!(marker.get("token").is_none());
    for name in [MARKER, DB_FILE, "workspace.lock"] {
        assert_eq!(
            fs::metadata(diary(&root, name))
                .unwrap()
                .permissions()
                .mode()
                & 0o7777,
            0o600
        );
    }
    assert!(!diary(&root, MARKER_TMP).exists());
    drop(ws);
    let again = open(&root, None).unwrap();
    assert_eq!(
        (again.readiness, again.identity.workspace_id),
        (Readiness::Ready, id)
    );
}

#[test]
fn every_interruption_point_is_resumable_and_never_leaves_a_marker_early() {
    for fault in [Fault::AfterDatabase, Fault::AfterMarkerTemp] {
        let (_t, root) = base();
        assert_eq!(open(&root, Some(fault)).err(), Some(WorkspaceError::Io));
        assert!(!diary(&root, MARKER).exists(), "{fault:?}");
        let ws = open(&root, None).unwrap();
        assert_eq!(ws.readiness, Readiness::Ready, "{fault:?}");
        assert!(!diary(&root, MARKER_TMP).exists());
        assert!(ws.ledger().is_ok());
    }
}

#[test]
fn interrupted_initialization_keeps_the_same_workspace_id() {
    let (_t, root) = base();
    open(&root, Some(Fault::AfterMarkerTemp)).err();
    let first: String = {
        let lock = WorkspaceLock::acquire(&root).unwrap();
        let db = Db::open_existing(
            &PrivateDir::open(&root.join(".diary")).unwrap(),
            &diary(&root, DB_FILE),
            true,
        )
        .unwrap();
        drop(lock);
        ensure_workspace_id(&db).unwrap()
    };
    assert_eq!(open(&root, None).unwrap().identity.workspace_id, first);
}

#[test]
fn empty_stale_database_file_is_reused() {
    let (_t, root) = base();
    drop(WorkspaceLock::acquire(&root).unwrap());
    let dir = PrivateDir::open(&root.join(".diary")).unwrap();
    dir.create(DB_FILE).unwrap(); // crash right after creating the file
    assert_eq!(open(&root, None).unwrap().readiness, Readiness::Ready);
}

#[test]
fn unexpected_files_require_inspection_and_nothing_is_modified() {
    // A private file we do not know, in .diary.
    let (_t, root) = base();
    drop(WorkspaceLock::acquire(&root).unwrap());
    PrivateDir::open(&root.join(".diary"))
        .unwrap()
        .create("mystery.bin")
        .unwrap();
    let before = listing(&root);
    let ws = open(&root, None).unwrap();
    assert_eq!(
        (ws.readiness, ws.reason),
        (Readiness::InspectionRequired, Some(Reason::UnexpectedFiles))
    );
    assert!(ws.ledger().is_err());
    assert_eq!(listing(&root), before);
    assert!(!diary(&root, DB_FILE).exists() && !diary(&root, MARKER).exists());

    // Another file beside .diary, with no marker.
    let (_t, root) = base();
    drop(WorkspaceLock::acquire(&root).unwrap());
    fs::write(root.join("notes.md"), "mine").unwrap();
    assert_eq!(
        open(&root, None).unwrap().readiness,
        Readiness::InspectionRequired
    );
    assert!(!diary(&root, DB_FILE).exists());

    // A symlink where the database should be.
    let (_t, root) = base();
    drop(WorkspaceLock::acquire(&root).unwrap());
    let target = root.parent().unwrap().join("outside");
    fs::write(&target, "").unwrap();
    std::os::unix::fs::symlink(&target, diary(&root, DB_FILE)).unwrap();
    assert_eq!(
        open(&root, None).unwrap().readiness,
        Readiness::InspectionRequired
    );
    assert_eq!(fs::read(&target).unwrap(), b"");
}

#[test]
fn unmarked_database_with_operations_is_never_adopted() {
    let (_t, root) = base();
    let ws = open(&root, None).unwrap();
    assert!(matches!(
        ws.ledger().unwrap().admit(operation(&ws, 1)).unwrap(),
        Admission::New(_)
    ));
    drop(ws);
    fs::remove_file(diary(&root, MARKER)).unwrap();
    let db = fs::read(diary(&root, DB_FILE)).unwrap();
    let ws = open(&root, None).unwrap();
    assert_eq!(ws.readiness, Readiness::InspectionRequired);
    assert_eq!(fs::read(diary(&root, DB_FILE)).unwrap(), db);
    assert!(!diary(&root, MARKER).exists());
}

#[test]
fn damaged_marker_requires_inspection() {
    let (_t, root) = base();
    drop(open(&root, None).unwrap());
    fs::write(diary(&root, MARKER), "{\"schema_version\":2}").unwrap();
    let ws = open(&root, None).unwrap();
    assert_eq!(
        (ws.readiness, ws.reason),
        (Readiness::InspectionRequired, Some(Reason::MarkerInvalid))
    );
}

#[test]
fn lost_or_damaged_database_enters_recovery_without_recreating_anything() {
    // Missing database.
    let (_t, root) = base();
    drop(open(&root, None).unwrap());
    for suffix in ["", "-wal", "-shm"] {
        let _ = fs::remove_file(diary(&root, &format!("{DB_FILE}{suffix}")));
    }
    let marker = fs::read(diary(&root, MARKER)).unwrap();
    let ws = open(&root, None).unwrap();
    assert_eq!(
        (ws.readiness, ws.reason),
        (Readiness::Recovery, Some(Reason::DatabaseMissing))
    );
    assert_eq!(ws.ledger().err(), Some(LedgerError::Unavailable));
    assert!(
        !diary(&root, DB_FILE).exists(),
        "no empty ledger was created"
    );
    assert_eq!(fs::read(diary(&root, MARKER)).unwrap(), marker);

    // Garbage database.
    let (_t, root) = base();
    drop(open(&root, None).unwrap());
    fs::write(diary(&root, DB_FILE), vec![7u8; 8192]).unwrap();
    for suffix in ["-wal", "-shm"] {
        let _ = fs::remove_file(diary(&root, &format!("{DB_FILE}{suffix}")));
    }
    let before = listing(&root);
    let ws = open(&root, None).unwrap();
    assert_eq!(
        (ws.readiness, ws.reason),
        (Readiness::Recovery, Some(Reason::DatabaseDamaged))
    );
    assert_eq!(
        listing(&root),
        before,
        "recovery deletes and rewrites nothing"
    );

    // A table dropped from an intact file.
    let (_t, root) = base();
    let ws = open(&root, None).unwrap();
    ws.db
        .as_ref()
        .unwrap()
        .with(|c| {
            c.execute_batch("DROP TABLE operation_steps;")
                .map_err(|_| DbError::Storage)
        })
        .unwrap();
    drop(ws);
    assert_eq!(
        open(&root, None).unwrap().reason,
        Some(Reason::DatabaseDamaged)
    );
}

#[test]
fn newer_schema_and_foreign_database_are_recovery() {
    let (_t, root) = base();
    let ws = open(&root, None).unwrap();
    ws.db
        .as_ref()
        .unwrap()
        .with(|c| {
            c.pragma_update(None, "user_version", 9)
                .map_err(|_| DbError::Storage)
        })
        .unwrap();
    drop(ws);
    assert_eq!(open(&root, None).unwrap().reason, Some(Reason::SchemaNewer));

    let (_t, root) = base();
    let ws = open(&root, None).unwrap();
    ws.db
        .as_ref()
        .unwrap()
        .with(|c| {
            c.execute(
                "UPDATE workspace_meta SET workspace_id = 'someone-else'",
                [],
            )
            .map_err(|_| DbError::Storage)
        })
        .unwrap();
    drop(ws);
    assert_eq!(
        open(&root, None).unwrap().reason,
        Some(Reason::WorkspaceMismatch)
    );
}

#[test]
fn restart_quarantines_operations_and_a_moved_root_blocks_them() {
    let (temp, root) = base();
    let ws = open(&root, None).unwrap();
    ws.ledger().unwrap().admit(operation(&ws, 1)).unwrap();
    assert_eq!(
        ws.classify().unwrap()[0].class,
        Class::Blocked(Blocker::NoReconciler)
    );
    drop(ws);

    let moved = temp.path().canonicalize().unwrap().join("ws-moved");
    fs::rename(&root, &moved).unwrap();
    let ws = open(&moved, None).unwrap();
    assert_eq!(
        ws.classify().unwrap()[0].class,
        Class::Blocked(Blocker::RootChanged)
    );
    // The reservation is still held against the new root.
    assert!(ws.ledger().unwrap().admit(operation(&ws, 2)).is_err());
    assert_eq!(ws.ledger().unwrap().unfinished().unwrap().len(), 1);
}

#[test]
fn orphan_sidecar_without_a_database_requires_inspection() {
    let (_t, root) = base();
    drop(WorkspaceLock::acquire(&root).unwrap());
    PrivateDir::open(&root.join(".diary"))
        .unwrap()
        .create("state.sqlite3-wal")
        .unwrap();
    let ws = open(&root, None).unwrap();
    assert_eq!(ws.readiness, Readiness::InspectionRequired);
    assert!(!diary(&root, DB_FILE).exists());
}

#[test]
fn truncated_database_with_a_marker_is_damage_and_stays_untouched() {
    let (_t, root) = base();
    drop(open(&root, None).unwrap());
    for suffix in ["-wal", "-shm"] {
        let _ = fs::remove_file(diary(&root, &format!("{DB_FILE}{suffix}")));
    }
    fs::write(diary(&root, DB_FILE), b"").unwrap();
    let ws = open(&root, None).unwrap();
    assert_eq!(
        (ws.readiness, ws.reason),
        (Readiness::Recovery, Some(Reason::DatabaseDamaged))
    );
    assert_eq!(
        fs::metadata(diary(&root, DB_FILE)).unwrap().len(),
        0,
        "nothing was written"
    );
}

#[test]
fn unreadable_ledger_row_is_recovery_not_ready() {
    let (_t, root) = base();
    let ws = open(&root, None).unwrap();
    ws.ledger().unwrap().admit(operation(&ws, 1)).unwrap();
    ws.db
        .as_ref()
        .unwrap()
        .with(|c| {
            c.execute_batch(
                "PRAGMA ignore_check_constraints = ON; UPDATE operations SET state = 'bogus';",
            )
            .map_err(|_| DbError::Storage)
        })
        .unwrap();
    drop(ws);
    assert_eq!(open(&root, None).unwrap().readiness, Readiness::Recovery);
}

//! Simulated filesystem adapter tests; not native Trash acceptance evidence.
use super::*;
use crate::{
    source::{
        narihito_write::tests_support::{REL, golden},
        root::source_id,
        tests::Tree,
    },
    sqlite::DB_FILE,
};
use diary_application::imports::ImportRepository;
use diary_logging::private_fs::PrivateDir;
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    sync::atomic::{AtomicU8, AtomicU64, Ordering},
};

struct Env {
    _ws: tempfile::TempDir,
    src: Tree,
    service: Service,
    id: String,
    audio: String,
    clock: Arc<AtomicU64>,
    failure: Arc<AtomicU8>,
    calls: Arc<Mutex<Vec<u8>>>,
}
fn env(audio: bool) -> Env {
    let ws = tempfile::tempdir().unwrap();
    let path = ws.path().canonicalize().unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    let dir = PrivateDir::open(&path).unwrap();
    let db = Arc::new(Db::create(&dir, &path.join(DB_FILE)).unwrap());
    let src = Tree::new();
    src.write(REL, &golden("2024-02-29T01:02:03Z", "body"));
    let audio_rel = format!(
        "{}.assets/0b1d2c3e-4f50-4a6b-8c7d-9e0f1a2b3c4d.m4a",
        REL.strip_suffix(".md").unwrap()
    );
    if audio {
        src.write(&audio_rel, b"simulated audio");
    }
    let rel = normalize(REL, src.root.insensitive).unwrap();
    let id = source_id(&src.root, &rel);
    let namespace: [u8; 32] =
        Sha256::digest(src.root.canonical.to_str().unwrap().as_bytes()).into();
    db.bind_source(&id, &namespace, &rel.key).unwrap();
    let mut service = Service::new(
        db,
        Identity {
            workspace_id: "workspace".into(),
            root: path.to_str().unwrap().into(),
            native: "1:2".into(),
            adapter_version: 1,
        },
        vec![Source {
            alias: "test".into(),
            kind: SourceKind::Narihito,
            root: src.temp.path().into(),
        }],
    );
    service.enabled = true;
    service.recover().unwrap();
    let clock = Arc::new(AtomicU64::new(100));
    let c = clock.clone();
    service.clock = Box::new(move || c.load(Ordering::SeqCst));
    let failure = Arc::new(AtomicU8::new(255));
    let f = failure.clone();
    let calls = Arc::new(Mutex::new(vec![]));
    let moved = calls.clone();
    let receipts = path.join("simulated-receipts");
    fs::create_dir(&receipts).unwrap();
    service.mover = Some(Box::new(move |root, rel, expected| {
        let ordinal = if rel.spelling.ends_with(".md") { 0 } else { 1 };
        moved.lock().unwrap().push(ordinal);
        if f.load(Ordering::SeqCst) == ordinal {
            return MoveResult::Failed(PreparationError::Unavailable);
        }
        if f.load(Ordering::SeqCst) == ordinal + 10 {
            return MoveResult::Uncertain { receipt: None };
        }
        let target = receipts.join(format!("{ordinal}-{}", random_uuid().unwrap()));
        fs::rename(root.canonical.join(&rel.spelling), &target).unwrap();
        MoveResult::Moved(native::Receipt {
            path: target,
            dev: expected.dev,
            ino: expected.ino,
        })
    }));
    Env {
        _ws: ws,
        src,
        service,
        id,
        audio: audio_rel,
        clock,
        failure,
        calls,
    }
}
fn uuid(n: u8) -> String {
    format!("00000000-0000-4000-8000-0000000000{n:02x}")
}
fn initial(e: &Env) -> DeletePreview {
    e.service
        .preview(
            "session",
            1,
            DeletePreviewRequest {
                action: DeleteAction::Initial,
                source_id: e.id.clone(),
                operation_id: None,
                expected_checkpoint: 0,
            },
            &|| false,
        )
        .unwrap()
}
fn confirm(e: &Env, p: &DeletePreview, n: u8, action: DeleteAction) -> DeleteConfirmRequest {
    DeleteConfirmRequest {
        action,
        source_id: e.id.clone(),
        operation_id: OperationId::parse(&uuid(1)).unwrap(),
        step_id: StepId::parse(&uuid(n)).unwrap(),
        expected_checkpoint: p.expected_checkpoint,
        plan_hash: p.plan_hash.clone(),
        ordinals: p.ordinals.clone(),
        token: p.token.clone(),
    }
}
fn retry(e: &Env, checkpoint: u64) -> Result<DeletePreview, DeleteError> {
    e.service.preview(
        "session",
        1,
        DeletePreviewRequest {
            action: DeleteAction::Continuation,
            source_id: e.id.clone(),
            operation_id: Some(OperationId::parse(&uuid(1)).unwrap()),
            expected_checkpoint: checkpoint,
        },
        &|| false,
    )
}

#[test]
fn preview_is_read_only_exact_and_duplicate_survives_expiry_session_and_disabled_capability() {
    let mut e = env(true);
    let p = initial(&e);
    assert_eq!(
        p.members
            .iter()
            .map(|m| m.relative_label.as_str())
            .collect::<Vec<_>>(),
        [REL, e.audio.as_str()]
    );
    assert!(e.calls.lock().unwrap().is_empty());
    let request = confirm(&e, &p, 2, DeleteAction::Initial);
    let ack = e
        .service
        .confirm("session", 1, request.clone(), &|| false)
        .unwrap();
    assert_eq!(*e.calls.lock().unwrap(), [0, 1]);
    assert_eq!(
        e.service
            .operation(&ack.operation_id)
            .unwrap()
            .unwrap()
            .state,
        DeleteState::Succeeded
    );
    e.clock.store(120_000, Ordering::SeqCst);
    e.service = Service::new(
        e.service.db.clone(),
        e.service.identity.clone(),
        e.service.sources.clone(),
    );
    assert_eq!(
        e.service
            .confirm("new-session", 2, request, &|| false)
            .unwrap(),
        ack
    );
    assert_eq!(*e.calls.lock().unwrap(), [0, 1]);
    assert!(
        e.service
            .unfinished(None, 50)
            .unwrap()
            .operations
            .is_empty()
    );
}

#[test]
fn expiry_and_session_config_member_changes_do_not_admit() {
    for variation in 0..7 {
        let e = env(true);
        let p = initial(&e);
        let request = confirm(&e, &p, 2, DeleteAction::Initial);
        let (session, generation) = match variation {
            0 => {
                e.clock.store(60_100, Ordering::SeqCst);
                ("session", 1)
            }
            1 => ("other", 1),
            2 => ("session", 2),
            3 => {
                e.src.write(&e.audio, b"changed");
                ("session", 1)
            }
            4 => {
                e.src.write(REL, &golden("2024-02-29T01:02:04Z", "new"));
                ("session", 1)
            }
            5 => {
                fs::rename(e.src.temp.path(), e._ws.path().join("moved-source")).unwrap();
                fs::create_dir(e.src.temp.path()).unwrap();
                ("session", 1)
            }
            _ => {
                fs::rename(e.src.temp.path(), e._ws.path().join("moved-source")).unwrap();
                let other = e._ws.path().join("repointed");
                fs::create_dir(&other).unwrap();
                std::os::unix::fs::symlink(&other, e.src.temp.path()).unwrap();
                ("session", 1)
            }
        };
        assert!(
            e.service
                .confirm(session, generation, request.clone(), &|| false)
                .is_err()
        );
        assert!(e.service.db.get(&request.operation_id).unwrap().is_none());
        assert!(e.calls.lock().unwrap().is_empty());
        assert!(
            if variation >= 5 {
                e._ws.path().join("moved-source").join(REL)
            } else {
                e.src.temp.path().join(REL)
            }
            .exists()
        );
    }
}

#[test]
fn pending_audio_follows_confirmed_markdown_retry_and_retry_does_not_move_twice() {
    let e = env(true);
    e.failure.store(0, Ordering::SeqCst);
    let p = initial(&e);
    let request = confirm(&e, &p, 2, DeleteAction::Initial);
    e.service
        .confirm("session", 1, request.clone(), &|| false)
        .unwrap();
    let result = e.service.operation(&request.operation_id).unwrap().unwrap();
    assert_eq!(result.state, DeleteState::NeedsConfirmation);
    assert_eq!(result.members[1].state, DeleteMemberState::Pending);
    e.failure.store(255, Ordering::SeqCst);
    let p = retry(&e, 1).unwrap();
    assert_eq!(p.ordinals, [0, 1]);
    let request = confirm(&e, &p, 3, DeleteAction::Continuation);
    let ack = e
        .service
        .confirm("session", 1, request.clone(), &|| false)
        .unwrap();
    assert_eq!(ack.admitted_checkpoint, 2);
    assert_eq!(*e.calls.lock().unwrap(), [0, 0, 1]);
    assert_eq!(
        e.service.confirm("session", 1, request, &|| false).unwrap(),
        ack
    );
    assert_eq!(*e.calls.lock().unwrap(), [0, 0, 1]);
}

#[test]
fn audio_retry_blocks_replacement_claim_and_preserves_replacement_markdown() {
    let e = env(true);
    e.failure.store(1, Ordering::SeqCst);
    let p = initial(&e);
    let request = confirm(&e, &p, 2, DeleteAction::Initial);
    e.service.confirm("session", 1, request, &|| false).unwrap();
    assert_eq!(*e.calls.lock().unwrap(), [0, 1]);
    e.src
        .write(REL, &golden("2024-02-29T01:02:05Z", "replacement"));
    assert_eq!(retry(&e, 1).unwrap_err(), DeleteError::OwnershipAmbiguous);
    fs::remove_file(e.src.temp.path().join(REL)).unwrap();
    let p = retry(&e, 1).unwrap();
    assert_eq!(p.ordinals, [1]);
    e.failure.store(255, Ordering::SeqCst);
    e.service
        .confirm(
            "session",
            1,
            confirm(&e, &p, 3, DeleteAction::Continuation),
            &|| false,
        )
        .unwrap();
    assert_eq!(*e.calls.lock().unwrap(), [0, 1, 1]);
}

#[test]
fn missing_audio_is_reserved_and_appearance_before_confirm_blocks_without_moves() {
    let e = env(false);
    let p = initial(&e);
    assert_eq!(p.members[1].state, DeleteMemberState::Missing);
    e.src.write(&e.audio, b"appeared");
    assert_eq!(
        e.service
            .confirm(
                "session",
                1,
                confirm(&e, &p, 2, DeleteAction::Initial),
                &|| false
            )
            .unwrap_err(),
        DeleteError::Changed
    );
    assert!(e.calls.lock().unwrap().is_empty());
}

#[test]
fn uncertainty_is_discoverable_after_recovery_and_never_authorizes_continuation() {
    let e = env(true);
    e.failure.store(10, Ordering::SeqCst);
    let p = initial(&e);
    e.service
        .confirm(
            "session",
            1,
            confirm(&e, &p, 2, DeleteAction::Initial),
            &|| false,
        )
        .unwrap();
    assert_eq!(e.service.recover().unwrap(), 1);
    let page = e.service.unfinished(None, 50).unwrap();
    assert_eq!(page.operations.len(), 1);
    assert_eq!(page.operations[0].state, DeleteState::Uncertain);
    assert_eq!(retry(&e, 1).unwrap_err(), DeleteError::Uncertain);
    assert_eq!(*e.calls.lock().unwrap(), [0]);
}

#[test]
fn discovery_pages_and_binds_cursors_to_workspace() {
    let mut e = env(true);
    e.failure.store(0, Ordering::SeqCst);
    for i in 1..=3 {
        if i > 1 {
            let suffix = if i == 2 { "4e" } else { "4f" };
            let rel = REL.replace("3c4d.md", &format!("3c{suffix}.md"));
            let bytes = String::from_utf8(golden("2024-02-29T01:02:03Z", "body"))
                .unwrap()
                .replace("3c4d", &format!("3c{suffix}"));
            e.src.write(&rel, bytes.as_bytes());
            let path = normalize(&rel, e.src.root.insensitive).unwrap();
            e.id = source_id(&e.src.root, &path);
            let ns: [u8; 32] =
                Sha256::digest(e.src.root.canonical.to_str().unwrap().as_bytes()).into();
            e.service.db.bind_source(&e.id, &ns, &path.key).unwrap();
        }
        let p = initial(&e);
        let mut request = confirm(&e, &p, 20 + i, DeleteAction::Initial);
        request.operation_id = OperationId::parse(&uuid(i)).unwrap();
        e.service.confirm("session", 1, request, &|| false).unwrap();
    }
    let first = e.service.unfinished(None, 2).unwrap();
    assert_eq!(first.operations.len(), 2);
    let second = e
        .service
        .unfinished(first.next_cursor.as_deref(), 2)
        .unwrap();
    assert_eq!(second.operations.len(), 1);
    assert!(second.next_cursor.is_none());
    let mut other = Service::new(e.service.db.clone(), e.service.identity.clone(), vec![]);
    other.identity.workspace_id = "different".into();
    assert_eq!(
        other
            .unfinished(first.next_cursor.as_deref(), 2)
            .unwrap_err(),
        DeleteError::Invalid
    );
}

#[test]
fn previews_are_bounded_cancelled_preparation_does_not_publish_and_old_generation_is_discarded() {
    let e = env(true);
    assert_eq!(
        e.service
            .preview(
                "session",
                1,
                DeletePreviewRequest {
                    action: DeleteAction::Initial,
                    source_id: e.id.clone(),
                    operation_id: None,
                    expected_checkpoint: 0
                },
                &|| true
            )
            .unwrap_err(),
        DeleteError::PreviewStale
    );
    assert!(e.service.previews.lock().unwrap().is_empty());
    for _ in 0..MAX_PREVIEWS {
        initial(&e);
    }
    assert_eq!(e.service.previews.lock().unwrap().len(), MAX_PREVIEWS);
    assert_eq!(
        e.service
            .preview(
                "session",
                1,
                DeletePreviewRequest {
                    action: DeleteAction::Initial,
                    source_id: e.id.clone(),
                    operation_id: None,
                    expected_checkpoint: 0
                },
                &|| false
            )
            .unwrap_err(),
        DeleteError::Limit
    );
    e.service
        .preview(
            "new-session",
            2,
            DeletePreviewRequest {
                action: DeleteAction::Initial,
                source_id: e.id.clone(),
                operation_id: None,
                expected_checkpoint: 0,
            },
            &|| false,
        )
        .unwrap();
    assert_eq!(e.service.previews.lock().unwrap().len(), 1);
    assert!(e.service.db.unfinished().unwrap().is_empty());
    assert!(e.calls.lock().unwrap().is_empty());
}

#[test]
fn simultaneous_initial_and_continuation_confirmations_execute_each_step_once() {
    let e = env(true);
    e.failure.store(0, Ordering::SeqCst);
    let p = initial(&e);
    let request = confirm(&e, &p, 2, DeleteAction::Initial);
    std::thread::scope(|scope| {
        let a = scope.spawn(|| e.service.confirm("session", 1, request.clone(), &|| false));
        let b = scope.spawn(|| e.service.confirm("session", 1, request.clone(), &|| false));
        assert_eq!(a.join().unwrap().unwrap(), b.join().unwrap().unwrap());
    });
    assert_eq!(*e.calls.lock().unwrap(), [0]);
    e.failure.store(255, Ordering::SeqCst);
    let p = retry(&e, 1).unwrap();
    let request = confirm(&e, &p, 3, DeleteAction::Continuation);
    std::thread::scope(|scope| {
        let a = scope.spawn(|| e.service.confirm("session", 1, request.clone(), &|| false));
        let b = scope.spawn(|| e.service.confirm("session", 1, request.clone(), &|| false));
        assert_eq!(a.join().unwrap().unwrap(), b.join().unwrap().unwrap());
    });
    assert_eq!(*e.calls.lock().unwrap(), [0, 0, 1]);
    assert_eq!(
        e.service
            .operation(&request.operation_id)
            .unwrap()
            .unwrap()
            .checkpoint,
        2
    );
}

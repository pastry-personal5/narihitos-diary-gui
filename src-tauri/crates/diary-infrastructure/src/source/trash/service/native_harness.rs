//! Explicit, synthetic-only process worker. Not compiled into application binaries.
use super::*;
use crate::source::root::{resolve_roots, source_id};
use diary_application::imports::ImportRepository;
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::PathBuf,
};

fn base() -> PathBuf {
    let p = PathBuf::from(std::env::var("DIARY_TRASH_FIXTURE").unwrap())
        .canonicalize()
        .unwrap();
    assert!(p.starts_with(std::env::temp_dir().canonicalize().unwrap()));
    assert!(
        p.file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("diary-trash-smoke-")
    );
    assert_eq!(fs::metadata(&p).unwrap().uid(), unsafe { libc::getuid() });
    assert_eq!(
        fs::read_to_string(p.join("fixture-marker")).unwrap(),
        "synthetic-source-trash-v1"
    );
    p
}
pub(super) fn retain_receipt(
    result: &MoveResult,
    original: &FileDigest,
    root: &ResolvedRoot,
    rel: &diary_domain::source::path::SourcePath,
) -> Result<(), DeleteError> {
    if std::env::var_os("DIARY_TRASH_FIXTURE").is_none() {
        return Ok(());
    }
    if let MoveResult::Moved(receipt)
    | MoveResult::Uncertain {
        receipt: Some(receipt),
    } = result
    {
        let base = base();
        assert_eq!(root.canonical, base.join("source"));
        let row = json!({"receipt":receipt,"original":original,"relative":rel.spelling});
        let mut f = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .mode(0o600)
            .open(base.join("native-receipts.jsonl"))
            .map_err(|_| DeleteError::Storage)?;
        writeln!(f, "{row}").map_err(|_| DeleteError::Storage)?;
        f.sync_all().map_err(|_| DeleteError::Storage)?;
    }
    Ok(())
}
fn uuid(n: u8) -> String {
    format!("00000000-0000-4000-8000-0000000000{n:02x}")
}
fn save_request(p: &std::path::Path, r: &DeleteConfirmRequest) {
    let value = json!({"action":if r.action==DeleteAction::Initial {"initial"}else{"continuation"},"source_id":r.source_id,"operation_id":r.operation_id.as_str(),"step_id":r.step_id.as_str(),"expected_checkpoint":r.expected_checkpoint,"plan_hash":r.plan_hash,"ordinals":r.ordinals,"token":r.token});
    let mut f = fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(p)
        .unwrap();
    f.write_all(value.to_string().as_bytes()).unwrap();
    f.sync_all().unwrap();
}
fn load_request(p: &std::path::Path) -> DeleteConfirmRequest {
    let v: Value = serde_json::from_slice(&fs::read(p).unwrap()).unwrap();
    DeleteConfirmRequest {
        action: if v["action"] == "initial" {
            DeleteAction::Initial
        } else {
            DeleteAction::Continuation
        },
        source_id: v["source_id"].as_str().unwrap().into(),
        operation_id: OperationId::parse(v["operation_id"].as_str().unwrap()).unwrap(),
        step_id: StepId::parse(v["step_id"].as_str().unwrap()).unwrap(),
        expected_checkpoint: v["expected_checkpoint"].as_u64().unwrap(),
        plan_hash: v["plan_hash"].as_str().unwrap().into(),
        ordinals: serde_json::from_value(v["ordinals"].clone()).unwrap(),
        token: v["token"].as_str().unwrap().into(),
    }
}
#[test]
#[ignore = "explicit synthetic native process worker; scripts/smoke/trash.py"]
fn native_process_worker() {
    let base = base();
    let action = std::env::var("DIARY_TRASH_ACTION").unwrap();
    if action == "cleanup" {
        if let Ok(lines) = fs::read_to_string(base.join("native-receipts.jsonl")) {
            for line in lines.lines() {
                let v: Value = serde_json::from_str(line).unwrap();
                let receipt: native::Receipt =
                    serde_json::from_value(v["receipt"].clone()).unwrap();
                let original: FileDigest = serde_json::from_value(v["original"].clone()).unwrap();
                assert!(v["relative"].as_str().unwrap().starts_with("2024/02/29/"));
                assert!(
                    native::verify_receipt(&receipt, &original),
                    "fixture receipt changed; preserving it"
                );
                fs::remove_file(receipt.path).unwrap();
            }
        }
        return;
    }
    let path = base.join("workspace");
    fs::create_dir_all(&path).unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    let lock = crate::lifecycle::WorkspaceLock::acquire(&path).unwrap();
    let workspace = crate::workspace::Workspace::open(&lock).unwrap();
    let db = workspace.db();
    let sources = vec![Source {
        alias: "fixture".into(),
        kind: SourceKind::Narihito,
        root: base.join("source"),
    }];
    let roots = resolve_roots(&sources);
    let root = roots[0].as_ref().unwrap();
    let rel = fs::read_to_string(base.join("target-relative")).unwrap();
    let rel = normalize(&rel, root.insensitive).unwrap();
    let id = source_id(root, &rel);
    let ns: [u8; 32] = Sha256::digest(root.canonical.to_str().unwrap().as_bytes()).into();
    db.bind_source(&id, &ns, &rel.key).unwrap();
    let mut service = Service::new(db, workspace.identity.clone(), sources);
    service.recover().unwrap();
    if std::env::var_os("DIARY_HARNESS_DB_FAIL").is_some() {
        service.db.with(|c| c.execute_batch("CREATE TRIGGER fail_native_receipt BEFORE UPDATE OF receipt ON trash_members BEGIN SELECT RAISE(ABORT,'synthetic'); END;").map_err(|_| DbError::Storage)).unwrap();
    }
    service.enabled = true; // Unit-test-only override. Application enablement is unchanged.
    let op = OperationId::parse(&uuid(1)).unwrap();
    if action == "cleanup-database" {
        let (_, plan, members) = service.load(&op).unwrap();
        assert_eq!(plan.markdown_rel, rel.spelling);
        for m in members {
            if let Some(receipt) = m.receipt {
                let original = if m.ordinal == 0 {
                    &plan.markdown
                } else {
                    plan.audio.as_ref().unwrap()
                };
                assert!(
                    native::verify_receipt(&receipt, original),
                    "fixture receipt changed; preserving it"
                );
                fs::remove_file(receipt.path).unwrap();
            }
        }
        return;
    }
    let mut reply = json!({"normal_enabled":TRASH_PAIR_VERIFIED});
    if action == "initial" || action == "retry" {
        let checkpoint = service.operation(&op).unwrap().map_or(0, |r| r.checkpoint);
        let kind = if action == "initial" {
            DeleteAction::Initial
        } else {
            DeleteAction::Continuation
        };
        let preview = service.preview(
            "fixture-session",
            1,
            DeletePreviewRequest {
                action: kind,
                source_id: id.clone(),
                operation_id: if kind == DeleteAction::Initial {
                    None
                } else {
                    Some(op.clone())
                },
                expected_checkpoint: checkpoint,
            },
            &|| false,
        );
        match preview {
            Err(e) => {
                reply["error"] = json!(format!("{e:?}"));
            }
            Ok(p) => {
                let request = DeleteConfirmRequest {
                    action: kind,
                    source_id: id,
                    operation_id: op.clone(),
                    step_id: StepId::parse(&uuid(if kind == DeleteAction::Initial {
                        2
                    } else {
                        3
                    }))
                    .unwrap(),
                    expected_checkpoint: checkpoint,
                    plan_hash: p.plan_hash,
                    ordinals: p.ordinals,
                    token: p.token,
                };
                save_request(
                    &base.join(if action == "initial" {
                        "initial-request.json"
                    } else {
                        "retry-request.json"
                    }),
                    &request,
                );
                match service.confirm("fixture-session", 1, request, &|| false) {
                    Ok(a) => reply["ack"] = json!(a.admitted_checkpoint),
                    Err(e) => reply["error"] = json!(format!("{e:?}")),
                }
            }
        }
    } else if action == "duplicate" {
        service.enabled = false;
        let r = load_request(&base.join("initial-request.json"));
        let a = service.confirm("new-session", 99, r, &|| false).unwrap();
        reply["ack"] = json!(a.admitted_checkpoint);
    } else {
        assert_eq!(action, "recover");
    }
    if let Some(r) = service.operation(&op).unwrap() {
        reply["state"] = json!(r.state.as_str());
        reply["checkpoint"] = json!(r.checkpoint);
        reply["members"] = json!(
            r.members
                .iter()
                .map(|m| json!({"state":m.state.as_str(),"retry":m.retry_eligible}))
                .collect::<Vec<_>>()
        );
    }
    fs::write(base.join("reply.json"), reply.to_string()).unwrap();
}

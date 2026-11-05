use super::*;
use crate::{
    authentic::{Fault, FaultKind},
    lifecycle::WorkspaceLock,
    source::narihito_write::tests_support::{REL, golden},
    workspace::Workspace,
};
use diary_application::imports::ImportRepository;
use std::fs;

struct Env {
    _ws_dir: tempfile::TempDir,
    src: tempfile::TempDir,
    _lock: WorkspaceLock,
    ws: Workspace,
    writer: Writer,
    id: String,
}
fn op(n: u8) -> OperationId {
    OperationId::parse(&format!("00000000-0000-4000-8000-0000000000{n:02x}")).unwrap()
}
fn env() -> Env {
    let ws_dir = tempfile::tempdir().unwrap();
    let ws_root = ws_dir.path().canonicalize().unwrap().join("ws");
    fs::create_dir(&ws_root).unwrap();
    let src = tempfile::tempdir().unwrap();
    let file = src.path().join(REL);
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(&file, golden("2024-02-29T01:02:03Z", "hello\n")).unwrap();
    let sources = vec![Source {
        alias: "mobile".into(),
        kind: SourceKind::Narihito,
        root: src.path().into(),
    }];
    let lock = WorkspaceLock::acquire(&ws_root).unwrap();
    let ws = Workspace::open(&lock).unwrap();
    let mut writer = Writer::new(ws.db(), ws.identity.clone(), sources.clone());
    writer.enabled = true;
    writer.now = Box::new(|| 2_000_000_000);
    let root = resolve_roots(&sources).pop().unwrap().unwrap();
    let path = normalize(REL, root.insensitive).unwrap();
    let id = source_id(&root, &path);
    let ns: [u8; 32] = Sha256::digest(root.canonical.to_str().unwrap()).into();
    ws.db().bind_source(&id, &ns, &path.key).unwrap();
    Env {
        _ws_dir: ws_dir,
        src,
        _lock: lock,
        ws,
        writer,
        id,
    }
}
impl Env {
    fn file(&self) -> PathBuf {
        self.src.path().join(REL)
    }
    fn bytes(&self) -> Vec<u8> {
        fs::read(self.file()).unwrap()
    }
    fn digest(&self) -> String {
        sha_hex(&self.bytes())
    }
    fn req(&self, n: u8, base: &str, body: &str) -> SourceSaveRequest {
        SourceSaveRequest {
            operation_id: op(n),
            source_id: self.id.clone(),
            base_digest: base.into(),
            body: body.into(),
        }
    }
    fn save(&self, n: u8, base: &str, body: &str) -> Result<SourceSaved, SaveError> {
        self.writer.save_body(self.req(n, base, body))
    }
    fn names(&self) -> Vec<String> {
        let dir = self.file().parent().unwrap().to_owned();
        fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect()
    }
    fn state(&self, n: u8) -> OperationState {
        self.writer.db.get(&op(n)).unwrap().unwrap().state
    }
}

#[test]
fn saves_and_chains_the_returned_digest() {
    let e = env();
    let first = e.save(1, &e.digest(), "one\n").unwrap();
    assert_eq!(first.digest, e.digest());
    assert!(!first.unchanged);
    let text = String::from_utf8(e.bytes()).unwrap();
    assert!(text.ends_with("---\n\none\n"));
    assert!(text.contains("updated_at: \"2033-05-18T03:33:20Z\""));
    let second = e.save(2, &first.digest, "two\n").unwrap();
    assert_eq!(second.digest, e.digest());
    assert!(e.names().iter().all(|n| !n.ends_with(".tmp")), "{:?}", e.names());
    assert_eq!(e.state(2), OperationState::Succeeded);
}

#[test]
fn external_change_keeps_the_file() {
    let e = env();
    let base = e.digest();
    fs::write(e.file(), golden("2024-02-29T01:02:03Z", "phone edit\n")).unwrap();
    let theirs = e.bytes();
    assert_eq!(e.save(1, &base, "mine\n"), Err(SaveError::ExternalChange));
    assert_eq!(e.bytes(), theirs);
    assert!(e.writer.db.get(&op(1)).unwrap().is_none());
}

#[test]
fn unchanged_body_records_no_operation() {
    let e = env();
    let base = e.digest();
    let out = e.save(1, &base, "hello\n").unwrap();
    assert!(out.unchanged);
    assert_eq!(out.digest, base);
    assert!(e.writer.db.get(&op(1)).unwrap().is_none());
    assert_eq!(e.digest(), base);
}

#[test]
fn lost_reply_returns_first_outcome_and_same_id_other_body_mismatches() {
    let e = env();
    let base = e.digest();
    let first = e.save(1, &base, "one\n").unwrap();
    let written = e.bytes();
    assert_eq!(e.save(1, &base, "one\n").unwrap(), first);
    assert_eq!(e.bytes(), written);
    assert_eq!(e.save(1, &base, "other\n"), Err(SaveError::Mismatch));
}

#[test]
fn invalid_body_and_unsupported_file_and_disabled_writer() {
    let mut e = env();
    let base = e.digest();
    assert_eq!(e.save(1, &base, "a\rb"), Err(SaveError::InvalidBody));
    assert!(e.writer.db.get(&op(1)).unwrap().is_none());
    let broken = String::from_utf8(e.bytes()).unwrap().replace("favorite: false", "favorite: maybe");
    fs::write(e.file(), &broken).unwrap();
    assert_eq!(e.save(2, &e.digest(), "x"), Err(SaveError::UnsupportedFormat));
    fs::write(e.file(), golden("2024-02-29T01:02:03Z", "hello\n")).unwrap();
    e.writer.enabled = false;
    assert_eq!(e.save(3, &e.digest(), "x"), Err(SaveError::UnsupportedFormat));
}

#[test]
fn hard_link_and_unbound_id_are_refused() {
    let e = env();
    fs::hard_link(e.file(), e.src.path().join("link.md")).unwrap();
    assert_eq!(e.save(1, &e.digest(), "x"), Err(SaveError::UnsupportedFormat));
    let mut req = e.req(2, &e.digest(), "x");
    req.source_id = "0123456789abcdef0123456789abcdef".into();
    assert_eq!(e.writer.save_body(req), Err(SaveError::SourceStale));
}

#[test]
fn repointed_root_is_source_stale() {
    let mut e = env();
    let other = tempfile::tempdir().unwrap();
    e.writer.sources[0].root = other.path().into();
    assert_eq!(e.save(1, &e.digest(), "x"), Err(SaveError::SourceStale));
}

#[test]
fn crash_before_intent_publishes_nothing() {
    let e = env();
    let before = e.bytes();
    e.writer.inject(Fault { step: Step::Intent, when: When::Before, kind: FaultKind::Crash });
    assert!(e.save(1, &e.digest(), "x\n").is_err());
    assert_eq!(e.bytes(), before);
    assert!(e.writer.db.get(&op(1)).unwrap().is_none());
}

#[test]
fn crash_after_publish_is_finished_by_reconcile_and_retry_returns_it() {
    let e = env();
    let base = e.digest();
    e.writer.inject(Fault { step: Step::Publish, when: When::After, kind: FaultKind::Crash });
    assert!(e.save(1, &base, "x\n").is_err());
    assert_eq!(e.state(1), OperationState::Running);
    let written = e.digest();
    assert_eq!(e.writer.recover(&[op(1)]).unwrap(), 1);
    assert_eq!(e.state(1), OperationState::Succeeded);
    assert_eq!(e.save(1, &base, "x\n").unwrap().digest, written);
    assert_eq!(e.digest(), written);
}

#[test]
fn source_intent_resumes_after_restart_from_the_stage() {
    let e = env();
    let base = e.digest();
    e.writer.inject(Fault { step: Step::Publish, when: When::Before, kind: FaultKind::Crash });
    assert!(e.save(1, &base, "x\n").is_err());
    assert_eq!(e.digest(), base);
    // A new process: classification marks it resumable and recovery republishes from the stage.
    let classified = e.ws.classify().unwrap();
    assert!(classified.iter().any(|c| c.kind == KIND_SOURCE_SAVE
        && c.class == diary_application::operations::Class::Resume));
    e.writer.recover(&[op(1)]).unwrap();
    assert_eq!(e.state(1), OperationState::Succeeded);
    assert!(String::from_utf8(e.bytes()).unwrap().ends_with("\n\nx\n"));
}

#[test]
fn external_change_after_stage_keeps_both_and_never_restores() {
    let e = env();
    let base = e.digest();
    e.writer.inject(Fault { step: Step::Publish, when: When::Before, kind: FaultKind::Crash });
    assert!(e.save(1, &base, "mine\n").is_err());
    fs::write(e.file(), golden("2024-02-29T01:02:03Z", "phone\n")).unwrap();
    let theirs = e.bytes();
    e.writer.recover(&[op(1)]).unwrap();
    assert_eq!(e.state(1), OperationState::Failed);
    assert_eq!(e.bytes(), theirs);
    let stage = e.writer.pending().join(format!("{}.md", op(1).as_str()));
    assert!(stage.exists(), "the staged version is kept");
    assert_eq!(e.save(1, &base, "mine\n"), Err(SaveError::ExternalChange));
}

#[test]
fn failure_after_publication_is_uncertain_and_retry_finishes() {
    let e = env();
    let base = e.digest();
    e.writer.inject(Fault { step: Step::Verify, when: When::Before, kind: FaultKind::Io });
    // The failure came after publication, so the reply is "uncertain", never a plain error.
    assert_eq!(e.save(1, &base, "x\n"), Err(SaveError::Uncertain));
    assert_eq!(e.state(1), OperationState::Uncertain);
    // The retry judges by current bytes: the new digest is on disk, so it finishes.
    assert!(e.save(1, &base, "x\n").is_ok());
}

#[test]
fn orphan_temp_removed_on_next_save() {
    let e = env();
    let stray = e.file().parent().unwrap().join(format!(".diary-save-{}.tmp", op(9).as_str()));
    fs::write(&stray, b"left behind").unwrap();
    e.save(1, &e.digest(), "x\n").unwrap();
    assert!(!stray.exists());
}

#[test]
fn mode_preserved() {
    use std::os::unix::fs::PermissionsExt;
    let e = env();
    fs::set_permissions(e.file(), fs::Permissions::from_mode(0o640)).unwrap();
    e.save(1, &e.digest(), "x\n").unwrap();
    assert_eq!(fs::metadata(e.file()).unwrap().permissions().mode() & 0o7777, 0o640);
}

#[test]
fn reserved_while_unfinished() {
    let e = env();
    let base = e.digest();
    e.writer.inject(Fault { step: Step::Publish, when: When::Before, kind: FaultKind::Crash });
    assert!(e.save(1, &base, "x\n").is_err());
    assert_eq!(e.save(2, &base, "y\n"), Err(SaveError::Reserved));
}

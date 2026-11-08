use super::{fake::Fake, *};
use crate::{lifecycle::WorkspaceLock, workspace::Workspace};
use diary_application::{
    authentic::SaveRequest,
    export::{Exports, Started},
};
use diary_domain::authentic::AuthenticRevision;
use std::{path::PathBuf, time::Duration};

struct Env {
    _temp: tempfile::TempDir,
    ws: Workspace,
    _lock: WorkspaceLock,
    store: Arc<dyn AuthenticStore>,
    remote: PathBuf,
    fake: Arc<Fake>,
    svc: ExportCoordinator,
}
fn env() -> Env {
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path().canonicalize().unwrap();
    let root = base.join("ws");
    let remote = base.join("remote");
    std::fs::create_dir(&root).unwrap();
    std::fs::create_dir(&remote).unwrap();
    let lock = WorkspaceLock::acquire(&root).unwrap();
    let ws = Workspace::open(&lock).unwrap();
    let (store, _) = ws.open_authentic().unwrap().unwrap();
    let store: Arc<dyn AuthenticStore> = store;
    let fake = Arc::new(Fake::new(remote.clone()));
    let svc = ws.exports(store.clone(), Some(fake.clone())).unwrap();
    svc.recover().unwrap();
    Env {
        _temp: temp,
        ws,
        _lock: lock,
        store,
        remote,
        fake,
        svc,
    }
}
fn op(n: u8) -> OperationId {
    OperationId::parse(&format!("00000000-0000-4000-8000-0000000000{n:02x}")).unwrap()
}
fn step(n: u8) -> StepId {
    StepId::parse(&format!("11111111-0000-4000-8000-0000000000{n:02x}")).unwrap()
}
fn d(t: &str) -> DiaryDate {
    DiaryDate::parse(t).unwrap()
}
impl Env {
    fn save(
        &self,
        n: u8,
        date: &str,
        expected: Option<&AuthenticRevision>,
        body: &str,
    ) -> AuthenticRevision {
        self.store
            .save(SaveRequest {
                operation_id: op(100 + n),
                date: d(date),
                expected: expected.cloned(),
                body: body.into(),
                buffer_generation: 1,
            })
            .unwrap()
            .revision
    }
    fn request(&self, n: u8, date: &str, rev: &AuthenticRevision) -> StartRequest {
        StartRequest {
            operation_id: op(n),
            date: d(date),
            target: Target::Icloud,
            expected: rev.clone(),
        }
    }
    fn start(&self, n: u8, date: &str, rev: &AuthenticRevision) -> Result<Started, ExportError> {
        self.svc.start("s", 1, self.request(n, date, rev))
    }
    fn state(&self, n: u8) -> ExportState {
        self.svc.operation(&op(n)).unwrap().unwrap().state
    }
    fn wait(&self, n: u8, want: ExportState) {
        for _ in 0..400 {
            if self.state(n) == want {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("operation {n} is {:?}, wanted {want:?}", self.state(n));
    }
    fn remote_text(&self, date: &str) -> Option<String> {
        std::fs::read_to_string(self.remote.join(format!("{date}.md"))).ok()
    }
    fn preview(&self, n: u8, date: &str, rev: &AuthenticRevision) -> Preview {
        match self.start(n, date, rev).unwrap() {
            Started::Preview(p) => p,
            other => panic!("wanted a preview, got {other:?}"),
        }
    }
    fn confirm(&self, p: &Preview, s: u8) -> Result<Admitted, ExportError> {
        self.svc.confirm(
            "s",
            1,
            ConfirmRequest {
                operation_id: p.operation_id.clone(),
                step_id: step(s),
                expected_checkpoint: p.expected_checkpoint,
                token: p.token.clone(),
            },
        )
    }
    fn restart(&self) -> ExportCoordinator {
        let svc = self
            .ws
            .exports(self.store.clone(), Some(self.fake.clone()))
            .unwrap();
        svc.recover().unwrap();
        svc
    }
}

// T01

mod cancel_abandon;
mod concurrency;
mod notion;
mod recovery;
mod start;

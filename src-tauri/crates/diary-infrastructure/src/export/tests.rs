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
    svc: Service,
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
    fn restart(&self) -> Service {
        let svc = self
            .ws
            .exports(self.store.clone(), Some(self.fake.clone()))
            .unwrap();
        svc.recover().unwrap();
        svc
    }
}

// T01
#[test]
fn only_saved_revisions_are_exported_and_inputs_are_bound() {
    let e = env();
    let none = AuthenticRevision {
        sequence: 1,
        sha256: "0".repeat(64),
    };
    assert_eq!(
        e.start(1, "2026-11-05", &none).unwrap_err(),
        ExportError::NothingToExport
    );
    let rev = e.save(1, "2026-11-05", None, "one\n");
    let stale = AuthenticRevision {
        sequence: rev.sequence,
        sha256: "f".repeat(64),
    };
    assert_eq!(
        e.start(2, "2026-11-05", &stale).unwrap_err(),
        ExportError::StaleRevision
    );
    assert!(matches!(
        e.start(3, "2026-11-05", &rev).unwrap(),
        Started::Result(_)
    ));
    e.wait(3, ExportState::Succeeded);
    // Same ID, identical inputs: the recorded result. Different inputs: a mismatch.
    assert!(
        matches!(e.start(3, "2026-11-05", &rev).unwrap(), Started::Result(r) if r.state == ExportState::Succeeded)
    );
    let rev2 = e.save(2, "2026-11-05", Some(&rev), "two\n");
    assert_eq!(
        e.start(3, "2026-11-05", &rev2).unwrap_err(),
        ExportError::Mismatch
    );
}

#[test]
fn a_changed_root_is_a_new_namespace_and_never_reuses_receipts() {
    let e = env();
    let rev = e.save(1, "2026-11-05", None, "one\n");
    e.start(1, "2026-11-05", &rev).unwrap();
    e.wait(1, ExportState::Succeeded);
    let other = e._temp.path().canonicalize().unwrap().join("other");
    std::fs::create_dir(&other).unwrap();
    let svc =
        e.ws.exports(e.store.clone(), Some(Arc::new(Fake::new(other.clone()))))
            .unwrap();
    svc.recover().unwrap();
    // The other destination is empty: a first create, not an update or a confirmation.
    assert!(matches!(
        svc.start("s", 1, e.request(2, "2026-11-05", &rev)).unwrap(),
        Started::Result(_)
    ));
    for _ in 0..400 {
        if other.join("2026-11-05.md").exists() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(other.join("2026-11-05.md").exists());
}

// T02/T03: create, verified no-op, update; the snapshot is cleared at the terminal state.
#[test]
fn create_noop_and_update_follow_the_receipt() {
    let e = env();
    let rev = e.save(1, "2026-11-05", None, "one\n");
    e.start(1, "2026-11-05", &rev).unwrap();
    e.wait(1, ExportState::Succeeded);
    assert!(e.remote_text("2026-11-05").unwrap().contains("one"));
    assert!(e.ws.exports(e.store.clone(), None).is_some());
    assert!(
        e.svc
            .0
            .db
            .export_row(&op(1))
            .unwrap()
            .unwrap()
            .snapshot
            .is_none()
    );
    // A verified no-op never calls the destination's write.
    e.fake.fail_apply(true);
    assert!(
        matches!(e.start(2, "2026-11-05", &rev).unwrap(), Started::Result(r) if r.state == ExportState::Succeeded)
    );
    e.fake.fail_apply(false);
    let rev2 = e.save(2, "2026-11-05", Some(&rev), "two\n");
    e.start(3, "2026-11-05", &rev2).unwrap();
    e.wait(3, ExportState::Succeeded);
    assert!(e.remote_text("2026-11-05").unwrap().contains("two"));
}

#[test]
fn definite_failure_releases_the_destination() {
    let e = env();
    let rev = e.save(1, "2026-11-05", None, "one\n");
    e.fake.fail_apply(true);
    e.start(1, "2026-11-05", &rev).unwrap();
    e.wait(1, ExportState::Failed);
    assert!(e.remote_text("2026-11-05").is_none());
    assert!(!e.svc.has_unfinished().unwrap());
    e.fake.fail_apply(false);
    e.start(2, "2026-11-05", &rev).unwrap();
    e.wait(2, ExportState::Succeeded);
}

// T02/T05/T06: external change, deletion and an unowned occupant need confirmation.
#[test]
fn drift_deletion_and_unowned_targets_need_one_use_confirmation() {
    let e = env();
    // Unowned occupant of an absent receipt.
    std::fs::write(e.remote.join("2026-11-05.md"), "someone else").unwrap();
    let rev = e.save(1, "2026-11-05", None, "one\n");
    let p = e.preview(1, "2026-11-05", &rev);
    assert_eq!(p.reason, Reason::Unowned);
    assert_eq!(e.state(1), ExportState::NeedsConfirmation);
    assert!(e.svc.has_unfinished().unwrap());
    assert_eq!(
        e.remote_text("2026-11-05").unwrap(),
        "someone else",
        "no silent overwrite"
    );
    // Wrong session and wrong checkpoint never run the step.
    assert_eq!(
        e.svc
            .confirm(
                "other",
                1,
                ConfirmRequest {
                    operation_id: op(1),
                    step_id: step(1),
                    expected_checkpoint: p.expected_checkpoint,
                    token: p.token.clone()
                }
            )
            .unwrap_err(),
        ExportError::PreviewStale
    );
    assert_eq!(
        e.svc
            .confirm(
                "s",
                1,
                ConfirmRequest {
                    operation_id: op(1),
                    step_id: step(1),
                    expected_checkpoint: p.expected_checkpoint + 1,
                    token: p.token.clone()
                }
            )
            .unwrap_err(),
        ExportError::CheckpointMismatch
    );
    let ack = e.confirm(&p, 1).unwrap();
    assert_eq!(ack.admitted_checkpoint, p.expected_checkpoint + 1);
    e.wait(1, ExportState::Succeeded);
    assert!(e.remote_text("2026-11-05").unwrap().contains("one"));
    // T06: a duplicate returns the recorded step, token consumed, op terminal, nothing re-run.
    let again = e.confirm(&p, 1).unwrap();
    assert_eq!(again, ack);
    // A new step with the consumed token is refused.
    assert_eq!(
        e.confirm(&p, 2).unwrap_err(),
        ExportError::CheckpointMismatch
    );

    // External change after a receipt.
    std::fs::write(e.remote.join("2026-11-05.md"), "edited outside").unwrap();
    let rev2 = e.save(2, "2026-11-05", Some(&rev), "two\n");
    let p2 = e.preview(2, "2026-11-05", &rev2);
    assert_eq!(p2.reason, Reason::Changed);
    // The remote moves again before confirmation: a new preview is required.
    std::fs::write(e.remote.join("2026-11-05.md"), "edited again").unwrap();
    assert_eq!(e.confirm(&p2, 3).unwrap_err(), ExportError::PreviewStale);
    assert_eq!(e.state(2), ExportState::NeedsConfirmation);
    // Same ID and inputs issue a fresh preview for the same paused operation.
    let p3 = e.preview(2, "2026-11-05", &rev2);
    assert_ne!(p3.token, p2.token);
    e.confirm(&p3, 4).unwrap();
    e.wait(2, ExportState::Succeeded);

    // External deletion after a receipt: recreation needs confirmation.
    std::fs::remove_file(e.remote.join("2026-11-05.md")).unwrap();
    let rev3 = e.save(3, "2026-11-05", Some(&rev2), "three\n");
    let p4 = e.preview(3, "2026-11-05", &rev3);
    assert_eq!(p4.reason, Reason::Deleted);
    assert_eq!(p4.write, Write::Create);
    e.confirm(&p4, 5).unwrap();
    e.wait(3, ExportState::Succeeded);
}

#[test]
fn concurrent_confirmations_advance_once() {
    let e = env();
    std::fs::write(e.remote.join("2026-11-05.md"), "x").unwrap();
    let rev = e.save(1, "2026-11-05", None, "one\n");
    let p = e.preview(1, "2026-11-05", &rev);
    let results: Vec<_> = std::thread::scope(|s| {
        let a = s.spawn(|| e.confirm(&p, 1));
        let b = s.spawn(|| e.confirm(&p, 2));
        vec![a.join().unwrap(), b.join().unwrap()]
    });
    assert_eq!(
        results.iter().filter(|r| r.is_ok()).count(),
        1,
        "{results:?}"
    );
    e.wait(1, ExportState::Succeeded);
}

// T04: the destination serializes; the live Authentic date is never reserved.
#[test]
fn paused_exports_hold_the_destination_across_restart_but_not_authentic_saves() {
    let e = env();
    std::fs::write(e.remote.join("2026-11-05.md"), "x").unwrap();
    let rev = e.save(1, "2026-11-05", None, "one\n");
    e.preview(1, "2026-11-05", &rev);
    let rev_b = e.save(2, "2026-11-06", None, "b\n");
    assert_eq!(
        e.start(2, "2026-11-06", &rev_b).unwrap_err(),
        ExportError::Busy
    );
    // Autosave of the exported date proceeds while the export is paused.
    e.save(3, "2026-11-05", Some(&rev), "one more\n");
    let again = e.restart();
    assert_eq!(
        again.operation(&op(1)).unwrap().unwrap().state,
        ExportState::NeedsConfirmation
    );
    assert!(again.has_unfinished().unwrap());
    assert_eq!(
        again
            .start("s", 1, e.request(2, "2026-11-06", &rev_b))
            .unwrap_err(),
        ExportError::Busy
    );
}

// T07: crash at every boundary recovers to a recorded state, never a second write.
#[test]
fn crash_before_intent_records_nothing() {
    let e = env();
    let rev = e.save(1, "2026-11-05", None, "one\n");
    let svc =
        e.ws.exports(e.store.clone(), Some(e.fake.clone()))
            .unwrap()
            .with_fault("before_intent:io");
    svc.recover().unwrap();
    assert!(svc.start("s", 1, e.request(1, "2026-11-05", &rev)).is_err());
    assert!(e.svc.operation(&op(1)).unwrap().is_none());
    assert!(e.remote_text("2026-11-05").is_none());
}

#[test]
fn crash_before_any_side_effect_asks_for_a_fresh_confirmation() {
    for point in ["after_intent:io", "after_admission:io"] {
        let e = env();
        let rev = e.save(1, "2026-11-05", None, "one\n");
        let svc =
            e.ws.exports(e.store.clone(), Some(e.fake.clone()))
                .unwrap()
                .with_fault(point);
        svc.recover().unwrap();
        let _ = svc.start("s", 1, e.request(1, "2026-11-05", &rev));
        std::thread::sleep(Duration::from_millis(100));
        assert!(e.remote_text("2026-11-05").is_none(), "{point}");
        let after = e.restart();
        assert_eq!(
            after.operation(&op(1)).unwrap().unwrap().state,
            ExportState::NeedsConfirmation,
            "{point}"
        );
        let Started::Preview(p) = after
            .start("s", 1, e.request(1, "2026-11-05", &rev))
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(p.write, Write::Create);
        after
            .confirm(
                "s",
                1,
                ConfirmRequest {
                    operation_id: op(1),
                    step_id: step(1),
                    expected_checkpoint: p.expected_checkpoint,
                    token: p.token,
                },
            )
            .unwrap();
        for _ in 0..400 {
            if after.operation(&op(1)).unwrap().unwrap().state == ExportState::Succeeded {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(
            after.operation(&op(1)).unwrap().unwrap().state,
            ExportState::Succeeded,
            "{point}"
        );
    }
}

#[test]
fn crash_after_the_side_effect_is_proven_by_read_back_not_replayed() {
    for point in ["after_apply:io", "before_receipt:io"] {
        let e = env();
        let rev = e.save(1, "2026-11-05", None, "one\n");
        let svc =
            e.ws.exports(e.store.clone(), Some(e.fake.clone()))
                .unwrap()
                .with_fault(point);
        svc.recover().unwrap();
        svc.start("s", 1, e.request(1, "2026-11-05", &rev)).unwrap();
        std::thread::sleep(Duration::from_millis(150));
        assert!(e.remote_text("2026-11-05").is_some(), "{point}");
        // Make any replay observable: a second create would fail and be recorded as failed.
        let after = e.restart();
        assert_eq!(
            after.operation(&op(1)).unwrap().unwrap().state,
            ExportState::Succeeded,
            "{point}"
        );
        assert!(after.operation(&op(1)).unwrap().unwrap().reason.is_none());
        assert!(!after.has_unfinished().unwrap());
    }
}

#[test]
fn unprovable_outcomes_become_uncertain_and_block_the_destination() {
    let e = env();
    let rev = e.save(1, "2026-11-05", None, "one\n");
    let svc =
        e.ws.exports(e.store.clone(), Some(e.fake.clone()))
            .unwrap()
            .with_fault("before_apply:io");
    svc.recover().unwrap();
    svc.start("s", 1, e.request(1, "2026-11-05", &rev)).unwrap();
    std::thread::sleep(Duration::from_millis(150));
    let after = e.restart();
    assert_eq!(
        after.operation(&op(1)).unwrap().unwrap().state,
        ExportState::Uncertain
    );
    // The unresolved earlier write reserves the target: nothing else creates there.
    let rev_b = e.save(2, "2026-11-06", None, "b\n");
    assert_eq!(
        after
            .start("s", 1, e.request(2, "2026-11-06", &rev_b))
            .unwrap_err(),
        ExportError::Uncertain
    );
    assert!(after.has_unfinished().unwrap());
    // Confirming an uncertain operation is refused.
    assert_eq!(
        after
            .confirm(
                "s",
                1,
                ConfirmRequest {
                    operation_id: op(1),
                    step_id: step(9),
                    expected_checkpoint: 2,
                    token: "t".into()
                }
            )
            .unwrap_err(),
        ExportError::Uncertain
    );
}

#[test]
fn a_lost_reply_is_reconciled_by_read_back_without_a_second_write() {
    let e = env();
    let rev = e.save(1, "2026-11-05", None, "one\n");
    e.fake.lose_reply(true);
    e.start(1, "2026-11-05", &rev).unwrap();
    e.wait(1, ExportState::Succeeded);
}

// T08/T09 (service level): typed lookup, paging, disabled targets and the reload signal.
#[test]
fn lookup_pages_and_disabled_targets() {
    let e = env();
    std::fs::write(e.remote.join("2026-11-05.md"), "x").unwrap();
    let rev = e.save(1, "2026-11-05", None, "one\n");
    e.preview(1, "2026-11-05", &rev);
    let page = e.svc.unfinished(None, 10).unwrap();
    assert_eq!(page.operations.len(), 1);
    assert_eq!(page.operations[0].reason.as_deref(), Some("unowned"));
    assert_eq!(
        e.svc.unfinished(Some("x.y"), 10).unwrap_err(),
        ExportError::Invalid
    );
    assert!(e.svc.operation(&op(77)).unwrap().is_none());
    let off = e.ws.exports(e.store.clone(), None).unwrap();
    off.recover().unwrap();
    assert_eq!(
        off.start("s", 1, e.request(2, "2026-11-05", &rev))
            .unwrap_err(),
        ExportError::Disabled
    );
    assert!(!off.available(Target::Icloud));
    assert!(
        off.has_unfinished().unwrap(),
        "a surviving paused export is visible without an adapter"
    );
    assert!(e.svc.available(Target::Icloud));
    assert!(!e.svc.available(Target::Notion));
}

#[test]
fn changed_root_cannot_preview_or_confirm_a_paused_operation() {
    let e = env();
    std::fs::write(e.remote.join("2026-11-05.md"), "original occupant").unwrap();
    let rev = e.save(1, "2026-11-05", None, "one\n");
    let p = e.preview(1, "2026-11-05", &rev);
    let other = e._temp.path().join("other");
    std::fs::create_dir(&other).unwrap();
    let svc =
        e.ws.exports(e.store.clone(), Some(Arc::new(Fake::new(other.clone()))))
            .unwrap();
    svc.recover().unwrap();
    assert_eq!(
        svc.confirm(
            "s",
            1,
            ConfirmRequest {
                operation_id: op(1),
                step_id: step(1),
                expected_checkpoint: p.expected_checkpoint,
                token: p.token,
            }
        )
        .unwrap_err(),
        ExportError::Disabled
    );
    let (rec, row) = svc.0.load(&op(1)).unwrap();
    assert_eq!(
        svc.0
            .repreview("s", 1, &rec, &row, &**svc.0.adapter.as_ref().unwrap())
            .unwrap_err(),
        ExportError::Disabled
    );
    assert_eq!(e.remote_text("2026-11-05").unwrap(), "original occupant");
    assert!(!other.join("2026-11-05.md").exists());
}

struct CheckedAdapter {
    fake: Arc<Fake>,
    db: Arc<Db>,
    id: OperationId,
    observations: std::sync::atomic::AtomicUsize,
    writes: std::sync::atomic::AtomicUsize,
    barrier: Option<std::sync::Barrier>,
}
impl ExportAdapter for CheckedAdapter {
    fn target(&self) -> Target {
        self.fake.target()
    }
    fn root(&self) -> String {
        self.fake.root()
    }
    fn observe(&self, date: &DiaryDate) -> Observation {
        assert!(
            self.db.export_row(&self.id).unwrap().is_some(),
            "intent precedes observation"
        );
        let count = self.observations.fetch_add(1, Ordering::SeqCst);
        // The first observation creates a preview; the next two are concurrent confirmations.
        if (1..=2).contains(&count)
            && let Some(barrier) = &self.barrier
        {
            barrier.wait();
        }
        self.fake.observe(date)
    }
    fn apply(
        &self,
        date: &DiaryDate,
        bytes: &[u8],
        write: Write,
        expected: &Observation,
    ) -> Applied {
        self.writes.fetch_add(1, Ordering::SeqCst);
        self.fake.apply(date, bytes, write, expected)
    }
    fn read_back(&self, date: &DiaryDate) -> Observation {
        self.fake.read_back(date)
    }
}

#[test]
fn intent_and_reservation_precede_destination_observation() {
    let e = env();
    std::fs::write(e.remote.join("2026-11-05.md"), "x").unwrap();
    let rev = e.save(1, "2026-11-05", None, "one\n");
    let adapter = Arc::new(CheckedAdapter {
        fake: e.fake.clone(),
        db: e.svc.0.db.clone(),
        id: op(1),
        observations: 0.into(),
        writes: 0.into(),
        barrier: None,
    });
    let svc =
        e.ws.exports(e.store.clone(), Some(adapter.clone()))
            .unwrap();
    svc.recover().unwrap();
    assert!(matches!(
        svc.start("s", 1, e.request(1, "2026-11-05", &rev)).unwrap(),
        Started::Preview(_)
    ));
    assert_eq!(adapter.observations.load(Ordering::SeqCst), 1);
    let other = e.save(2, "2026-11-06", None, "two\n");
    assert_eq!(
        svc.start("s", 1, e.request(2, "2026-11-06", &other))
            .unwrap_err(),
        ExportError::Busy
    );
    assert_eq!(
        adapter.observations.load(Ordering::SeqCst),
        1,
        "reserved destination is not inspected"
    );
}

#[test]
fn concurrent_duplicate_confirmation_records_one_step_and_writes_once() {
    let e = env();
    std::fs::write(e.remote.join("2026-11-05.md"), "x").unwrap();
    let rev = e.save(1, "2026-11-05", None, "one\n");
    let adapter = Arc::new(CheckedAdapter {
        fake: e.fake.clone(),
        db: e.svc.0.db.clone(),
        id: op(1),
        observations: 0.into(),
        writes: 0.into(),
        barrier: Some(std::sync::Barrier::new(2)),
    });
    let svc =
        e.ws.exports(e.store.clone(), Some(adapter.clone()))
            .unwrap();
    svc.recover().unwrap();
    let Started::Preview(p) = svc.start("s", 1, e.request(1, "2026-11-05", &rev)).unwrap() else {
        panic!()
    };
    let request = ConfirmRequest {
        operation_id: op(1),
        step_id: step(1),
        expected_checkpoint: p.expected_checkpoint,
        token: p.token.clone(),
    };
    let (a, b) = std::thread::scope(|s| {
        let a = s.spawn(|| svc.confirm("s", 1, request.clone()));
        let b = s.spawn(|| svc.confirm("s", 1, request.clone()));
        (a.join().unwrap().unwrap(), b.join().unwrap().unwrap())
    });
    assert_eq!(a, b);
    e.wait(1, ExportState::Succeeded);
    assert_eq!(adapter.writes.load(Ordering::SeqCst), 1);
    // The transaction itself must return the recorded outcome even for a consumed token.
    let rec = svc.0.db.get(&op(1)).unwrap().unwrap();
    let hash = hash_input(
        "export_confirm",
        1,
        &[
            ("operation", &rec.input_hash),
            ("checkpoint", &p.expected_checkpoint.to_string()),
        ],
    );
    assert!(matches!(
        svc.0
            .db
            .confirm_export(
                &op(1),
                &NewStep {
                    step_id: step(1),
                    input_hash: hash,
                    expected_checkpoint: p.expected_checkpoint,
                    outcome: "unused".into(),
                    state: Some(OperationState::Running),
                },
                &hex_sha(p.token.as_bytes())
            )
            .unwrap(),
        StepOutcome::Recorded { .. }
    ));
}

fn uncertain(e: &Env, rev: &AuthenticRevision) -> Service {
    let svc =
        e.ws.exports(e.store.clone(), Some(e.fake.clone()))
            .unwrap()
            .with_fault("before_apply:io");
    svc.recover().unwrap();
    svc.start("s", 1, e.request(1, "2026-11-05", rev)).unwrap();
    std::thread::sleep(Duration::from_millis(150));
    let after = e.restart();
    assert_eq!(
        after.operation(&op(1)).unwrap().unwrap().state,
        ExportState::Uncertain
    );
    after
}

// T06: cancel declines a pending confirmation, never touches the remote and frees the target.
#[test]
fn cancel_declines_a_pending_confirmation_and_releases_the_destination() {
    let e = env();
    std::fs::write(e.remote.join("2026-11-05.md"), "occupant").unwrap();
    let rev = e.save(1, "2026-11-05", None, "one\n");
    e.preview(1, "2026-11-05", &rev);
    let done = e.svc.cancel(&op(1)).unwrap();
    assert_eq!(done.state, ExportState::Cancelled);
    assert_eq!(e.remote_text("2026-11-05").unwrap(), "occupant");
    assert!(!e.svc.has_unfinished().unwrap());
    // Idempotent, and the destination accepts a new export.
    assert_eq!(e.svc.cancel(&op(1)).unwrap().state, ExportState::Cancelled);
    let other = e.save(2, "2026-11-06", None, "two\n");
    e.start(2, "2026-11-06", &other).unwrap();
    e.wait(2, ExportState::Succeeded);
}

// T06: a running export stops before `apply`; an uncertain one is not cancellable.
#[test]
fn cancel_before_apply_stops_without_writing() {
    let e = env();
    std::fs::write(e.remote.join("2026-11-05.md"), "occupant").unwrap();
    let rev = e.save(1, "2026-11-05", None, "one\n");
    let p = e.preview(1, "2026-11-05", &rev);
    // Pre-set the flag as `cancel` does for a running export, then let the step run.
    e.svc
        .0
        .cancels
        .lock()
        .unwrap()
        .insert(op(1).as_str().into());
    e.confirm(&p, 1).unwrap();
    e.wait(1, ExportState::Cancelled);
    assert_eq!(e.remote_text("2026-11-05").unwrap(), "occupant");
    assert!(!e.svc.has_unfinished().unwrap());
}

#[test]
fn uncertain_exports_are_not_cancellable() {
    let e = env();
    let rev = e.save(1, "2026-11-05", None, "one\n");
    let after = uncertain(&e, &rev);
    assert_eq!(after.cancel(&op(1)).unwrap_err(), ExportError::Uncertain);
}

// T07: a read-back equal to the saved revision proves the write.
#[test]
fn abandon_resolves_by_read_back_when_the_write_landed() {
    let e = env();
    let rev = e.save(1, "2026-11-05", None, "one\n");
    let after = uncertain(&e, &rev);
    let bytes = after
        .0
        .db
        .export_row(&op(1))
        .unwrap()
        .unwrap()
        .snapshot
        .unwrap();
    std::fs::write(e.remote.join("2026-11-05.md"), bytes).unwrap();
    match after.abandon("s", 1, &op(1)).unwrap() {
        Started::Result(r) => assert_eq!(r.state, ExportState::Succeeded),
        other => panic!("{other:?}"),
    }
    assert!(!after.has_unfinished().unwrap());
}

// T07: otherwise a fresh preview and a one-use confirmation record `abandoned`.
#[test]
fn abandon_needs_a_one_use_confirmation_and_releases_the_block() {
    let e = env();
    let rev = e.save(1, "2026-11-05", None, "one\n");
    let after = uncertain(&e, &rev);
    let Started::Preview(p) = after.abandon("s", 1, &op(1)).unwrap() else {
        panic!("wanted a preview");
    };
    assert_eq!(p.reason, Reason::Uncertain);
    let ask = |step_n: u8, token: &str, session: &str| {
        after.abandon_confirm(
            session,
            1,
            ConfirmRequest {
                operation_id: op(1),
                step_id: step(step_n),
                expected_checkpoint: p.expected_checkpoint,
                token: token.into(),
            },
        )
    };
    assert_eq!(
        ask(1, &p.token, "other").unwrap_err(),
        ExportError::PreviewStale
    );
    // An abandon token cannot authorize a normal confirmation.
    assert!(
        after
            .confirm(
                "s",
                1,
                ConfirmRequest {
                    operation_id: op(1),
                    step_id: step(8),
                    expected_checkpoint: p.expected_checkpoint,
                    token: p.token.clone()
                }
            )
            .is_err()
    );
    let done = ask(1, &p.token, "s").unwrap();
    assert_eq!(done.state, ExportState::Cancelled);
    assert_eq!(done.reason.as_deref(), Some("abandoned"));
    // A duplicate returns the recorded result; the consumed token cannot start a new step.
    assert_eq!(ask(1, &p.token, "s").unwrap().state, ExportState::Cancelled);
    assert_eq!(
        ask(2, &p.token, "s").unwrap_err(),
        ExportError::CheckpointMismatch
    );
    assert!(!after.has_unfinished().unwrap());
    assert!(e.remote_text("2026-11-05").is_none());
    let other = e.save(2, "2026-11-06", None, "two\n");
    after
        .start("s", 1, e.request(2, "2026-11-06", &other))
        .unwrap();
}

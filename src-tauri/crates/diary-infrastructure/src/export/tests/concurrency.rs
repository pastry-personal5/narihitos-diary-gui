use super::*;

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
    // A later start force-stops the paused export instead of being refused.
    let other = e.save(2, "2026-11-06", None, "two\n");
    svc.start("s", 1, e.request(2, "2026-11-06", &other))
        .unwrap();
    assert_eq!(
        svc.operation(&op(1)).unwrap().unwrap().state,
        ExportState::Cancelled
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

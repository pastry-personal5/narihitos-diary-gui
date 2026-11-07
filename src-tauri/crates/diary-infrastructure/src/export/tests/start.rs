use super::*;

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

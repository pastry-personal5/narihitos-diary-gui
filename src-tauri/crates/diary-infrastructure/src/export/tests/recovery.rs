use super::*;

#[test]
fn paused_exports_survive_restart_and_a_new_start_force_stops_them() {
    let e = env();
    std::fs::write(e.remote.join("2026-11-05.md"), "x").unwrap();
    let rev = e.save(1, "2026-11-05", None, "one\n");
    e.preview(1, "2026-11-05", &rev);
    // Autosave of the exported date proceeds while the export is paused.
    e.save(3, "2026-11-05", Some(&rev), "one more\n");
    let again = e.restart();
    assert_eq!(
        again.operation(&op(1)).unwrap().unwrap().state,
        ExportState::NeedsConfirmation
    );
    assert!(again.has_unfinished().unwrap());
    // The paused export survives the restart, but the next start force-stops it.
    let rev_b = e.save(2, "2026-11-06", None, "b\n");
    again
        .start("s", 1, e.request(2, "2026-11-06", &rev_b))
        .unwrap();
    assert_eq!(
        again.operation(&op(1)).unwrap().unwrap().state,
        ExportState::Cancelled
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
            .repreview("s", 1, &rec, &row, &**svc.0.adapters.first().unwrap())
            .unwrap_err(),
        ExportError::Disabled
    );
    assert_eq!(e.remote_text("2026-11-05").unwrap(), "original occupant");
    assert!(!other.join("2026-11-05.md").exists());
}

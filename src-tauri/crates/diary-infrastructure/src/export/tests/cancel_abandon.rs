use super::*;

fn uncertain(e: &Env, rev: &AuthenticRevision) -> ExportCoordinator {
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

// A saved export still waiting for confirmation is stopped automatically by the next start.
#[test]
fn a_new_start_force_stops_a_stale_pending_confirmation() {
    let e = env();
    std::fs::write(e.remote.join("2026-11-05.md"), "occupant").unwrap();
    let rev = e.save(1, "2026-11-05", None, "one\n");
    e.preview(1, "2026-11-05", &rev);
    let other = e.save(2, "2026-11-06", None, "two\n");
    e.start(2, "2026-11-06", &other).unwrap();
    e.wait(2, ExportState::Succeeded);
    assert_eq!(
        e.svc.operation(&op(1)).unwrap().unwrap().state,
        ExportState::Cancelled
    );
    assert_eq!(e.remote_text("2026-11-05").unwrap(), "occupant");
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

// P1-M27: the service with a Notion adapter beside the iCloud one.

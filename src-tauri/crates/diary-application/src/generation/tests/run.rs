//! Sequential execution, cancellation, busy, lease and budget.
use super::*;

#[test]
fn styles_in_order_and_one_call_per_candidate() {
    let mut c = coordinator();
    let mut s = Store::default();
    let g = admit(&mut s, &mut c, "j", frozen(5));
    let fake = Fake::new((0..5).map(|_| candidate()).collect());
    execute(&g, &fake, &|| 0, &mut || {});
    assert_eq!(fake.calls.borrow().len(), 5);
    for (i, r) in fake.calls.borrow().iter().enumerate() {
        let Mode::Candidate {
            index,
            count,
            packet,
        } = r.mode()
        else {
            panic!()
        };
        assert_eq!((*index, *count), (i as u8 + 1, 5));
        assert_eq!(packet.style(), Style::ALL[i]);
    }
    assert_eq!(g.lock().unwrap().counts().ready, 5);
}
#[test]
fn partial_failure_keeps_ready_and_zero_valid_is_failure() {
    for (results, expected, ready) in [
        (
            vec![
                candidate(),
                Err(AgentErrorCode::Timeout.into()),
                candidate(),
            ],
            JobState::Succeeded,
            2,
        ),
        (
            vec![
                Err(AgentErrorCode::Context.into()),
                Err(AgentErrorCode::Failed.into()),
                Err(AgentErrorCode::Timeout.into()),
            ],
            JobState::Failed,
            0,
        ),
    ] {
        let mut c = coordinator();
        let mut s = Store::default();
        let g = admit(&mut s, &mut c, "j", frozen(3));
        let fake = Fake::new(results);
        execute(&g, &fake, &|| 0, &mut || {});
        let g = g.lock().unwrap();
        assert_eq!(g.terminal, Some(expected));
        assert_eq!(g.counts().ready, ready);
        assert_eq!(g.counts().finished, 3);
        assert_eq!(fake.calls.borrow().len(), 3);
    }
}
#[test]
fn cancel_keeps_ready_and_counts_slots_late_result_dropped() {
    let mut c = coordinator();
    let mut s = Store::default();
    let g = admit(&mut s, &mut c, "j", frozen(4));
    let fake = Fake::new(vec![candidate(), candidate()]);
    let token = g.lock().unwrap().run.token();
    let cancel = Run::from(token);
    execute(&g, &fake, &|| 0, &mut || {
        if fake.calls.borrow().len() == 2 {
            cancel.request_cancel();
        }
    });
    let g = g.lock().unwrap();
    assert_eq!(
        g.slots,
        vec![
            Slot::Ready,
            Slot::Cancelled,
            Slot::NotStarted,
            Slot::NotStarted
        ]
    );
    assert!(g.candidates[1].is_none());
    assert_eq!(g.terminal, Some(JobState::Cancelled));
    assert_eq!(
        g.counts(),
        Counts {
            requested: 4,
            finished: 2,
            ready: 1,
            failed: 0,
            cancelled: 1,
            not_started: 2
        }
    );
}
#[test]
fn busy_between_candidates_and_cancel_between_candidates() {
    let mut c = coordinator();
    let mut s = Store::default();
    let g = admit(&mut s, &mut c, "j", frozen(3));
    let fake = Fake::new(vec![candidate()]);
    execute(&g, &fake, &|| 0, &mut || {
        let g = g.lock().unwrap();
        let snap = g.shared.lock().unwrap();
        if snap.phase == Some(Phase::BetweenCandidates) {
            assert_eq!(
                c.admit(
                    "s",
                    0,
                    crate::Admission::new("other", DIGEST, "other", 0, crate::JobKind::Chat)
                ),
                Err(crate::Error::Busy)
            );
            assert!(snap.text.is_empty());
            g.cancel();
        }
    });
    assert_eq!(fake.calls.borrow().len(), 1);
    let g = g.lock().unwrap();
    assert_eq!(g.counts().not_started, 2);
    assert_eq!(g.terminal, Some(JobState::Cancelled));
}
#[test]
fn lease_end_cancels_generation() {
    let mut c = coordinator();
    let mut s = Store::default();
    let g = admit(&mut s, &mut c, "j", frozen(3));
    s.clear();
    let fake = Fake::new(vec![]);
    execute(&g, &fake, &|| 0, &mut || {});
    assert!(fake.calls.borrow().is_empty());
    assert_eq!(g.lock().unwrap().terminal, Some(JobState::Cancelled));
    assert!(s.get("s", "j").is_none());
}
#[test]
fn store_cap_refuses_admission_and_discard_keeps_admission_record() {
    let mut c = coordinator();
    let mut s = Store::default();
    for i in 0..MAX_JOBS {
        let id = format!("j{i}");
        let g = admit(&mut s, &mut c, &id, frozen(1));
        execute(&g, &Fake::new(vec![candidate()]), &|| 0, &mut || {});
        terminal(&s, &mut c, &g);
    }
    let a = Admission {
        session: "s",
        now: 0,
        request_id: "overflow",
        digest: DIGEST,
        job_id: "overflow",
    };
    assert!(matches!(
        s.admit(&mut c, a, frozen(1)),
        Err(Error::DiscardRequired)
    ));
    assert!(s.get("foreign", "j0").is_none());
    let a = Admission {
        session: "s",
        now: 0,
        request_id: "j0",
        digest: DIGEST,
        job_id: "unused",
    };
    assert_eq!(s.admit(&mut c, a, frozen(1)).unwrap().0.id, "j0");
    s.discard(&mut c, "s", 0, "j0").unwrap();
    assert!(s.get("s", "j0").is_none());
    assert_eq!(
        c.lookup_request("s", 0, "j0", DIGEST).unwrap().unwrap().id,
        "j0"
    );
    let a = Admission {
        session: "s",
        now: 0,
        request_id: "j0",
        digest: DIGEST,
        job_id: "unused",
    };
    assert!(matches!(
        s.admit(&mut c, a, frozen(1)),
        Err(Error::NotFound)
    ));
    let g = admit(&mut s, &mut c, "new", frozen(1));
    assert_eq!(g.lock().unwrap().job_id, "new");
}
#[test]
fn output_budget_failure_keeps_ready() {
    let mut c = coordinator();
    let mut s = Store::default();
    let g = admit(&mut s, &mut c, "j", frozen(3));
    let mut big = Candidate {
        draft_markdown: "x".repeat(MAX_TEXT_BYTES),
        claim_support: vec![],
        warnings: vec![],
    };
    big.warnings = vec!["w".repeat(MAX_STORE_BYTES)];
    let fake = Fake::new(vec![
        candidate(),
        Ok(AgentOutput::Candidate(big)),
        candidate(),
    ]);
    execute(&g, &fake, &|| 0, &mut || {});
    let g = g.lock().unwrap();
    assert_eq!(
        g.slots,
        vec![
            Slot::Ready,
            Slot::Failed(AgentErrorCode::Context),
            Slot::Ready
        ]
    );
    assert_eq!(g.terminal, Some(JobState::Succeeded));
}
#[test]
fn discard_releases_content_bytes() {
    let mut c = coordinator();
    let mut s = Store::default();
    let g = admit(&mut s, &mut c, "j", frozen(1));
    execute(&g, &Fake::new(vec![candidate()]), &|| 0, &mut || {});
    terminal(&s, &mut c, &g);
    assert_eq!(
        c.settle_result("s", 0, "j", MAX_STORE_BYTES),
        Err(crate::Error::Limit)
    );
    s.discard(&mut c, "s", 0, "j").unwrap();
    assert_eq!(c.settle_result("s", 0, "j", 1), Err(crate::Error::Limit));
    assert!(c.get("s", 0, "j").is_ok());
}

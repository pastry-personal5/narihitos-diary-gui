use super::*;

const DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
fn setup() -> Coordinator {
    let mut c = Coordinator::new(
        "instance".into(),
        Limits {
            optional_jobs: 1,
            replay_events: 2,
            replay_bytes: 100,
            event_bytes: 60,
            ..Limits::default()
        },
    );
    c.open("client", 0).unwrap();
    c
}
fn admit(c: &mut Coordinator) -> Job {
    c.admit("client", 0, "request", DIGEST, "job", 100).unwrap()
}

#[test]
fn lease_excludes_competitors_and_expiry_erases_state() {
    let mut c = setup();
    admit(&mut c);
    assert_eq!(c.open("other", 1), Err(Error::ClientBusy));
    c.open("client", 5_000).unwrap();
    c.heartbeat("client", 10_000).unwrap();
    assert!(!c.expire(39_999));
    assert!(c.expire(40_000));
    c.open("other", 40_000).unwrap();
    assert_eq!(c.get("other", 40_000, "job"), Err(Error::NotFound));
    assert_eq!(c.get("client", 40_000, "job"), Err(Error::SessionExpired));
}

#[test]
fn retry_recovers_lost_admission_reply_even_at_capacity() {
    let mut c = setup();
    let job = admit(&mut c);
    assert_eq!(
        c.admit("client", 1, "second", DIGEST, "second-job", 100),
        Err(Error::Limit)
    );
    assert_eq!(
        c.admit("client", 1, "request", DIGEST, "unused-new-id", usize::MAX),
        Ok(job)
    );
    assert_eq!(
        c.admit("client", 1, "request", &"b".repeat(64), "unused", 0),
        Err(Error::RequestMismatch)
    );
    assert_eq!(c.list("client", 1, None, 200).unwrap().len(), 1);
}

#[test]
fn cancellation_remains_available_under_saturation_and_is_idempotent() {
    let mut c = setup();
    admit(&mut c);
    c.transition("client", 0, "job", JobState::Running).unwrap();
    let cancelled = c.cancel("client", 0, "job").unwrap();
    assert_eq!(cancelled.sequence, 3);
    assert_eq!(c.cancel("client", 0, "job"), Ok(cancelled));
    assert_eq!(
        c.transition("client", 0, "job", JobState::Running),
        Err(Error::InvalidTransition)
    );
    c.heartbeat("client", 5_000).unwrap();
    c.admit("client", 5_000, "next", DIGEST, "next-job", 0)
        .unwrap();
}

#[test]
fn ordered_replay_reports_gap_even_when_notification_was_lost() {
    let mut c = setup();
    admit(&mut c);
    let first = c.publish("client", 0, "job", 40).unwrap();
    c.transition("client", 0, "job", JobState::Running).unwrap();
    let second = c.publish("client", 0, "job", 40).unwrap();
    c.cancel("client", 0, "job").unwrap();
    let third = c.publish("client", 0, "job", 40).unwrap();
    assert!(first.event_id < second.event_id && second.event_id < third.event_id);
    assert_eq!(
        c.replay("client", 0, "instance", 0),
        Ok(Replay::ResyncRequired)
    );
    assert_eq!(
        c.replay("client", 0, "old-instance", third.event_id),
        Ok(Replay::ResyncRequired)
    );
    assert_eq!(
        c.replay("client", 0, "instance", first.event_id),
        Ok(Replay::Events(vec![second, third]))
    );
    assert_eq!(
        c.get("client", 0, "job").unwrap().state,
        JobState::Cancelled
    );
}

#[test]
fn encoded_byte_bounds_force_resync_without_losing_current_state() {
    let mut c = setup();
    admit(&mut c);
    c.publish("client", 0, "job", 60).unwrap();
    c.transition("client", 0, "job", JobState::Running).unwrap();
    c.publish("client", 0, "job", 60).unwrap();
    assert_eq!(
        c.replay("client", 0, "instance", 0),
        Ok(Replay::ResyncRequired)
    );
    c.cancel("client", 0, "job").unwrap();
    assert_eq!(c.publish("client", 0, "job", 61), Err(Error::Limit));
    assert_eq!(
        c.replay("client", 0, "instance", 2),
        Ok(Replay::ResyncRequired)
    );
    assert_eq!(
        c.get("client", 0, "job").unwrap().state,
        JobState::Cancelled
    );
}

#[test]
fn terminal_metadata_and_reserved_results_remain_bounded() {
    let mut c = Coordinator::new(
        "instance".into(),
        Limits {
            retained_bytes: 2500,
            ..Limits::default()
        },
    );
    c.open("client", 0).unwrap();
    admit(&mut c);
    c.cancel("client", 0, "job").unwrap();
    assert_eq!(
        c.admit("client", 0, "next", DIGEST, "next-job", 0),
        Err(Error::Limit)
    );
    assert_eq!(
        c.admit("client", 0, "request", DIGEST, "ignored", 0)
            .unwrap()
            .state,
        JobState::Cancelled
    );
    c.close("client", 0).unwrap();
    c.open("client-new", 0).unwrap();
    c.admit("client-new", 0, "next", DIGEST, "next-job", 0)
        .unwrap();
}

#[test]
fn optional_admission_preserves_save_reserve_and_control_access() {
    let mut c = Coordinator::new("instance".into(), Limits::default());
    c.open("client", 0).unwrap();
    for n in 0..24 {
        c.admit(
            "client",
            0,
            &format!("request-{n}"),
            DIGEST,
            &format!("job-{n}"),
            0,
        )
        .unwrap();
    }
    assert_eq!(
        c.admit("client", 0, "overflow", DIGEST, "overflow", 0),
        Err(Error::Limit)
    );
    c.heartbeat("client", HEARTBEAT_INTERVAL_MS).unwrap();
    assert_eq!(
        c.list("client", HEARTBEAT_INTERVAL_MS, None, 200)
            .unwrap()
            .len(),
        24
    );
    c.cancel("client", HEARTBEAT_INTERVAL_MS, "job-0").unwrap();
}

#[test]
fn notification_retry_does_not_fabricate_new_job_sequences_or_events() {
    let mut c = setup();
    admit(&mut c);
    let first = c.publish("client", 0, "job", 40).unwrap();
    assert_eq!(c.publish("client", 0, "job", 40).unwrap(), first);
    assert_eq!(c.event_cursor(), first.event_id);
    c.cancel("client", 0, "job").unwrap();
    let cancelled = c.publish("client", 0, "job", 40).unwrap();
    c.cancel("client", 0, "job").unwrap();
    assert_eq!(c.publish("client", 0, "job", 40).unwrap(), cancelled);
    assert_eq!(
        c.replay("client", 0, "instance", 0),
        Ok(Replay::Events(vec![first, cancelled]))
    );
}

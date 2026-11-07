use super::*;

fn seen(present: bool, fingerprint: &str, digest: &str) -> Observation {
    Observation {
        present,
        available: true,
        fingerprint: fingerprint.into(),
        digest: digest.into(),
        ids: String::new(),
        unavailable: None,
    }
}
fn receipt(digest: &str, fingerprint: &str) -> Receipt {
    Receipt {
        revision_sha: digest.into(),
        projection_digest: digest.into(),
        remote_ids: String::new(),
        remote_fingerprint: fingerprint.into(),
    }
}

// T02: every row of the decision table.
#[test]
fn decision_table_is_total() {
    let abs = Observation::absent();
    assert_eq!(
        decide(None, "a", &abs, false),
        Decision::Write(Write::Create)
    );
    assert_eq!(
        decide(None, "a", &seen(true, "f", "a"), false),
        Decision::Confirm(Reason::Unowned, Write::Replace)
    );
    let r = receipt("a", "f");
    assert_eq!(
        decide(Some(&r), "a", &seen(true, "f", "a"), false),
        Decision::Noop
    );
    assert_eq!(
        decide(Some(&r), "b", &seen(true, "f", "a"), false),
        Decision::Write(Write::Replace)
    );
    assert_eq!(
        decide(Some(&r), "b", &seen(true, "g", "x"), false),
        Decision::Confirm(Reason::Changed, Write::Replace)
    );
    // Same local revision but a changed remote is never a no-op.
    assert_eq!(
        decide(Some(&r), "a", &seen(true, "g", "a"), false),
        Decision::Confirm(Reason::Changed, Write::Replace)
    );
    assert_eq!(
        decide(Some(&r), "a", &abs, false),
        Decision::Confirm(Reason::Deleted, Write::Create)
    );
    assert_eq!(decide(None, "a", &abs, true), Decision::Uncertain);
    assert_eq!(
        decide(Some(&r), "a", &seen(true, "f", "a"), true),
        Decision::Uncertain
    );
    let mut offline = abs;
    offline.available = false;
    assert_eq!(decide(None, "a", &offline, true), Decision::Unavailable);
}

#[test]
fn names_round_trip() {
    for t in [Target::Icloud, Target::Notion] {
        assert_eq!(Target::parse(t.as_str()), Some(t));
    }
    for r in [Reason::Changed, Reason::Deleted, Reason::Unowned] {
        assert_eq!(Reason::parse(r.as_str()), Some(r));
    }
    assert_eq!(Target::parse("dropbox"), None);
}

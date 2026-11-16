//! Selection capture, sources and the shared transient cap.
use super::*;

#[test]
fn empty_selection_rejected_and_ineligible_snippets_excluded() {
    let i = Imports::new();
    let a = Authentic {
        present: AtomicBool::new(false),
    };
    assert!(matches!(
        capture(input(&[]), &i, &a, &|| false),
        Err(Error::EmptySelection)
    ));
    assert!(matches!(
        capture(input(&["unknown"]), &i, &a, &|| false),
        Err(Error::EmptySelection)
    ));
    assert_eq!(i.reads.load(Ordering::SeqCst), 0);
    let f = capture(input(&["unknown", "a"]), &i, &a, &|| false).unwrap();
    assert_eq!(f.items.len(), 1);
}
#[test]
fn duplicate_source_rejected_within_and_across_import_snapshots() {
    let a = Authentic {
        present: AtomicBool::new(false),
    };
    for second_import in ["import", "another-import"] {
        let i = Imports::new();
        let mut request = input(&["a", "a"]);
        request.selections[1].import_id = second_import.into();
        assert!(matches!(
            capture(request, &i, &a, &|| false),
            Err(Error::InvalidInput)
        ));
        assert_eq!(i.reads.load(Ordering::SeqCst), 1);
    }
}
#[test]
fn changed_source_refused_wrong_import_date_refused_stale_config_refused() {
    let a = Authentic {
        present: AtomicBool::new(false),
    };
    for i in [
        Imports {
            changed: true,
            ..Imports::new()
        },
        Imports {
            wrong_date: true,
            ..Imports::new()
        },
        Imports {
            stale: true,
            ..Imports::new()
        },
    ] {
        assert!(matches!(
            capture(input(&["a"]), &i, &a, &|| false),
            Err(Error::ChangedSource)
        ));
    }
}
#[test]
fn packet_order_and_labels_context_is_frozen_absent_baseline_allowed() {
    let i = Imports::new();
    let a = Authentic {
        present: AtomicBool::new(false),
    };
    let f = capture(input(&["b", "a"]), &i, &a, &|| false).unwrap();
    assert!(f.baseline.is_none());
    assert_eq!(f.locale.as_str(), "ko-KR");
    assert_eq!(f.date, date());
    assert_eq!(f.count, 3);
    let p = f.packet(0).unwrap();
    assert_eq!(p.snippets()[0].body, "body a");
    assert_eq!(p.snippets()[1].id, "snippet-2");
    assert_eq!(p.snippets()[0].offset, "+09:00");
    assert_eq!(p.snippets()[0].instant_utc, "1970-01-01T00:00:00.000Z");
    assert_eq!(p.snippets()[0].civil_date, date().to_string());
    assert!(!format!("{p:?}").contains("PRIVATE"));
    a.present.store(true, Ordering::SeqCst);
    assert_eq!(f.packet(0).unwrap(), p);
}

#[test]
fn shared_transient_cap_charges_generation_and_preserves_metadata() {
    let mut c = Coordinator::new(
        "i".into(),
        Limits {
            retained_bytes: MAX_STORE_BYTES - 1,
            ..Limits::default()
        },
    );
    c.open("s", 0).unwrap();
    let mut store = Store::default();
    let a = Admission {
        session: "s",
        now: 0,
        request_id: "j",
        digest: DIGEST,
        job_id: "j",
    };
    assert!(matches!(
        store.admit(&mut c, a, frozen(1)),
        Err(Error::Coordination(crate::Error::Limit))
    ));
    assert!(store.get("s", "j").is_none());
    assert!(c.lookup_request("s", 0, "j", DIGEST).unwrap().is_none());
    let mut c = coordinator();
    let g = admit(&mut store, &mut c, "j", frozen(1));
    assert_eq!(
        c.settle_result("s", 0, "j", 0),
        Err(crate::Error::InvalidTransition)
    );
    execute(&g, &Fake::new(vec![candidate()]), &|| 0, &mut || {});
    terminal(&store, &mut c, &g);
    store.discard(&mut c, "s", 0, "j").unwrap();
    // Admission metadata stays charged even after content is discarded.
    assert_eq!(
        c.admit(
            "s",
            0,
            crate::Admission::new(
                "x",
                DIGEST,
                "x",
                crate::MAX_TRANSIENT_BYTES,
                crate::JobKind::Import
            )
        ),
        Err(crate::Error::Limit)
    );
}

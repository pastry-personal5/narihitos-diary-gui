use super::*;

const A: &str = "07249746-e280-4d51-a86a-cbcb198ee101";

fn identity() -> Identity {
    Identity {
        workspace_id: "w".into(),
        root: "/r".into(),
        native: "1:2".into(),
        adapter_version: 1,
    }
}
fn record(kind: &str, identity: Identity) -> Record {
    Record {
        id: OperationId::parse(A).unwrap(),
        kind: kind.into(),
        hash_version: 1,
        input_hash: "0".repeat(64),
        state: OperationState::Paused,
        checkpoint: 0,
        result: None,
        identity,
    }
}

#[test]
fn ids_and_keys_are_validated() {
    assert!(OperationId::parse(A).is_some());
    assert!(
        StepId::parse("2026-11-05").is_none(),
        "a date is not a UUID"
    );
    assert!(OperationId::parse(&A.to_uppercase()).is_none());
    assert!(ResourceKey::parse("authentic:2026-11-05").is_some());
    assert!(ResourceKey::parse("").is_none());
    assert!(ResourceKey::parse("a b").is_none());
    assert!(ResourceKey::parse(&"a".repeat(257)).is_none());
}

#[test]
fn states_round_trip_and_terminal_set_is_exact() {
    for s in [
        "running",
        "paused",
        "uncertain",
        "succeeded",
        "failed",
        "cancelled",
    ] {
        assert_eq!(OperationState::parse(s).unwrap().as_str(), s);
    }
    assert!(OperationState::parse("done").is_none());
    let terminal: Vec<_> = [
        "running",
        "paused",
        "uncertain",
        "succeeded",
        "failed",
        "cancelled",
    ]
    .into_iter()
    .filter(|s| OperationState::parse(s).unwrap().is_terminal())
    .collect();
    assert_eq!(terminal, ["succeeded", "failed", "cancelled"]);
}

#[test]
fn classification_binds_to_workspace_root_and_adapter() {
    let cur = identity();
    let class = |r: Record| classify(&[r], &cur, &["save"])[0].class.clone();
    assert_eq!(class(record("save", identity())), Class::Resume);
    let mut moved = identity();
    moved.root = "/elsewhere".into();
    assert_eq!(
        class(record("save", moved)),
        Class::Blocked(Blocker::RootChanged)
    );
    let mut reused = identity();
    reused.native = "9:9".into();
    assert_eq!(
        class(record("save", reused)),
        Class::Blocked(Blocker::RootChanged)
    );
    let mut other = identity();
    other.workspace_id = "x".into();
    assert_eq!(
        class(record("save", other)),
        Class::Blocked(Blocker::RootChanged)
    );
    let mut old = identity();
    old.adapter_version = 0;
    assert_eq!(
        class(record("save", old)),
        Class::Blocked(Blocker::AdapterChanged)
    );
    assert_eq!(
        class(record("export", identity())),
        Class::Blocked(Blocker::NoReconciler)
    );
}

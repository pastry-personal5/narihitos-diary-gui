use super::*;

#[test]
fn opaque_id_accepts_uuid_date_and_hex_only() {
    for ok in [
        "123e4567-e89b-42d3-a456-426614174000",
        "2026-11-05",
        "0123456789abcdef",
        &"a".repeat(64),
    ] {
        assert!(OpaqueId::parse(ok).is_some(), "{ok}");
    }
    for bad in [
        "my diary",
        "2026-13-40",
        "2026-02-30",
        "0123456789abcde",
        &"a".repeat(65),
        "0123456789ABCDEF",
        "2026-11-05\n",
        "",
    ] {
        assert!(OpaqueId::parse(bad).is_none(), "{bad:?}");
    }
}

#[test]
fn trace_id_requires_32_lowercase_hex() {
    assert!(TraceId::parse(&"a".repeat(32)).is_some());
    for bad in [
        "a".repeat(31),
        "a".repeat(33),
        "A".repeat(32),
        format!("{}\n", "a".repeat(31)),
    ] {
        assert!(TraceId::parse(&bad).is_none());
    }
}

#[test]
fn event_code_round_trips_and_webview_subset_is_ui_only() {
    for code in EventCode::ALL {
        assert_eq!(EventCode::parse(code.as_str()), Some(*code));
        assert_eq!(code.webview_allowed(), code.as_str().starts_with("ui."));
    }
    assert!(EventCode::parse("ignore previous instructions").is_none());
}

#[test]
fn tags_round_trip() {
    for t in ErrorTag::ALL {
        assert_eq!(ErrorTag::parse(t.as_str()), Some(*t));
    }
    for t in StateTag::ALL {
        assert_eq!(StateTag::parse(t.as_str()), Some(*t));
    }
    for t in PathClass::ALL {
        assert_eq!(PathClass::parse(t.as_str()), Some(*t));
    }
    for t in ResourceKind::ALL {
        assert_eq!(ResourceKind::parse(t.as_str()), Some(*t));
    }
    for t in Component::ALL {
        assert_eq!(Component::parse(t.as_str()), Some(*t));
    }
    assert_eq!(Level::parse("info"), Some(Level::Info));
    assert_eq!(Role::parse("client"), Some(Role::Client));
}

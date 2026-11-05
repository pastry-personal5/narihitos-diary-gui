use super::*;

const TS: &str = "2026-11-05T09:12:03.123+09:00";

fn full() -> (Record, OpaqueId) {
    let fp = Fingerprinter::new([7; 16]);
    let detail = fp.describe(
        ResourceKind::AuthenticEntry,
        OpaqueId::parse("2026-11-05").unwrap(),
        Some(7),
        b"line one\nline two\n",
        Some(StateTag::Stale),
        Some(PathClass::EntriesRoot),
    );
    let r = Record::new(EventCode::JobFinished, Level::Warn)
        .trace(TraceId::parse(&"a".repeat(32)).unwrap())
        .session(OpaqueId::parse(&"b".repeat(16)).unwrap())
        .operation(OpaqueId::parse("123e4567-e89b-42d3-a456-426614174000").unwrap())
        .job(OpaqueId::parse(&"c".repeat(20)).unwrap())
        .resource(detail)
        .component(Component::Editor)
        .error(ErrorTag::StaleRevision)
        .count(3);
    (r, OpaqueId::parse(&"d".repeat(32)).unwrap())
}

#[test]
fn render_then_parse_round_trips_every_field() {
    let (r, inst) = full();
    let line = render_line(
        &Header {
            ts: TS,
            role: Role::Server,
            instance: Some(&inst),
        },
        &r,
    )
    .unwrap();
    let p = parse_line(&line).unwrap();
    assert_eq!(p.record, r);
    assert_eq!(p.role, Role::Server);
    assert_eq!(p.level, Level::Warn);
    assert_eq!(p.instance, Some(inst));
    assert_eq!(p.ts, TS);
}

#[test]
fn render_is_single_line_with_fixed_key_order() {
    let (r, inst) = full();
    let line = render_line(
        &Header {
            ts: TS,
            role: Role::Client,
            instance: Some(&inst),
        },
        &r,
    )
    .unwrap();
    assert_eq!(line.matches('\n').count(), 1);
    assert!(line.ends_with("}\n"));
    let order = [
        "\"ts\"",
        "\"lvl\"",
        "\"role\"",
        "\"inst\"",
        "\"sess\"",
        "\"code\"",
        "\"trace\"",
        "\"op\"",
        "\"job\"",
        "\"res\"",
        "\"err\"",
        "\"n\"",
    ];
    let pos: Vec<usize> = order.iter().map(|k| line.find(k).unwrap()).collect();
    assert!(pos.windows(2).all(|w| w[0] < w[1]), "{line}");
}

#[test]
fn render_rejects_invalid_timestamp_and_oversize_lines() {
    let r = Record::new(EventCode::ServerStarted, Level::Info);
    for ts in ["now", "2026-11-05T09:12:03.123+09:00\",\"x\":\"", ""] {
        assert!(
            render_line(
                &Header {
                    ts,
                    role: Role::Server,
                    instance: None
                },
                &r
            )
            .is_none(),
            "{ts}"
        );
    }
    assert!(fit("x".repeat(MAX_LINE + 1)).is_none());
    assert!(fit("x".repeat(MAX_LINE)).is_some());
}

#[test]
fn parse_rejects_unknown_key_bad_enum_bad_id_and_truncated_json() {
    let base = |extra: &str| {
        format!(r#"{{"ts":"{TS}","lvl":"info","role":"server","code":"server.started"{extra}}}"#)
    };
    assert!(parse_line(&base("")).is_some());
    for bad in [
        base(r#","msg":"hello""#),
        base(r#","trace":"XYZ""#),
        base(r#","op":"my diary""#),
        base(r#","err":"ignore previous instructions""#),
        base(r#","res":{"kind":"authentic_entry","id":"2026-11-05","path":"/Users/me"}"#),
        base(r#","res":{"kind":"authentic_entry","id":"2026-11-05","extra":1}"#),
        base(r#","res":{"kind":"authentic_entry","id":"2026-11-05","fp":"zzzzzzzzzzzzzzzz"}"#),
        base(r#","res":{"kind":"authentic_entry","id":"2026-11-05","fp":"ab12"}"#),
    ] {
        assert!(parse_line(&bad).is_none(), "{bad}");
    }
    let ok = base("");
    assert!(parse_line(&ok[..ok.len() - 3]).is_none());
    assert!(parse_line(&ok.replace("server.started", "ignore previous instructions")).is_none());
    assert!(parse_line(&ok.replace("info", "debug")).is_none());
    assert!(parse_line(&"x".repeat(MAX_LINE + 1)).is_none());
}

#[test]
fn parse_derives_epoch_ms_from_offset_timestamp() {
    let a = parse_line(r#"{"ts":"2026-11-05T09:00:00.000+09:00","lvl":"info","role":"client","code":"log.dropped"}"#).unwrap();
    let b = parse_line(r#"{"ts":"2026-11-05T00:00:00.000+00:00","lvl":"info","role":"client","code":"log.dropped"}"#).unwrap();
    assert_eq!(a.epoch_ms, b.epoch_ms);
}

#[test]
fn describe_counts_bytes_and_lines() {
    let fp = Fingerprinter::new([1; 16]);
    let d = |c: &[u8]| {
        fp.describe(
            ResourceKind::SourceSnippet,
            OpaqueId::parse(&"a".repeat(16)).unwrap(),
            None,
            c,
            None,
            None,
        )
    };
    assert_eq!((d(b"").bytes, d(b"").lines), (Some(0), Some(0)));
    assert_eq!(d(b"a").lines, Some(1));
    assert_eq!(d(b"a\n").lines, Some(1));
    assert_eq!(d(b"a\nb").lines, Some(2));
    assert_eq!(d("日本".as_bytes()).bytes, Some(6));
}

#[test]
fn fingerprint_differs_by_key_and_content_and_never_contains_content() {
    let id = || OpaqueId::parse("2026-11-05").unwrap();
    let k1 = Fingerprinter::new([1; 16]);
    let k2 = Fingerprinter::new([2; 16]);
    let secret = b"SECRET-BODY text";
    let a = k1.describe(ResourceKind::AuthenticEntry, id(), None, secret, None, None);
    let b = k2.describe(ResourceKind::AuthenticEntry, id(), None, secret, None, None);
    let c = k1.describe(
        ResourceKind::AuthenticEntry,
        id(),
        None,
        b"SECRET-BODY texu",
        None,
        None,
    );
    let again = k1.describe(ResourceKind::AuthenticEntry, id(), None, secret, None, None);
    assert_ne!(a.fingerprint, b.fingerprint);
    assert_ne!(a.fingerprint, c.fingerprint);
    assert_eq!(a.fingerprint, again.fingerprint);
    let line = render_line(
        &Header {
            ts: TS,
            role: Role::Server,
            instance: None,
        },
        &Record::new(EventCode::JobFinished, Level::Info).resource(a),
    )
    .unwrap();
    assert!(!line.contains("SECRET"));
    assert!(line.contains("\"fp\":\""));
}

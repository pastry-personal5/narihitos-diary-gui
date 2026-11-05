use super::*;

const ID: &str = "0b9f1e2a-3c4d-4e5f-8a6b-7c8d9e0f1a2b";

fn header() -> AuthenticHeader {
    AuthenticHeader {
        id: ID.into(),
        date: date(),
        timezone: "Asia/Seoul".into(),
        timezone_database: "2026e".into(),
        created_at: "2026-11-05T10:00:00Z".into(),
        updated_at: "2026-11-05T10:00:00Z".into(),
        revision: 1,
        extensions: vec![],
    }
}

const GOLDEN: &str = "---\nschema_version: 1\nasset_type: diary\nkind: authentic\nid: \"0b9f1e2a-3c4d-4e5f-8a6b-7c8d9e0f1a2b\"\ndiary_date: \"2026-11-05\"\ndiary_timezone: \"Asia/Seoul\"\ntimezone_database: \"2026e\"\ncreated_at: \"2026-11-05T10:00:00Z\"\nupdated_at: \"2026-11-05T10:00:00Z\"\nrevision: 1\n---\nHello\n";

fn date() -> DiaryDate {
    DiaryDate::parse("2026-11-05").unwrap()
}

#[test]
fn golden_file_round_trips_byte_identically() {
    let bytes = render(&header(), "Hello\n").unwrap();
    assert_eq!(bytes, GOLDEN.as_bytes());
    let (h, body) = parse(&bytes, &date()).unwrap();
    assert_eq!(h, header());
    assert_eq!(render(&h, &body).unwrap(), bytes);
}

#[test]
fn empty_body_and_unicode_round_trip() {
    for body in ["", "한국어 e\u{301} 😀\n\n  x"] {
        let bytes = render(&header(), body).unwrap();
        assert_eq!(parse(&bytes, &date()).unwrap().1, body);
    }
}

#[test]
fn scalar_extensions_survive() {
    let mut h = header();
    h.extensions = vec![
        ("mood".into(), Scalar::Str("a \"b\" \\ c".into())),
        ("score".into(), Scalar::Int(-7)),
        ("pinned".into(), Scalar::Bool(true)),
    ];
    let bytes = render(&h, "x").unwrap();
    let (back, _) = parse(&bytes, &date()).unwrap();
    assert_eq!(back.extensions, h.extensions);
    assert_eq!(render(&back, "x").unwrap(), bytes);
}

fn with_ext(line: &str) -> Vec<u8> {
    GOLDEN
        .replacen("revision: 1\n", &format!("revision: 1\n{line}\n"), 1)
        .into_bytes()
}

#[test]
fn unsupported_yaml_is_rejected_never_repaired() {
    for line in [
        "a: &x 1",
        "b: *x",
        "c: !!str d",
        "d: [1, 2]",
        "e: {f: 1}",
        "g: |",
        "h: bare words",
        "mood: \"x\"\nmood: \"y\"",
        "id: \"dup\"",
        "revision: 2",
        "  indented: 1",
        "tilde: ~",
        "num: 01",
    ] {
        assert!(parse(&with_ext(line), &date()).is_err(), "{line}");
    }
}

#[test]
fn bad_files_are_rejected() {
    let ok = GOLDEN;
    let cases: Vec<(Vec<u8>, FormatError)> = vec![
        (b"\xEF\xBB\xBF---\n".to_vec(), FormatError::ByteOrderMark),
        (vec![0xFF, 0xFE], FormatError::InvalidUtf8),
        (
            ok.replace('\n', "\r\n").into_bytes(),
            FormatError::CarriageReturn,
        ),
        (b"no header".to_vec(), FormatError::MissingHeader),
        (
            ok.replace("schema_version: 1", "schema_version: 2")
                .into_bytes(),
            FormatError::UnknownSchema,
        ),
        (
            ok.replace(
                "2026-11-05\"\ndiary_timezone",
                "2026-11-06\"\ndiary_timezone",
            )
            .into_bytes(),
            FormatError::DateMismatch,
        ),
        (
            ok.replace(
                "2026-11-05\"\ndiary_timezone",
                "2026-02-30\"\ndiary_timezone",
            )
            .into_bytes(),
            FormatError::BadValue("diary_date"),
        ),
        (
            ok.replace("kind: authentic", "kind: candidate")
                .into_bytes(),
            FormatError::BadValue("kind"),
        ),
        (
            ok.replace("revision: 1", "revision: 0").into_bytes(),
            FormatError::BadValue("revision"),
        ),
    ];
    for (bytes, expected) in cases {
        assert_eq!(parse(&bytes, &date()).unwrap_err(), expected);
    }
    // field order is fixed
    let swapped = ok
        .replace("kind: authentic\nid", "id")
        .replace("diary_date", "kind: authentic\ndiary_date");
    assert!(parse(swapped.as_bytes(), &date()).is_err());
}

#[test]
fn body_limit_is_one_mebibyte() {
    let at = "a".repeat(MAX_BODY_BYTES);
    let bytes = render(&header(), &at).unwrap();
    assert!(parse(&bytes, &date()).is_ok());
    assert_eq!(
        render(&header(), &format!("{at}a")),
        Err(FormatError::BodyTooLarge)
    );
    assert_eq!(
        render(&header(), "a\r\nb"),
        Err(FormatError::CarriageReturn)
    );
}

#[test]
fn dates_are_gregorian() {
    assert!(DiaryDate::parse("2028-02-29").is_some());
    for bad in [
        "2026-02-29",
        "2026-13-01",
        "2026-00-10",
        "2026-1-01",
        "2026-11-5x",
        "2026/11/05",
        "+026-11-05",
    ] {
        assert!(DiaryDate::parse(bad).is_none(), "{bad}");
    }
    assert!(DiaryDate::parse("1900-02-29").is_none());
    assert!(DiaryDate::parse("2000-02-29").is_some());
}

#[test]
fn render_rejects_invalid_headers() {
    let mut h = header();
    h.revision = 0;
    assert!(render(&h, "").is_err());
    let mut h = header();
    h.id = "not-a-uuid".into();
    assert!(render(&h, "").is_err());
    let mut h = header();
    h.extensions = vec![("id".into(), Scalar::Int(1))];
    assert!(render(&h, "").is_err());
}

#[test]
fn multibyte_instant_is_a_format_error_not_a_panic() {
    // 20 bytes with a character straddling byte 10.
    let bad = GOLDEN.replace(
        "2026-11-05T10:00:00Z\"\nupdated",
        "aaaaaaaaaé000000000\"\nupdated",
    );
    assert_eq!(
        parse(bad.as_bytes(), &date()),
        Err(FormatError::BadValue("created_at"))
    );
}

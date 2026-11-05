use super::*;

const ID: &str = "0b1d2c3e-4f50-4a6b-8c7d-9e0f1a2b3c4d";
pub(crate) const REL: &str =
    "2024/02/29/2024-02-29_01-02-03-0b1d2c3e-4f50-4a6b-8c7d-9e0f1a2b3c4d.md";

pub(crate) fn golden(updated: &str, body: &str) -> Vec<u8> {
    format!(
        "---\nschema_version: 1\nkind: \"ndm.diary-snippet\"\nid: \"{ID}\"\ncreated_at: \"2024-02-29T01:02:03Z\"\nupdated_at: \"{updated}\"\ntitle: \"Hello\"\ntitle_origin: \"first-sentence\"\nfavorite: false\nlanguage: \"en\"\naudio_filename: \"{ID}.m4a\"\nrecording_duration_ms: 1000\naudio_codec: \"aac\"\naudio_sample_rate_hz: 44100\naudio_channels: 1\naudio_bitrate_bps: 64000\ntranscription_method_id: \"ndm.transcription.apple\"\ntranscription_language_hint: \"en\"\ntranscription_settings_revision: 1\nunknown_flag: 7\n---\n\n{body}"
    )
    .into_bytes()
}
fn edit(from: &str, to: &str) -> Vec<u8> {
    String::from_utf8(golden("2024-02-29T01:02:03Z", "hi\n"))
        .unwrap()
        .replacen(from, to, 1)
        .into_bytes()
}

#[test]
fn accepts_golden_document() {
    let c = check(&golden("2024-02-29T01:02:03Z", "hi\n"), REL).unwrap();
    assert_eq!(
        (c.updated_at.as_str(), c.body.as_str()),
        ("2024-02-29T01:02:03Z", "hi\n")
    );
    assert!(check(&golden("2024-02-29T01:02:03Z", ""), REL).is_ok());
}

#[test]
fn rejects_each_producer_violation() {
    let upd = "updated_at: \"2024-02-29T01:02:03Z\"";
    let bad: Vec<(&str, Vec<u8>)> = vec![
        (
            "bom",
            [
                b"\xEF\xBB\xBF".as_slice(),
                &golden("2024-02-29T01:02:03Z", ""),
            ]
            .concat(),
        ),
        ("cr in body", golden("2024-02-29T01:02:03Z", "a\r\n")),
        ("no separator", edit("---\n\nhi", "---\nhi")),
        ("schema", edit("schema_version: 1", "schema_version: 2")),
        ("kind", edit("ndm.diary-snippet", "other")),
        (
            "method",
            edit("ndm.transcription.apple", "ndm.transcription.x"),
        ),
        ("origin", edit("first-sentence", "magic")),
        (
            "updated_at",
            edit(upd, "updated_at: \"2024-02-29T01:02:03.5Z\""),
        ),
        (
            "not a leap year",
            edit(upd, "updated_at: \"2023-02-29T01:02:03Z\""),
        ),
        (
            "id differs",
            edit(
                &format!("id: \"{ID}\""),
                "id: \"0b1d2c3e-4f50-4a6b-8c7d-9e0f1a2b3c4e\"",
            ),
        ),
        ("audio", edit(&format!("{ID}.m4a"), "x.m4a")),
        (
            "title length",
            edit("\"Hello\"", &format!("\"{}\"", "x".repeat(31))),
        ),
        ("list", edit("unknown_flag: 7", "unknown_flag: [1]")),
        ("single quote", edit("title: \"Hello\"", "title: 'Hello'")),
        ("indent", edit("unknown_flag: 7", " unknown_flag: 7")),
        ("underscore key", edit("unknown_flag", "_flag")),
        ("digit key", edit("unknown_flag", "9flag")),
        ("bare word", edit("unknown_flag: 7", "unknown_flag: word")),
        ("missing key", edit("language: \"en\"\n", "")),
        ("duplicate", edit("unknown_flag: 7", "language: \"en\"")),
        (
            "origin extras",
            edit("favorite: false", "favorite: false\ntitle_model_id: \"m\""),
        ),
        ("blank line", edit("favorite: false", "\nfavorite: false")),
        ("bad escape", edit("\"Hello\"", "\"a\\qb\"")),
        (
            "bad sha",
            edit("favorite: false", "favorite: false\naudio_sha256: \"ABC\""),
        ),
        (
            "duration",
            edit("recording_duration_ms: 1000", "recording_duration_ms: 301001"),
        ),
        (
            "settings revision",
            edit(
                "transcription_settings_revision: 1",
                "transcription_settings_revision: 2",
            ),
        ),
        ("rel mismatch", golden("2024-02-29T01:02:03Z", "")),
    ];
    for (name, bytes) in bad {
        let rel = if name == "rel mismatch" {
            "2024/02/29/other.md"
        } else {
            REL
        };
        assert!(check(&bytes, rel).is_err(), "{name}");
    }
    for n in [
        "01",
        "+1",
        "-0",
        "1.",
        ".5",
        "1e3",
        &"9".repeat(39),
        &format!("0.{}", "1".repeat(38)),
    ] {
        let bytes = edit("unknown_flag: 7", &format!("unknown_flag: {n}"));
        assert!(check(&bytes, REL).is_err(), "{n}");
    }
    for n in ["0", "-7", "0.5", "-12.25"] {
        let bytes = edit("unknown_flag: 7", &format!("unknown_flag: {n}"));
        assert!(check(&bytes, REL).is_ok(), "{n}");
    }
}

#[test]
fn apple_ai_titles_need_their_provenance() {
    assert!(check(&edit("first-sentence", "apple-ai"), REL).is_err());
    let full = edit(
        "favorite: false",
        "favorite: false\ntitle_prompt_revision: \"1\"\ntitle_os_version: \"26\"",
    );
    assert!(
        check(&full, REL).is_err(),
        "provenance with a non-apple origin"
    );
    let full = String::from_utf8(full)
        .unwrap()
        .replace("first-sentence", "apple-ai");
    assert!(check(full.as_bytes(), REL).is_ok());
}

#[test]
fn splice_changes_only_body_and_updated_at() {
    let old = golden("2024-02-29T01:02:03Z", "hi\n");
    let out = splice(&old, REL, "new\nbody", 1_709_168_000).unwrap();
    let (a, b) = (
        String::from_utf8(old).unwrap(),
        String::from_utf8(out).unwrap(),
    );
    // Now (1709168000) is before the previous stamp (…523), so the stamp moves by one second.
    assert!(b.contains("updated_at: \"2024-02-29T01:02:04Z\""));
    let front = |s: &str| s.split("\n---\n\n").next().unwrap().to_owned();
    assert_eq!(
        front(&a).replace("01:02:03Z\"\ntitle", "01:02:04Z\"\ntitle"),
        front(&b)
    );
    assert!(b.ends_with("---\n\nnew\nbody"));
}

#[test]
fn splice_leaves_title_and_origin_untouched() {
    let old = edit("first-sentence", "empty-body");
    let out = splice(&old, REL, "x", 9_999_999_999).unwrap();
    let text = String::from_utf8(out).unwrap();
    assert!(text.contains("title: \"Hello\"\ntitle_origin: \"empty-body\"\n"));
}

#[test]
fn splice_of_equal_body_returns_input() {
    let old = golden("2024-02-29T01:02:03Z", "hi\n");
    assert_eq!(splice(&old, REL, "hi\n", 1).unwrap(), old);
}

#[test]
fn updated_at_is_strictly_later() {
    let prev = "2024-02-29T01:02:03Z";
    assert_eq!(
        next_updated_at(prev, 2_000_000_000).unwrap(),
        "2033-05-18T03:33:20Z"
    );
    assert_eq!(
        next_updated_at(prev, 1_709_168_523).unwrap(),
        "2024-02-29T01:02:04Z"
    );
    assert_eq!(next_updated_at(prev, 1).unwrap(), "2024-02-29T01:02:04Z");
    let carry = [
        ("2024-02-29T01:02:59Z", "2024-02-29T01:03:00Z"),
        ("2024-02-29T23:59:59Z", "2024-03-01T00:00:00Z"),
        ("2023-12-31T23:59:59Z", "2024-01-01T00:00:00Z"),
        ("2023-02-28T23:59:59Z", "2023-03-01T00:00:00Z"),
    ];
    for (a, b) in carry {
        assert_eq!(next_updated_at(a, 0).unwrap(), b);
    }
    assert!(next_updated_at("nope", 0).is_err());
}

#[test]
fn separator_restored_when_first_line_deleted() {
    let old = golden("2024-02-29T01:02:03Z", "first\nsecond");
    let out = splice(&old, REL, "second", 2_000_000_000).unwrap();
    assert_eq!(check(&out, REL).unwrap().body, "second");
    assert!(String::from_utf8(out).unwrap().contains("\n---\n\nsecond"));
}

#[test]
fn rejects_body_with_cr_or_over_1mib() {
    let old = golden("2024-02-29T01:02:03Z", "");
    assert_eq!(splice(&old, REL, "a\rb", 1), Err(Unwritable::Body));
    assert_eq!(
        splice(&old, REL, &"x".repeat((1 << 20) + 1), 1),
        Err(Unwritable::Body)
    );
    assert!(splice(&old, REL, &"x".repeat(1 << 20), 2_000_000_000).is_ok());
}

#[test]
fn output_revalidates() {
    let old = golden("2024-02-29T01:02:03Z", "");
    let body = "---\n\nlooks like a close\n😀";
    let out = splice(&old, REL, body, 2_000_000_000).unwrap();
    assert_eq!(check(&out, REL).unwrap().body, body);
}

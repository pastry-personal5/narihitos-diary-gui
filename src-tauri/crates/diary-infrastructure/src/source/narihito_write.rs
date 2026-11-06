//! The producer-strict write gate and the body splice for Narihito mobile snippets.
//!
//! Soundness is the property: when `check` accepts bytes, the pinned producer decoder
//! (`DiarySnippetMarkdownCodec.decode`) reads them as a supported snippet. The gate may be
//! stricter than the decoder, never looser.
use crate::workspace::utc;
use diary_domain::source::front_matter::{MAX_BODY, MAX_FRONT};

/// Why bytes or a body cannot be written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Unwritable {
    /// The file is not one the producer would decode (`unsupported_format`).
    Format,
    /// The proposed body has a CR or is over 1 MiB (`invalid_body`).
    Body,
}

pub(crate) struct Checked {
    pub id: String,
    pub audio_sha256: Option<String>,
    pub updated_at: String,
    /// The editor view: the text after the producer's blank separator line.
    pub body: String,
}

const CLOSE: &str = "\n---\n\n";
const METHODS: [&str; 2] = ["ndm.transcription.apple", "ndm.transcription.cohere"];
const ORIGINS: [&str; 3] = ["apple-ai", "first-sentence", "empty-body"];
const REQUIRED: [&str; 18] = [
    "schema_version",
    "kind",
    "id",
    "created_at",
    "updated_at",
    "title",
    "title_origin",
    "favorite",
    "language",
    "audio_filename",
    "recording_duration_ms",
    "audio_codec",
    "audio_sample_rate_hz",
    "audio_channels",
    "audio_bitrate_bps",
    "transcription_method_id",
    "transcription_language_hint",
    "transcription_settings_revision",
];
const OPTIONAL_STRINGS: [&str; 8] = [
    "audio_sha256",
    "transcription_model_id",
    "transcription_model_version",
    "transcription_runtime_id",
    "transcription_conversion_id",
    "title_prompt_revision",
    "title_os_version",
    "title_model_id",
];

enum Scalar {
    Str(String),
    Int(i64),
    Bool,
    Decimal,
}

fn valid_key(k: &str) -> bool {
    let b = k.as_bytes();
    b.first().is_some_and(u8::is_ascii_lowercase)
        && b.iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'_')
}

/// Combining and joining characters that Swift would fuse with a neighbouring quote or
/// backslash into one `Character`. shortcut: a fixed list of common ranges, not the full
/// grapheme tables; upgrade if a real title is refused for it.
fn extends(c: char) -> bool {
    matches!(c as u32, 0x300..=0x36F | 0x3099..=0x309A | 0x200C..=0x200D | 0xFE00..=0xFE0F
        | 0x1F3FB..=0x1F3FF | 0xE0100..=0xE01EF)
}

fn unquote(raw: &str) -> Option<String> {
    let inner = raw.strip_prefix('"')?.strip_suffix('"')?;
    let mut out = String::new();
    let mut chars = inner.chars().peekable();
    if chars.peek().copied().is_some_and(extends) {
        return None;
    }
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                out.push(match chars.next()? {
                    '"' => '"',
                    '\\' => '\\',
                    'n' => '\n',
                    'r' => '\r',
                    't' => '\t',
                    _ => return None,
                });
                if chars.peek().copied().is_some_and(extends) {
                    return None;
                }
            }
            '"' | '\u{2028}' | '\u{2029}' => return None,
            c if c.is_control() => return None,
            c => out.push(c),
        }
    }
    Some(out)
}

fn scalar(raw: &str) -> Option<Scalar> {
    if raw.len() > 8192 {
        return None;
    }
    if raw.starts_with('"') {
        return unquote(raw).map(Scalar::Str);
    }
    match raw {
        "true" | "false" => return Some(Scalar::Bool),
        _ => {}
    }
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    let plain = |s: &str| s == "0" || (digits(s) && !s.starts_with('0'));
    let unsigned = raw.strip_prefix('-').unwrap_or(raw);
    if let Some((whole, frac)) = unsigned.split_once('.') {
        let count = raw.bytes().filter(u8::is_ascii_digit).count();
        return (plain(whole) && digits(frac) && count <= 38).then_some(Scalar::Decimal);
    }
    // "-0" and any leading zero are refused: the producer round-trips `Int64` spelling.
    if !plain(unsigned) || raw == "-0" {
        return None;
    }
    raw.parse().ok().map(Scalar::Int)
}

fn leap(y: i64) -> bool {
    y % 4 == 0 && (y % 100 != 0 || y % 400 == 0)
}

/// Seconds since the epoch of a strict `YYYY-MM-DDTHH:MM:SSZ`, or `None`.
fn instant_secs(s: &str) -> Option<u64> {
    let b = s.as_bytes();
    let separators = [
        (4, b'-'),
        (7, b'-'),
        (10, b'T'),
        (13, b':'),
        (16, b':'),
        (19, b'Z'),
    ];
    if b.len() != 20 || separators.iter().any(|&(i, c)| b[i] != c) {
        return None;
    }
    let n = |r: std::ops::Range<usize>| -> Option<i64> {
        b[r].iter().try_fold(0i64, |acc, d| {
            d.is_ascii_digit().then(|| acc * 10 + i64::from(d - b'0'))
        })
    };
    let (y, mo, d) = (n(0..4)?, n(5..7)?, n(8..10)?);
    let (h, mi, se) = (n(11..13)?, n(14..16)?, n(17..19)?);
    let month_days = [
        31,
        if leap(y) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    if !(1970..=9999).contains(&y)
        || !(1..=12).contains(&mo)
        || d < 1
        || d > month_days[(mo - 1) as usize]
        || h > 23
        || mi > 59
        || se > 59
    {
        return None;
    }
    // Hinnant's days-from-civil.
    let y2 = if mo <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let doy = (153 * (if mo > 2 { mo - 3 } else { mo + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    u64::try_from(days * 86_400 + h * 3600 + mi * 60 + se).ok()
}

/// The `updated_at` for a save: `now` if later than `previous`, else `previous` + 1 s.
pub(crate) fn next_updated_at(previous: &str, now_secs: u64) -> Result<String, Unwritable> {
    let prev = instant_secs(previous).ok_or(Unwritable::Format)?;
    Ok(utc(if now_secs > prev { now_secs } else { prev + 1 }))
}

/// The path the producer's layout gives a snippet.
fn expected_rel(created_at: &str, id: &str) -> String {
    format!(
        "{}/{}/{}/{}_{}-{id}.md",
        &created_at[..4],
        &created_at[5..7],
        &created_at[8..10],
        &created_at[..10],
        created_at[11..19].replace(':', "-")
    )
}

fn uuid_v4(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 36
        && b.iter().enumerate().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => *c == b'-',
            _ => c.is_ascii_digit() || (b'a'..=b'f').contains(c),
        })
        && b[14] == b'4'
        && matches!(b[19], b'8' | b'9' | b'a' | b'b')
}

/// Accepts only bytes the producer decodes as a supported snippet stored at `rel`.
pub(crate) fn check(bytes: &[u8], rel: &str) -> Result<Checked, Unwritable> {
    use Unwritable::Format as F;
    if bytes.len() > MAX_FRONT + MAX_BODY + 10 || bytes.contains(&b'\r') {
        return Err(F);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| F)?;
    let rest = text.strip_prefix("---\n").ok_or(F)?;
    let at = rest.find(CLOSE).ok_or(F)?;
    let (front, body) = (&rest[..at], &rest[at + CLOSE.len()..]);
    if front.len() > MAX_FRONT || body.len() > MAX_BODY {
        return Err(F);
    }
    let lines: Vec<&str> = if front.is_empty() {
        vec![]
    } else {
        front.split('\n').collect()
    };
    if lines.len() > 64 {
        return Err(F);
    }
    let mut entries: Vec<(&str, Scalar)> = Vec::new();
    for line in lines {
        let (key, raw) = line.split_once(": ").ok_or(F)?;
        if !valid_key(key) || entries.iter().any(|(k, _)| *k == key) {
            return Err(F);
        }
        entries.push((key, scalar(raw).ok_or(F)?));
    }
    let get = |k: &str| entries.iter().find(|(n, _)| *n == k).map(|(_, v)| v);
    if REQUIRED.iter().any(|k| get(k).is_none()) {
        return Err(F);
    }
    let string = |k: &str| match get(k) {
        Some(Scalar::Str(s)) => Ok(s.as_str()),
        _ => Err(F),
    };
    let int = |k: &str| match get(k) {
        Some(Scalar::Int(i)) => Ok(*i),
        _ => Err(F),
    };
    if int("schema_version")? != 1
        || string("kind")? != "ndm.diary-snippet"
        || !matches!(get("favorite"), Some(Scalar::Bool))
    {
        return Err(F);
    }
    let method = string("transcription_method_id")?;
    let origin = string("title_origin")?;
    if !METHODS.contains(&method) || !ORIGINS.contains(&origin) {
        return Err(F);
    }
    let (id, created, updated) = (string("id")?, string("created_at")?, string("updated_at")?);
    if !uuid_v4(id)
        || instant_secs(created).is_none()
        || instant_secs(updated).is_none()
        || rel != expected_rel(created, id)
        || string("audio_filename")? != format!("{id}.m4a")
    {
        return Err(F);
    }
    if let Some(v) = get("audio_sha256") {
        let Scalar::Str(s) = v else { return Err(F) };
        if s.len() != 64
            || !s
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(F);
        }
    }
    let revision = int("transcription_settings_revision")?;
    if !(0..=301_000).contains(&int("recording_duration_ms")?)
        || int("audio_sample_rate_hz")? <= 0
        || int("audio_channels")? <= 0
        || int("audio_bitrate_bps")? <= 0
        || !(revision == 1 || (revision == 2 && method == "ndm.transcription.cohere"))
        // shortcut: counts chars, not graphemes, so it may refuse a valid title; upgrade if
        // title regeneration is ever approved.
        || string("title")?.chars().count() > 30
    {
        return Err(F);
    }
    string("language")?;
    string("audio_codec")?;
    string("transcription_language_hint")?;
    if OPTIONAL_STRINGS
        .iter()
        .any(|k| get(k).is_some_and(|v| !matches!(v, Scalar::Str(_))))
    {
        return Err(F);
    }
    let provenance = ["title_prompt_revision", "title_os_version"];
    if origin == "apple-ai" {
        if provenance.iter().any(|k| get(k).is_none()) {
            return Err(F);
        }
    } else if provenance
        .iter()
        .chain(&["title_model_id"])
        .any(|k| get(k).is_some())
    {
        return Err(F);
    }
    Ok(Checked {
        id: id.to_owned(),
        audio_sha256: get("audio_sha256").and_then(|v| match v {
            Scalar::Str(s) => Some(s.clone()),
            _ => None,
        }),
        updated_at: updated.to_owned(),
        body: body.to_owned(),
    })
}

/// A body is writable when it has no CR and, with the separator newline the reader counts,
/// still fits the 1 MiB read cap (so the GUI can always re-read what it wrote; one byte under
/// the producer's own cap).
pub(crate) fn body_ok(body: &str) -> Result<(), Unwritable> {
    if body.len() >= MAX_BODY || body.contains('\r') {
        Err(Unwritable::Body)
    } else {
        Ok(())
    }
}

/// Rewrites only the `updated_at` line and the text after the blank separator. An equal body
/// returns the input unchanged. The result passes `check` before it is returned.
pub(crate) fn splice(
    bytes: &[u8],
    rel: &str,
    body: &str,
    now_secs: u64,
) -> Result<Vec<u8>, Unwritable> {
    body_ok(body)?;
    let current = check(bytes, rel)?;
    if current.body == body {
        return Ok(bytes.to_vec());
    }
    let next = next_updated_at(&current.updated_at, now_secs)?;
    let text = std::str::from_utf8(bytes).map_err(|_| Unwritable::Format)?;
    let split = text.find(CLOSE).ok_or(Unwritable::Format)? + CLOSE.len();
    let mut out = String::with_capacity(text.len() + body.len());
    for part in text[..split].split_inclusive('\n') {
        if part.starts_with("updated_at: ") {
            out.push_str(&format!("updated_at: \"{next}\"\n"));
        } else {
            out.push_str(part);
        }
    }
    out.push_str(body);
    let out = out.into_bytes();
    check(&out, rel)?;
    Ok(out)
}

#[cfg(test)]
mod tests;
/// Fixtures shared with the adapter and writer tests.
#[cfg(test)]
pub(crate) mod tests_support {
    pub(crate) use super::tests::{REL, golden};
}

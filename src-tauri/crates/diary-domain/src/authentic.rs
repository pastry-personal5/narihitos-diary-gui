//! Canonical Authentic Markdown, schema 1. Pure: no clock, zone, digest or I/O.
//!
//! Layout: `---\n` + header lines + `---\n` + body (UTF-8, LF, no BOM, body verbatim).
//! The header is a closed grammar of `key: scalar` lines in a fixed order, so duplicate keys,
//! aliases, tags, anchors and compound values cannot be expressed and are rejected.

use std::fmt;

pub const MAX_BODY_BYTES: usize = 1 << 20;
const MAX_HEADER_BYTES: usize = 16 * 1024;
const MAX_EXTENSIONS: usize = 16;
const MAX_STRING: usize = 256;
const KNOWN: [&str; 10] = [
    "schema_version",
    "asset_type",
    "kind",
    "id",
    "diary_date",
    "diary_timezone",
    "timezone_database",
    "created_at",
    "updated_at",
    "revision",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DiaryDate {
    year: u16,
    month: u8,
    day: u8,
}

impl DiaryDate {
    pub fn from_ymd(year: u16, month: u8, day: u8) -> Option<Self> {
        let leap =
            year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
        let days = match month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 if leap => 29,
            2 => 28,
            _ => return None,
        };
        ((1..=9999).contains(&year) && (1..=days).contains(&day)).then_some(Self {
            year,
            month,
            day,
        })
    }

    /// Exactly `YYYY-MM-DD`.
    pub fn parse(text: &str) -> Option<Self> {
        let b = text.as_bytes();
        if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
            return None;
        }
        let num = |r: std::ops::Range<usize>| -> Option<u16> {
            let part = text.get(r)?;
            if part.bytes().all(|c| c.is_ascii_digit()) {
                part.parse().ok()
            } else {
                None
            }
        };
        Self::from_ymd(num(0..4)?, num(5..7)? as u8, num(8..10)? as u8)
    }

    pub fn year(self) -> u16 {
        self.year
    }
    pub fn month(self) -> u8 {
        self.month
    }
    pub fn day(self) -> u8 {
        self.day
    }

    /// Calendar arithmetic, independent of elapsed hours or a timezone.
    pub fn next(self) -> Option<Self> {
        Self::from_ymd(self.year, self.month, self.day + 1).or_else(|| {
            if self.month == 12 {
                Self::from_ymd(self.year + 1, 1, 1)
            } else {
                Self::from_ymd(self.year, self.month + 1, 1)
            }
        })
    }

    pub fn previous(self) -> Option<Self> {
        if self.day > 1 {
            return Self::from_ymd(self.year, self.month, self.day - 1);
        }
        let (year, month) = if self.month == 1 {
            (self.year.checked_sub(1)?, 12)
        } else {
            (self.year, self.month - 1)
        };
        (28..=31)
            .rev()
            .find_map(|day| Self::from_ymd(year, month, day))
    }
}

impl fmt::Display for DiaryDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

/// Sequence plus SHA-256 (lowercase hex) of the complete file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthenticRevision {
    pub sequence: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scalar {
    Str(String),
    Int(i64),
    Bool(bool),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthenticHeader {
    pub id: String,
    pub date: DiaryDate,
    pub timezone: String,
    pub timezone_database: String,
    pub created_at: String,
    pub updated_at: String,
    pub revision: u64,
    pub extensions: Vec<(String, Scalar)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatError {
    InvalidUtf8,
    ByteOrderMark,
    CarriageReturn,
    MissingHeader,
    HeaderTooLarge,
    /// 1-based header line that is not `key: scalar` of the closed grammar.
    BadLine(usize),
    DuplicateKey(String),
    UnknownSchema,
    WrongField(&'static str),
    BadValue(&'static str),
    DateMismatch,
    BodyTooLarge,
    TooManyExtensions,
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidUtf8 => f.write_str("file is not valid UTF-8"),
            Self::ByteOrderMark => f.write_str("file starts with a byte order mark"),
            Self::CarriageReturn => f.write_str("file contains a carriage return; LF is required"),
            Self::MissingHeader => f.write_str("file has no schema 1 header"),
            Self::HeaderTooLarge => f.write_str("header is too large"),
            Self::BadLine(n) => write!(f, "header line {n} is outside the supported grammar"),
            Self::DuplicateKey(k) => write!(f, "header key `{k}` is duplicated"),
            Self::UnknownSchema => f.write_str("unknown schema version"),
            Self::WrongField(k) => write!(f, "header field `{k}` is missing or out of order"),
            Self::BadValue(k) => write!(f, "header field `{k}` has an invalid value"),
            Self::DateMismatch => f.write_str("header date does not match the file path"),
            Self::BodyTooLarge => f.write_str("body exceeds 1 MiB"),
            Self::TooManyExtensions => f.write_str("too many extension fields"),
        }
    }
}

impl std::error::Error for FormatError {}

pub fn valid_uuid(s: &str) -> bool {
    s.len() == 36
        && s.bytes().enumerate().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => c == b'-',
            _ => c.is_ascii_digit() || (b'a'..=b'f').contains(&c),
        })
}

fn valid_instant(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 20
        && s.get(..10).and_then(DiaryDate::parse).is_some()
        && b[10] == b'T'
        && b[13] == b':'
        && b[16] == b':'
        && b[19] == b'Z'
        && [11, 12, 14, 15, 17, 18]
            .iter()
            .all(|&i| b[i].is_ascii_digit())
        && s[11..13].parse::<u8>().is_ok_and(|h| h < 24)
        && s[14..16].parse::<u8>().is_ok_and(|m| m < 60)
        && s[17..19].parse::<u8>().is_ok_and(|x| x < 60)
}

pub fn valid_label(s: &str, max: usize) -> bool {
    !s.is_empty()
        && s.len() <= max
        && s.bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_+-/.".contains(&c))
}

fn valid_ext_key(k: &str) -> bool {
    let b = k.as_bytes();
    !b.is_empty()
        && b.len() <= 64
        && b[0].is_ascii_lowercase()
        && b.iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'_')
        && !KNOWN.contains(&k)
}

fn quote(s: &str, out: &mut String) -> Result<(), FormatError> {
    if s.len() > MAX_STRING || s.chars().any(|c| c.is_control()) {
        return Err(FormatError::BadValue("extension"));
    }
    out.push('"');
    for c in s.chars() {
        if c == '"' || c == '\\' {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('"');
    Ok(())
}

fn unquote(v: &str) -> Option<String> {
    let inner = v.strip_prefix('"')?.strip_suffix('"')?;
    let mut out = String::new();
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next()? {
                e @ ('"' | '\\') => out.push(e),
                _ => return None,
            },
            '"' => return None,
            c if c.is_control() => return None,
            c => out.push(c),
        }
    }
    (out.len() <= MAX_STRING).then_some(out)
}

fn scalar(v: &str) -> Option<Scalar> {
    if v.starts_with('"') {
        return unquote(v).map(Scalar::Str);
    }
    match v {
        "true" => return Some(Scalar::Bool(true)),
        "false" => return Some(Scalar::Bool(false)),
        _ => {}
    }
    let digits = v.strip_prefix('-').unwrap_or(v);
    let ok = !digits.is_empty()
        && digits.len() <= 18
        && digits.bytes().all(|c| c.is_ascii_digit())
        && (v == "0" || !digits.starts_with('0'));
    if ok {
        v.parse().ok().map(Scalar::Int)
    } else {
        None
    }
}

/// Serialize `header` and `body` to the canonical bytes. Never mutates the body.
pub fn render(header: &AuthenticHeader, body: &str) -> Result<Vec<u8>, FormatError> {
    if body.len() > MAX_BODY_BYTES {
        return Err(FormatError::BodyTooLarge);
    }
    if body.contains('\r') {
        return Err(FormatError::CarriageReturn);
    }
    if header.revision == 0 || header.revision > i64::MAX as u64 {
        return Err(FormatError::BadValue("revision"));
    }
    if !valid_uuid(&header.id) {
        return Err(FormatError::BadValue("id"));
    }
    if !valid_label(&header.timezone, 64) {
        return Err(FormatError::BadValue("diary_timezone"));
    }
    if !valid_label(&header.timezone_database, 32) {
        return Err(FormatError::BadValue("timezone_database"));
    }
    if !valid_instant(&header.created_at) {
        return Err(FormatError::BadValue("created_at"));
    }
    if !valid_instant(&header.updated_at) {
        return Err(FormatError::BadValue("updated_at"));
    }
    if header.extensions.len() > MAX_EXTENSIONS {
        return Err(FormatError::TooManyExtensions);
    }
    let mut out = format!(
        "---\nschema_version: 1\nasset_type: diary\nkind: authentic\nid: \"{}\"\ndiary_date: \"{}\"\ndiary_timezone: \"{}\"\ntimezone_database: \"{}\"\ncreated_at: \"{}\"\nupdated_at: \"{}\"\nrevision: {}\n",
        header.id,
        header.date,
        header.timezone,
        header.timezone_database,
        header.created_at,
        header.updated_at,
        header.revision
    );
    let mut seen: Vec<&str> = Vec::new();
    for (key, value) in &header.extensions {
        if !valid_ext_key(key) || seen.contains(&key.as_str()) {
            return Err(FormatError::DuplicateKey(key.clone()));
        }
        seen.push(key);
        out.push_str(key);
        out.push_str(": ");
        match value {
            Scalar::Str(s) => quote(s, &mut out)?,
            Scalar::Int(n) => out.push_str(&n.to_string()),
            Scalar::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        }
        out.push('\n');
    }
    out.push_str("---\n");
    out.push_str(body);
    Ok(out.into_bytes())
}

/// Strictly parse a whole file. `path_date` is the date implied by the file's location.
pub fn parse(
    bytes: &[u8],
    path_date: &DiaryDate,
) -> Result<(AuthenticHeader, String), FormatError> {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return Err(FormatError::ByteOrderMark);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| FormatError::InvalidUtf8)?;
    if text.contains('\r') {
        return Err(FormatError::CarriageReturn);
    }
    let rest = text
        .strip_prefix("---\n")
        .ok_or(FormatError::MissingHeader)?;
    // `end` is the offset of the closing `---\n`; the header text is `rest[..end]`.
    let end = if rest.starts_with("---\n") {
        0
    } else {
        rest.find("\n---\n")
            .map(|i| i + 1)
            .ok_or(FormatError::MissingHeader)?
    };
    if end > MAX_HEADER_BYTES {
        return Err(FormatError::HeaderTooLarge);
    }
    let body = &rest[end + 4..];
    if body.len() > MAX_BODY_BYTES {
        return Err(FormatError::BodyTooLarge);
    }
    let mut lines = rest[..end].lines().enumerate();
    let mut next = |key: &'static str| -> Result<&str, FormatError> {
        let (n, line) = lines.next().ok_or(FormatError::WrongField(key))?;
        match line.strip_prefix(key).and_then(|r| r.strip_prefix(": ")) {
            Some(v) if !v.is_empty() && !v.ends_with(' ') => Ok(v),
            _ if line.starts_with(key) => Err(FormatError::BadLine(n + 1)),
            _ => Err(FormatError::WrongField(key)),
        }
    };
    if next("schema_version")? != "1" {
        return Err(FormatError::UnknownSchema);
    }
    if next("asset_type")? != "diary" {
        return Err(FormatError::BadValue("asset_type"));
    }
    if next("kind")? != "authentic" {
        return Err(FormatError::BadValue("kind"));
    }
    let id = unquote(next("id")?).ok_or(FormatError::BadValue("id"))?;
    let date_text = unquote(next("diary_date")?).ok_or(FormatError::BadValue("diary_date"))?;
    let timezone =
        unquote(next("diary_timezone")?).ok_or(FormatError::BadValue("diary_timezone"))?;
    let timezone_database =
        unquote(next("timezone_database")?).ok_or(FormatError::BadValue("timezone_database"))?;
    let created_at = unquote(next("created_at")?).ok_or(FormatError::BadValue("created_at"))?;
    let updated_at = unquote(next("updated_at")?).ok_or(FormatError::BadValue("updated_at"))?;
    let revision_text = next("revision")?;
    let revision: u64 = if revision_text.bytes().all(|c| c.is_ascii_digit())
        && !revision_text.starts_with('0')
        && revision_text.len() <= 18
    {
        revision_text
            .parse()
            .map_err(|_| FormatError::BadValue("revision"))?
    } else {
        return Err(FormatError::BadValue("revision"));
    };
    let date = DiaryDate::parse(&date_text).ok_or(FormatError::BadValue("diary_date"))?;
    if !valid_uuid(&id) {
        return Err(FormatError::BadValue("id"));
    }
    if !valid_label(&timezone, 64) {
        return Err(FormatError::BadValue("diary_timezone"));
    }
    if !valid_label(&timezone_database, 32) {
        return Err(FormatError::BadValue("timezone_database"));
    }
    if !valid_instant(&created_at) {
        return Err(FormatError::BadValue("created_at"));
    }
    if !valid_instant(&updated_at) {
        return Err(FormatError::BadValue("updated_at"));
    }
    if date != *path_date {
        return Err(FormatError::DateMismatch);
    }
    let mut extensions: Vec<(String, Scalar)> = Vec::new();
    for (n, line) in lines {
        let (key, value) = line.split_once(": ").ok_or(FormatError::BadLine(n + 1))?;
        if KNOWN.contains(&key) || extensions.iter().any(|(k, _)| k == key) {
            return Err(FormatError::DuplicateKey(key.to_owned()));
        }
        if !valid_ext_key(key) || value.ends_with(' ') {
            return Err(FormatError::BadLine(n + 1));
        }
        let value = scalar(value).ok_or(FormatError::BadLine(n + 1))?;
        extensions.push((key.to_owned(), value));
    }
    if extensions.len() > MAX_EXTENSIONS {
        return Err(FormatError::TooManyExtensions);
    }
    Ok((
        AuthenticHeader {
            id,
            date,
            timezone,
            timezone_database,
            created_at,
            updated_at,
            revision,
            extensions,
        },
        body.to_owned(),
    ))
}

#[cfg(test)]
mod tests;

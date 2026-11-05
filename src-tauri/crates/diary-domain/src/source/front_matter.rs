use std::fmt;
pub const MAX_FILE: usize = 8 << 20;
pub const MAX_FRONT: usize = 64 << 10;
pub const MAX_BODY: usize = 1 << 20;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Newline {
    Lf,
    CrLf,
}
#[derive(Clone, PartialEq, Eq)]
pub enum Value {
    Scalar(String),
    List(Vec<String>),
}
impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scalar(s) => f.debug_tuple("Scalar").field(&s.len()).finish(),
            Self::List(v) => f.debug_tuple("List").field(&v.len()).finish(),
        }
    }
}
#[derive(Clone, PartialEq, Eq)]
pub struct Parsed {
    pub raw_front_matter: String,
    pub fields: Vec<(String, Value)>,
    pub body: String,
    pub newline: Newline,
    pub bom: bool,
}
impl fmt::Debug for Parsed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Parsed")
            .field("front_bytes", &self.raw_front_matter.len())
            .field("body_bytes", &self.body.len())
            .field("keys", &self.fields.len())
            .finish()
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseError {
    FileTooLarge,
    Utf8,
    StrayCr,
    MixedNewlines,
    MissingFrontMatter,
    Unclosed,
    DuplicateKey,
    Alias,
    Tag,
    TooDeep,
    FrontMatterTooLarge,
    TooManyKeys,
    ScalarTooLarge,
    BodyTooLarge,
    BadLine,
}

fn scalar(raw: &str) -> Result<String, ParseError> {
    use ParseError::*;
    if raw.len() > 8192 {
        return Err(ScalarTooLarge);
    }
    if raw.starts_with(['&', '*']) {
        return Err(Alias);
    }
    if raw.starts_with('!') {
        return Err(Tag);
    }
    if raw.starts_with(['[', '{']) {
        return Err(TooDeep);
    }
    if raw.starts_with('\'') {
        if raw.len() < 2 || !raw.ends_with('\'') {
            return Err(BadLine);
        }
        let inside = &raw[1..raw.len() - 1];
        let mut out = String::new();
        let mut chars = inside.chars();
        while let Some(c) = chars.next() {
            if c == '\'' && chars.next() != Some('\'') {
                return Err(BadLine);
            }
            out.push(c);
        }
        return Ok(out);
    }
    if raw.starts_with('"') {
        if raw.len() < 2 || !raw.ends_with('"') {
            return Err(BadLine);
        }
        let mut out = String::new();
        let mut chars = raw[1..raw.len() - 1].chars();
        while let Some(c) = chars.next() {
            match c {
                '\\' => out.push(match chars.next() {
                    Some('n') => '\n',
                    Some('r') => '\r',
                    Some('t') => '\t',
                    Some('"') => '"',
                    Some('\\') => '\\',
                    _ => return Err(BadLine),
                }),
                '"' => return Err(BadLine),
                c if c.is_control() => return Err(BadLine),
                c => out.push(c),
            }
        }
        return Ok(out);
    }
    if raw.contains(": ") {
        return Err(TooDeep);
    }
    if raw.starts_with(['|', '>']) || raw.chars().any(char::is_control) {
        return Err(BadLine);
    }
    Ok(raw.into())
}
fn value(raw: &str) -> Result<Value, ParseError> {
    if raw == "[]" {
        return Ok(Value::List(Vec::new()));
    }
    if raw.starts_with('[') {
        if !raw.ends_with(']') {
            return Err(ParseError::BadLine);
        }
        let inside = &raw[1..raw.len() - 1];
        // Split only commas outside quotes; nested containers are rejected by scalar().
        let mut quote = None;
        let mut escaped = false;
        let mut start = 0;
        let mut list = Vec::new();
        for (i, c) in inside.char_indices() {
            if escaped {
                escaped = false;
                continue;
            }
            if quote == Some('"') && c == '\\' {
                escaped = true;
                continue;
            }
            if let Some(q) = quote {
                if c == q {
                    quote = None;
                }
            } else if c == '\'' || c == '"' {
                quote = Some(c);
            } else if c == ',' {
                list.push(scalar(inside[start..i].trim())?);
                start = i + 1;
            }
        }
        if quote.is_some() {
            return Err(ParseError::BadLine);
        }
        // `[ ]` is an empty list and a trailing comma adds no item, as in YAML flow sequences.
        let tail = inside[start..].trim();
        if !(tail.is_empty() && (start == 0 || !list.is_empty())) {
            list.push(scalar(tail)?);
        }
        return Ok(Value::List(list));
    }
    Ok(Value::Scalar(scalar(raw)?))
}
pub fn parse(bytes: &[u8]) -> Result<Parsed, ParseError> {
    use ParseError::*;
    if bytes.len() > MAX_FILE {
        return Err(FileTooLarge);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| Utf8)?;
    let bom = text.starts_with('\u{feff}');
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let b = text.as_bytes();
    let mut lf = false;
    let mut crlf = false;
    for (i, c) in b.iter().enumerate() {
        if *c == b'\r' && b.get(i + 1) != Some(&b'\n') {
            return Err(StrayCr);
        }
        if *c == b'\n' {
            if i > 0 && b[i - 1] == b'\r' {
                crlf = true;
            } else {
                lf = true;
            }
        }
    }
    if lf && crlf {
        return Err(MixedNewlines);
    }
    let newline = if crlf { Newline::CrLf } else { Newline::Lf };
    let mut lines = text.split_inclusive('\n');
    let clean = |s: &str| s.trim_end_matches('\n').trim_end_matches('\r').to_string();
    let first = lines.next().ok_or(MissingFrontMatter)?;
    if clean(first) != "---" {
        return Err(MissingFrontMatter);
    }
    let start = first.len();
    let mut offset = start;
    let mut end = None;
    for line in lines {
        if offset - start > MAX_FRONT {
            return Err(FrontMatterTooLarge);
        }
        if clean(line) == "---" {
            end = Some((offset, offset + line.len()));
            break;
        }
        offset += line.len();
    }
    let (close, body_start) = end.ok_or(if offset - start > MAX_FRONT {
        FrontMatterTooLarge
    } else {
        Unclosed
    })?;
    if close - start > MAX_FRONT {
        return Err(FrontMatterTooLarge);
    }
    let body = &text[body_start..];
    if body.len() > MAX_BODY {
        return Err(BodyTooLarge);
    }
    let raw = &text[start..close];
    let mut fields: Vec<(String, Value)> = Vec::new();
    for line in raw.lines() {
        if line.starts_with(char::is_whitespace) {
            // Only one flat two-space list level is accepted. Trimming arbitrary
            // indentation would silently flatten nested YAML into sibling items.
            let continuation = line.strip_prefix("  ").ok_or(TooDeep)?;
            if continuation.starts_with(char::is_whitespace) {
                return Err(TooDeep);
            }
            let (_, last) = fields.last_mut().ok_or(TooDeep)?;
            if continuation == "[]" && matches!(last, Value::Scalar(s) if s.is_empty()) {
                *last = Value::List(vec![]);
                continue;
            }
            if let Some(item) = continuation.strip_prefix("- ") {
                if matches!(last, Value::Scalar(s) if s.is_empty()) {
                    *last = Value::List(vec![]);
                }
                let Value::List(list) = last else {
                    return Err(TooDeep);
                };
                list.push(scalar(item.trim())?);
                continue;
            }
            return Err(TooDeep);
        }
        let (key, raw) = line.split_once(':').ok_or(BadLine)?;
        if key.is_empty() || !key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
            return Err(BadLine);
        }
        if fields.iter().any(|(k, _)| k == key) {
            return Err(DuplicateKey);
        }
        if fields.len() >= 64 {
            return Err(TooManyKeys);
        }
        fields.push((key.into(), value(raw.trim())?));
    }
    Ok(Parsed {
        raw_front_matter: raw.into(),
        fields,
        body: body.into(),
        newline,
        bom,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn doc(header: &str, body: &str) -> Vec<u8> {
        format!("---\n{header}\n---\n{body}").into_bytes()
    }
    #[test]
    fn preservation_and_producer_quoting() {
        let body = "\nhello\n![audio](x.assets/a.m4a)\n";
        let bytes = doc(
            "id: \"x\"\ntags:\n  []\nunknown: 'it''s mine'\nlist: [a, \"b,c\"]\nescaped: \"a\\n\\t\\r\\\"\\\\\"",
            body,
        );
        let p = parse(&bytes).unwrap();
        assert_eq!(p.body, body);
        assert_eq!(p.fields[1].1, Value::List(vec![]));
        assert_eq!(p.fields[2].1, Value::Scalar("it's mine".into()));
        assert_eq!(p.fields[3].1, Value::List(vec!["a".into(), "b,c".into()]));
        let crlf = format!(
            "\u{feff}{}",
            String::from_utf8(bytes).unwrap().replace('\n', "\r\n")
        );
        let p = parse(crlf.as_bytes()).unwrap();
        assert!(p.bom);
        assert_eq!(p.newline, Newline::CrLf);
        assert_eq!(p.body, body.replace('\n', "\r\n"));
        for h in ["tags: []", "tags:\n  []", "tags: [ ]"] {
            assert_eq!(parse(&doc(h, "")).unwrap().fields[0].1, Value::List(vec![]));
        }
        assert_eq!(
            parse(&doc("tags: [a, ]", "")).unwrap().fields[0].1,
            Value::List(vec!["a".into()])
        );
        assert_eq!(
            parse(&doc("tags:\n  - one\n  - two", "")).unwrap().fields[0].1,
            Value::List(vec!["one".into(), "two".into()])
        );
    }
    #[test]
    fn every_error_is_bounded_and_distinct() {
        use ParseError::*;
        let cases = vec![
            (vec![b'x'; MAX_FILE + 1], FileTooLarge),
            (vec![255], Utf8),
            (b"---\na: 1\n---\nx\r".to_vec(), StrayCr),
            (b"---\r\na: 1\n---\n".to_vec(), MixedNewlines),
            (b"hello".to_vec(), MissingFrontMatter),
            (b"---\na: 1".to_vec(), Unclosed),
            (doc("a: 1\na: 2", ""), DuplicateKey),
            (doc("a: &anchor", ""), Alias),
            (doc("a: *anchor", ""), Alias),
            (doc("a: !tag x", ""), Tag),
            (doc("a:\n  nested: 1", ""), TooDeep),
            (doc("a: [[1]]", ""), TooDeep),
            (doc("a:\n  - one\n    - nested", ""), TooDeep),
            (doc(&"x".repeat(MAX_FRONT + 1), ""), FrontMatterTooLarge),
            (
                doc(
                    &(0..65)
                        .map(|i| format!("k{i}: 1"))
                        .collect::<Vec<_>>()
                        .join("\n"),
                    "",
                ),
                TooManyKeys,
            ),
            (doc(&format!("a: {}", "x".repeat(8193)), ""), ScalarTooLarge),
            (doc("a: 1", &"x".repeat(MAX_BODY + 1)), BodyTooLarge),
            (doc("not a key", ""), BadLine),
            (doc("a: \"bad\\q\"", ""), BadLine),
        ];
        for (bytes, e) in cases {
            assert_eq!(parse(&bytes), Err(e), "{e:?}");
        }
    }
    #[test]
    fn mutated_input_never_panics_or_bypasses_limits() {
        let bytes = doc("id: \"synthetic\"\ntags: [one, two]", "body 😀\n");
        for end in 0..=bytes.len() {
            let _ = parse(&bytes[..end]);
        }
        for i in 0..bytes.len() {
            for byte in 0..=255 {
                let mut b = bytes.clone();
                b[i] = byte;
                if let Ok(p) = parse(&b) {
                    assert!(p.body.len() <= MAX_BODY);
                    assert!(p.fields.len() <= 64);
                    assert!(p.raw_front_matter.len() <= MAX_FRONT);
                }
            }
        }
    }
}

//! Lossless paragraph/structure review: tokenizer, deterministic alignment and recomposition.
//!
//! Pure and I/O free. Digests come from an injected [`Hasher`] (SHA-256 lives in infrastructure).

use pulldown_cmark::{Event, Options, Parser, Tag};
use std::{collections::BTreeSet, ops::Range};

pub const RULES_VERSION: u32 = 1;
pub const MAX_UNITS: usize = 4_000;
pub const MAX_COMPARISONS: usize = 4_000_000;

pub trait Hasher: Send + Sync {
    fn sha256_hex(&self, bytes: &[u8]) -> String;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewError {
    /// Exact source spans cannot be preserved; never fall back to whole-file replacement.
    Unsupported,
    Limit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitKind {
    Prefix,
    Paragraph,
    Heading,
    ListItem,
    Blockquote,
    Fence,
    Table,
    Other,
}

impl UnitKind {
    /// Label shown for indivisible structures; plain paragraphs need none.
    pub fn label(self) -> Option<&'static str> {
        Some(match self {
            Self::Heading => "heading",
            Self::ListItem => "list item",
            Self::Blockquote => "blockquote",
            Self::Fence => "code block",
            Self::Table => "table",
            Self::Other => "block",
            Self::Prefix | Self::Paragraph => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unit {
    pub kind: UnitKind,
    pub range: Range<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HunkKind {
    Replace,
    Insert,
    Delete,
}

impl HunkKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Replace => "replace",
            Self::Insert => "insert",
            Self::Delete => "delete",
        }
    }
}

/// Byte ranges into the baseline and candidate texts. Insert has an empty baseline range at
/// the boundary; Delete has an empty candidate range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hunk {
    pub id: String,
    pub kind: HunkKind,
    pub unit: UnitKind,
    pub baseline: Range<usize>,
    pub candidate: Range<usize>,
}

/// A hunk not yet decided, located in the current composed body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pending {
    pub id: String,
    pub kind: HunkKind,
    pub unit: UnitKind,
    /// Baseline text the hunk would remove or replace (empty for Insert).
    pub body: Range<usize>,
    /// Candidate text the hunk would add (empty for Delete).
    pub text: String,
}

fn kind_of(tag: &Tag) -> UnitKind {
    match tag {
        Tag::Paragraph => UnitKind::Paragraph,
        Tag::Heading { .. } => UnitKind::Heading,
        Tag::BlockQuote(_) => UnitKind::Blockquote,
        Tag::CodeBlock(_) => UnitKind::Fence,
        Tag::Table(_) => UnitKind::Table,
        _ => UnitKind::Other,
    }
}

/// Link reference definitions emit no event; their lines are the only non-blank text allowed
/// between blocks.
fn skippable(gap: &str) -> bool {
    gap.lines().all(|l| {
        let t = l.trim_start();
        t.is_empty() || t.starts_with('[')
    })
}

/// Split into byte-exact units: a whitespace/definition prefix unit, then one unit per
/// top-level block (list items individually), each owning the separator after it.
pub fn units(text: &str) -> Result<Vec<Unit>, ReviewError> {
    let mut blocks: Vec<(UnitKind, Range<usize>)> = Vec::new();
    let mut depth = 0usize;
    let mut top_list = false;
    for (event, range) in Parser::new_ext(text, Options::ENABLE_TABLES).into_offset_iter() {
        match event {
            Event::Start(tag) => {
                if depth == 0 && matches!(tag, Tag::List(_)) {
                    top_list = true;
                } else if (depth == 0 && !top_list)
                    || (depth == 1 && top_list && matches!(tag, Tag::Item))
                {
                    let kind = if top_list {
                        UnitKind::ListItem
                    } else {
                        kind_of(&tag)
                    };
                    blocks.push((kind, range));
                }
                depth += 1;
            }
            Event::End(_) => {
                depth = depth.checked_sub(1).ok_or(ReviewError::Unsupported)?;
                if depth == 0 {
                    top_list = false;
                }
            }
            Event::Rule if depth == 0 => blocks.push((UnitKind::Other, range)),
            _ => {}
        }
    }
    let mut out: Vec<Unit> = Vec::with_capacity(blocks.len() + 1);
    let mut pos = 0usize;
    for (kind, r) in blocks {
        if r.start < pos || r.end > text.len() || r.start >= r.end {
            return Err(ReviewError::Unsupported);
        }
        let gap = text.get(pos..r.start).ok_or(ReviewError::Unsupported)?;
        if !skippable(gap) {
            return Err(ReviewError::Unsupported);
        }
        match out.last_mut() {
            Some(last) => last.range.end = r.start,
            None if r.start > 0 => out.push(Unit {
                kind: UnitKind::Prefix,
                range: 0..r.start,
            }),
            None => {}
        }
        out.push(Unit {
            kind,
            range: r.start..r.end,
        });
        pos = r.end;
    }
    let tail = text.get(pos..).ok_or(ReviewError::Unsupported)?;
    if !skippable(tail) {
        return Err(ReviewError::Unsupported);
    }
    match out.last_mut() {
        Some(last) => last.range.end = text.len(),
        None if !text.is_empty() => out.push(Unit {
            kind: UnitKind::Prefix,
            range: 0..text.len(),
        }),
        None => {}
    }
    verify(text, &out)?;
    Ok(out)
}

/// Units must tile the text with no hole or overlap and sit on char boundaries.
pub fn verify(text: &str, units: &[Unit]) -> Result<(), ReviewError> {
    let mut pos = 0;
    for u in units {
        if u.range.start != pos
            || u.range.end <= u.range.start
            || !text.is_char_boundary(u.range.end)
        {
            return Err(ReviewError::Unsupported);
        }
        pos = u.range.end;
    }
    if pos == text.len() {
        Ok(())
    } else {
        Err(ReviewError::Unsupported)
    }
}

fn field(buf: &mut Vec<u8>, bytes: &[u8]) {
    buf.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    buf.extend_from_slice(bytes);
}

/// SHA-256 over the length-prefixed IDs in ascending byte order; the empty set hashes zero IDs.
pub fn set_digest(h: &dyn Hasher, ids: &BTreeSet<String>) -> String {
    let mut buf = Vec::new();
    for id in ids {
        field(&mut buf, id.as_bytes());
    }
    h.sha256_hex(&buf)
}

pub fn text_digest(h: &dyn Hasher, text: &str) -> String {
    h.sha256_hex(text.as_bytes())
}

/// Longest common subsequence of byte-exact unit text; ties take the earliest baseline then
/// earliest candidate unit. Unchanged units anchor; each differing region pairs units by order.
pub fn align(h: &dyn Hasher, b_text: &str, c_text: &str) -> Result<Vec<Hunk>, ReviewError> {
    let bu = units(b_text)?;
    let cu = units(c_text)?;
    if bu.len() + cu.len() > MAX_UNITS || bu.len().saturating_mul(cu.len()) > MAX_COMPARISONS {
        return Err(ReviewError::Limit);
    }
    let (m, n) = (bu.len(), cu.len());
    let same = |i: usize, j: usize| b_text[bu[i].range.clone()] == c_text[cu[j].range.clone()];
    let w = n + 1;
    let mut t = vec![0u16; (m + 1) * w];
    for i in (0..m).rev() {
        for j in (0..n).rev() {
            t[i * w + j] = if same(i, j) {
                t[(i + 1) * w + j + 1] + 1
            } else {
                t[(i + 1) * w + j].max(t[i * w + j + 1])
            };
        }
    }
    // Anchors in order.
    let mut anchors = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < m && j < n {
        if same(i, j) && t[i * w + j] == t[(i + 1) * w + j + 1] + 1 {
            anchors.push((i, j));
            i += 1;
            j += 1;
        } else if t[(i + 1) * w + j] == t[i * w + j] {
            i += 1;
        } else {
            j += 1;
        }
    }
    anchors.push((m, n));
    let (bd, cd) = (text_digest(h, b_text), text_digest(h, c_text));
    let make = |kind: HunkKind, unit: UnitKind, baseline: Range<usize>, candidate: Range<usize>| {
        let mut buf = Vec::new();
        field(&mut buf, &RULES_VERSION.to_be_bytes());
        field(&mut buf, bd.as_bytes());
        field(&mut buf, cd.as_bytes());
        field(&mut buf, kind.as_str().as_bytes());
        field(&mut buf, &(baseline.start as u64).to_be_bytes());
        field(&mut buf, &(candidate.start as u64).to_be_bytes());
        Hunk {
            id: h.sha256_hex(&buf),
            kind,
            unit,
            baseline,
            candidate,
        }
    };
    let b_at = |i: usize| bu.get(i).map_or(b_text.len(), |u| u.range.start);
    let mut hunks = Vec::new();
    let (mut bi, mut ci) = (0, 0);
    for (ai, aj) in anchors {
        let (nb, nc) = (ai - bi, aj - ci);
        for k in 0..nb.min(nc) {
            let (b, c) = (&bu[bi + k], &cu[ci + k]);
            hunks.push(make(
                HunkKind::Replace,
                c.kind,
                b.range.clone(),
                c.range.clone(),
            ));
        }
        for b in bu.iter().take(ai).skip(bi + nc.min(nb)) {
            hunks.push(make(HunkKind::Delete, b.kind, b.range.clone(), 0..0));
        }
        for c in cu.iter().take(aj).skip(ci + nb.min(nc)) {
            let at = b_at(ai);
            hunks.push(make(HunkKind::Insert, c.kind, at..at, c.range.clone()));
        }
        bi = ai + 1;
        ci = aj + 1;
    }
    Ok(hunks)
}

/// Recompose from the immutable baseline plus decided hunks, in original order. Returns the
/// body and the still-pending hunks located in it.
pub fn view(
    b_text: &str,
    c_text: &str,
    hunks: &[Hunk],
    accepted: &BTreeSet<String>,
    rejected: &BTreeSet<String>,
) -> (String, Vec<Pending>) {
    let mut out = String::with_capacity(b_text.len());
    let mut pending = Vec::new();
    let mut cursor = 0;
    for hk in hunks {
        out.push_str(&b_text[cursor..hk.baseline.start]);
        cursor = hk.baseline.start;
        if accepted.contains(&hk.id) {
            out.push_str(&c_text[hk.candidate.clone()]);
            cursor = hk.baseline.end;
        } else if !rejected.contains(&hk.id) {
            let at = out.len();
            pending.push(Pending {
                id: hk.id.clone(),
                kind: hk.kind,
                unit: hk.unit,
                body: at..at + hk.baseline.len(),
                text: c_text[hk.candidate.clone()].to_owned(),
            });
        }
    }
    out.push_str(&b_text[cursor..]);
    (out, pending)
}

pub fn compose(b_text: &str, c_text: &str, hunks: &[Hunk], accepted: &BTreeSet<String>) -> String {
    view(b_text, c_text, hunks, accepted, &BTreeSet::new()).0
}

#[cfg(test)]
mod tests;

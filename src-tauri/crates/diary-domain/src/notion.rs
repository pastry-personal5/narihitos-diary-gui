//! Deterministic Markdown to Notion block projection. Pure and I/O free.
//!
//! Inline Markdown is kept as literal source text (no annotations), so nothing is silently
//! dropped. Constructs Notion cannot represent become literal text and are counted in
//! [`Projection::warnings`]. JSON encoding and hashing live in infrastructure.

use crate::authentic::DiaryDate;
use crate::calendar::weekday;
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use std::ops::Range;

/// The day heading text, for example `2026-11-04 (WED)`. English weekdays keep it stable.
pub fn day_title(date: DiaryDate) -> String {
    const DAYS: [&str; 7] = ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"];
    format!("{date} ({})", DAYS[usize::from(weekday(date))])
}

/// Notion allows 2,000 characters per rich-text item; UTF-16 units are the conservative count.
pub const MAX_RICH_UNITS: usize = 2_000;
/// A block carries at most 100 rich-text items, so longer text continues in a new block.
const MAX_BLOCK_UNITS: usize = 100 * MAX_RICH_UNITS - 400;
pub const MAX_CHILDREN: usize = 100;
/// Below the documented 1000-block payload cap, counting nested blocks.
pub const MAX_BLOCKS_PER_REQUEST: usize = 900;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    Paragraph,
    /// Levels 1 to 3.
    Heading(u8),
    Bullet,
    Numbered,
    Quote,
    Code(String),
    Divider,
}
impl Kind {
    pub fn tag(&self) -> &'static str {
        match self {
            Self::Paragraph => "paragraph",
            Self::Heading(1) => "heading_1",
            Self::Heading(2) => "heading_2",
            Self::Heading(_) => "heading_3",
            Self::Bullet => "bulleted_list_item",
            Self::Numbered => "numbered_list_item",
            Self::Quote => "quote",
            Self::Code(_) => "code",
            Self::Divider => "divider",
        }
    }
    pub fn from_tag(tag: &str) -> Option<Self> {
        Some(match tag {
            "paragraph" => Self::Paragraph,
            "heading_1" => Self::Heading(1),
            "heading_2" => Self::Heading(2),
            "heading_3" => Self::Heading(3),
            "bulleted_list_item" => Self::Bullet,
            "numbered_list_item" => Self::Numbered,
            "quote" => Self::Quote,
            "code" => Self::Code(String::new()),
            "divider" => Self::Divider,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub kind: Kind,
    pub text: String,
    pub children: Vec<Block>,
}
impl Block {
    fn leaf(kind: Kind, text: impl Into<String>) -> Self {
        Self {
            kind,
            text: text.into(),
            children: Vec::new(),
        }
    }
    /// This block plus every descendant.
    pub fn count(&self) -> usize {
        1 + self.children.iter().map(Block::count).sum::<usize>()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Projection {
    pub blocks: Vec<Block>,
    /// Constructs kept only as literal text, or restructured to fit the service limits.
    pub warnings: u32,
}

/// Splits on `char` boundaries into pieces of at most [`MAX_RICH_UNITS`] UTF-16 units.
pub fn split_rich(text: &str) -> Vec<&str> {
    split_units(text, MAX_RICH_UNITS)
}
fn split_units(text: &str, limit: usize) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut start, mut units) = (0, 0);
    for (i, c) in text.char_indices() {
        let n = c.len_utf16();
        if units + n > limit {
            out.push(&text[start..i]);
            start = i;
            units = 0;
        }
        units += n;
    }
    if start < text.len() || out.is_empty() {
        out.push(&text[start..]);
    }
    out
}

fn is_block(tag: &Tag) -> bool {
    matches!(
        tag,
        Tag::Paragraph
            | Tag::Heading { .. }
            | Tag::BlockQuote(_)
            | Tag::CodeBlock(_)
            | Tag::HtmlBlock
            | Tag::List(_)
            | Tag::Item
            | Tag::FootnoteDefinition(_)
            | Tag::Table(_)
            | Tag::TableHead
            | Tag::TableRow
            | Tag::TableCell
            | Tag::MetadataBlock(_)
            | Tag::DefinitionList
            | Tag::DefinitionListTitle
            | Tag::DefinitionListDefinition
    )
}

struct Ctx<'a> {
    src: &'a str,
    events: Vec<(Event<'a>, Range<usize>)>,
    warnings: u32,
}

/// Source text of an inline run, with continuation indentation removed.
fn literal(src: &str, range: Range<usize>) -> String {
    src[range]
        .lines()
        .map(str::trim)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

impl<'a> Ctx<'a> {
    /// Index just past the `End` matching the `Start` at `i`.
    fn skip(&self, i: usize) -> usize {
        let mut depth = 0usize;
        let mut j = i;
        while j < self.events.len() {
            match &self.events[j].0 {
                Event::Start(_) => depth += 1,
                Event::End(_) => {
                    depth -= 1;
                    if depth == 0 {
                        return j + 1;
                    }
                }
                _ => {}
            }
            j += 1;
        }
        j
    }
    /// The inline source range covered by the events inside the container starting at `i`.
    fn inline_range(&self, i: usize) -> Option<Range<usize>> {
        let end = self.skip(i);
        let mut range: Option<Range<usize>> = None;
        for (event, r) in &self.events[i + 1..end - 1] {
            if matches!(event, Event::Start(t) if is_block(t)) {
                continue;
            }
            range = Some(match range {
                None => r.clone(),
                Some(a) => a.start.min(r.start)..a.end.max(r.end),
            });
        }
        range
    }
    fn unsupported(&mut self, i: usize) -> Block {
        self.warnings += 1;
        Block::leaf(Kind::Paragraph, literal(self.src, self.events[i].1.clone()))
    }
    fn blocks(&mut self, mut i: usize, stop_at_end: bool) -> (Vec<Block>, usize) {
        let mut out = Vec::new();
        while i < self.events.len() {
            let event = self.events[i].0.clone();
            match event {
                Event::End(_) if stop_at_end => return (out, i + 1),
                Event::Start(Tag::Paragraph) => {
                    let text = self
                        .inline_range(i)
                        .map(|r| literal(self.src, r))
                        .unwrap_or_default();
                    out.push(Block::leaf(Kind::Paragraph, text));
                    i = self.skip(i);
                }
                Event::Start(Tag::Heading { level, .. }) => {
                    let n = level as u8;
                    if n > 3 {
                        self.warnings += 1;
                    }
                    let text = self
                        .inline_range(i)
                        .map(|r| literal(self.src, r))
                        .unwrap_or_default();
                    // Heading 2 marks a day in the export, so every body heading becomes heading 3.
                    out.push(Block::leaf(Kind::Heading(3), text));
                    i = self.skip(i);
                }
                Event::Start(Tag::CodeBlock(kind)) => {
                    let lang = match kind {
                        pulldown_cmark::CodeBlockKind::Fenced(info) => {
                            info.split_whitespace().next().unwrap_or("").to_string()
                        }
                        _ => String::new(),
                    };
                    let end = self.skip(i);
                    let mut text = String::new();
                    for (e, _) in &self.events[i + 1..end - 1] {
                        if let Event::Text(t) = e {
                            text.push_str(t);
                        }
                    }
                    out.push(Block::leaf(
                        Kind::Code(lang),
                        text.trim_end_matches('\n').to_string(),
                    ));
                    i = end;
                }
                Event::Start(Tag::BlockQuote(_)) => {
                    let end = self.skip(i);
                    let nested = self.events[i + 1..end - 1].iter().any(|(e, _)| {
                        matches!(
                            e,
                            Event::Start(
                                Tag::List(_)
                                    | Tag::CodeBlock(_)
                                    | Tag::Table(_)
                                    | Tag::BlockQuote(_)
                                    | Tag::HtmlBlock
                            )
                        )
                    });
                    if nested {
                        self.warnings += 1;
                    }
                    let raw = &self.src[self.events[i].1.clone()];
                    let text = raw
                        .lines()
                        .map(|l| l.trim_start().trim_start_matches('>').trim())
                        .collect::<Vec<_>>()
                        .join("\n")
                        .trim()
                        .to_string();
                    out.push(Block::leaf(Kind::Quote, text));
                    i = end;
                }
                Event::Start(Tag::List(first)) => {
                    let (items, next) = self.list(i + 1, first.is_some());
                    out.extend(items);
                    i = next;
                }
                Event::Rule => {
                    out.push(Block::leaf(Kind::Divider, ""));
                    i += 1;
                }
                Event::Start(_) => {
                    out.push(self.unsupported(i));
                    i = self.skip(i);
                }
                Event::End(_) => i += 1,
                // Stray inline events at block level (for example raw HTML): keep literally.
                other => {
                    if !matches!(other, Event::SoftBreak | Event::HardBreak) {
                        self.warnings += 1;
                        out.push(Block::leaf(
                            Kind::Paragraph,
                            literal(self.src, self.events[i].1.clone()),
                        ));
                    }
                    i += 1;
                }
            }
        }
        (out, i)
    }
    /// Items of one list; `i` is just past the list's `Start`.
    fn list(&mut self, mut i: usize, numbered: bool) -> (Vec<Block>, usize) {
        let kind = if numbered {
            Kind::Numbered
        } else {
            Kind::Bullet
        };
        let mut items = Vec::new();
        while i < self.events.len() {
            match self.events[i].0.clone() {
                Event::End(TagEnd::List(_)) => return (items, i + 1),
                Event::Start(Tag::Item) => {
                    let end = self.skip(i);
                    let mut range: Option<Range<usize>> = None;
                    let mut children = Vec::new();
                    let mut j = i + 1;
                    while j < end - 1 {
                        let (event, r) = self.events[j].clone();
                        match event {
                            Event::Start(Tag::List(first)) => {
                                let (nested, next) = self.list(j + 1, first.is_some());
                                children.extend(nested);
                                j = next;
                            }
                            Event::Start(Tag::Paragraph) => {
                                if let Some(p) = self.inline_range(j) {
                                    range = Some(match range {
                                        None => p,
                                        Some(a) => a.start.min(p.start)..a.end.max(p.end),
                                    });
                                }
                                j = self.skip(j);
                            }
                            Event::Start(ref t) if is_block(t) => {
                                children.push(self.unsupported(j));
                                j = self.skip(j);
                            }
                            Event::End(_) => j += 1,
                            _ => {
                                range = Some(match range {
                                    None => r,
                                    Some(a) => a.start.min(r.start)..a.end.max(r.end),
                                });
                                j += 1;
                            }
                        }
                    }
                    let text = range.map(|r| literal(self.src, r)).unwrap_or_default();
                    items.push(Block {
                        kind: kind.clone(),
                        text,
                        children,
                    });
                    i = end;
                }
                _ => i += 1,
            }
        }
        (items, i)
    }
}

/// Notion accepts nested children only within a request's nesting limit; keep one level and
/// flatten deeper items after their parent as literal indented text.
fn limit_depth(blocks: Vec<Block>, level: usize, warnings: &mut u32) -> Vec<Block> {
    let mut out = Vec::new();
    for mut block in blocks {
        let children = std::mem::take(&mut block.children);
        if level == 0 {
            block.children = limit_depth(children, 1, warnings);
            out.push(block);
        } else {
            out.push(block);
            if !children.is_empty() {
                *warnings += 1;
            }
            fn flat(children: Vec<Block>, depth: usize, out: &mut Vec<Block>) {
                for mut c in children {
                    let nested = std::mem::take(&mut c.children);
                    c.text = format!("{}{}", "  ".repeat(depth), c.text);
                    out.push(c);
                    flat(nested, depth + 1, out);
                }
            }
            flat(children, 1, &mut out);
        }
    }
    out
}

/// Continues oversized text in following blocks of the same kind.
fn split_oversize(blocks: Vec<Block>, warnings: &mut u32) -> Vec<Block> {
    let mut out = Vec::new();
    for mut block in blocks {
        block.children = split_oversize(std::mem::take(&mut block.children), warnings);
        if block.text.encode_utf16().count() <= MAX_BLOCK_UNITS {
            out.push(block);
            continue;
        }
        *warnings += 1;
        let text = std::mem::take(&mut block.text);
        let pieces = split_units(&text, MAX_BLOCK_UNITS);
        let last = pieces.len() - 1;
        for (n, piece) in pieces.into_iter().enumerate() {
            out.push(Block {
                kind: block.kind.clone(),
                text: piece.to_string(),
                children: if n == last {
                    std::mem::take(&mut block.children)
                } else {
                    Vec::new()
                },
            });
        }
    }
    out
}

/// Projects an Authentic body (Markdown) into Notion blocks.
pub fn project(body: &str) -> Projection {
    let events: Vec<_> = Parser::new_ext(body, Options::ENABLE_TABLES)
        .into_offset_iter()
        .collect();
    let mut ctx = Ctx {
        src: body,
        events,
        warnings: 0,
    };
    let (blocks, _) = ctx.blocks(0, false);
    let mut warnings = ctx.warnings;
    let blocks = limit_depth(blocks, 0, &mut warnings);
    let blocks = split_oversize(blocks, &mut warnings);
    Projection { blocks, warnings }
}

/// Deterministic text of a block tree; equal canonical forms mean equal remote content.
/// Code languages are excluded because Notion constrains them to its own list.
pub fn canonical(blocks: &[Block]) -> String {
    fn walk(blocks: &[Block], depth: usize, out: &mut String) {
        for b in blocks {
            out.push_str(&format!(
                "{depth}|{}|{}|{}\n",
                b.kind.tag(),
                b.text.encode_utf16().count(),
                b.text
            ));
            walk(&b.children, depth + 1, out);
        }
    }
    let mut out = String::new();
    walk(blocks, 0, &mut out);
    out
}

/// Greedy chunks of whole top-level blocks: at most [`MAX_CHILDREN`] top-level blocks,
/// [`MAX_BLOCKS_PER_REQUEST`] blocks in total and `max_bytes` by the caller's size function.
/// A single block over `max_bytes` still gets its own chunk; the service then rejects it.
pub fn chunk(
    blocks: &[Block],
    size: impl Fn(&Block) -> usize,
    max_bytes: usize,
) -> Vec<Vec<Block>> {
    let mut chunks: Vec<Vec<Block>> = Vec::new();
    let (mut bytes, mut total) = (0usize, 0usize);
    for block in blocks {
        let (s, n) = (size(block), block.count());
        let fits = chunks.last().is_some_and(|c| {
            c.len() < MAX_CHILDREN && total + n <= MAX_BLOCKS_PER_REQUEST && bytes + s <= max_bytes
        });
        if fits {
            chunks.last_mut().expect("checked").push(block.clone());
            bytes += s;
            total += n;
        } else {
            chunks.push(vec![block.clone()]);
            bytes = s;
            total = n;
        }
    }
    chunks
}

#[cfg(test)]
mod tests;

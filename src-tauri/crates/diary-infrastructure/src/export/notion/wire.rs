//! JSON shapes for the Notion API: request bodies from projected blocks and remote responses
//! back into comparable blocks. Remote text is never logged.
use super::client::Error;
use crate::export::hex_sha;
use diary_domain::{
    authentic::DiaryDate,
    notion::{Block, Kind, canonical, split_rich},
};
use serde_json::{Map, Value, json};
use std::collections::HashSet;

const LANGUAGES: &[&str] = &[
    "bash",
    "c",
    "c++",
    "css",
    "go",
    "html",
    "java",
    "javascript",
    "json",
    "markdown",
    "python",
    "rust",
    "shell",
    "sql",
    "swift",
    "typescript",
    "yaml",
];
const MAX_PAGES: usize = 50;
const MAX_DEPTH: usize = 4;

pub fn rich(text: &str) -> Value {
    Value::Array(
        split_rich(text)
            .into_iter()
            .filter(|p| !p.is_empty())
            .map(|p| json!({"type":"text","text":{"content":p}}))
            .collect(),
    )
}
fn typed(tag: &str, inner: Value) -> Value {
    let mut m = Map::new();
    m.insert("object".into(), json!("block"));
    m.insert("type".into(), json!(tag));
    m.insert(tag.into(), inner);
    Value::Object(m)
}
pub fn block(b: &Block) -> Value {
    let mut inner = match b.kind {
        Kind::Divider => json!({}),
        _ => json!({"rich_text": rich(&b.text)}),
    };
    if let Kind::Code(lang) = &b.kind {
        let lang = lang.to_ascii_lowercase();
        let lang = if LANGUAGES.contains(&lang.as_str()) {
            lang
        } else {
            "plain text".into()
        };
        inner["language"] = json!(lang);
    }
    if !b.children.is_empty() {
        inner["children"] = Value::Array(b.children.iter().map(block).collect());
    }
    typed(b.kind.tag(), inner)
}
pub fn encoded_size(b: &Block) -> usize {
    serde_json::to_vec(&block(b)).map_or(usize::MAX / 2, |v| v.len() + 1)
}
/// A day heading: heading 2 marks the start of one diary day in a month page.
pub fn heading(title: &str) -> Value {
    typed("heading_2", json!({"rich_text": rich(title)}))
}
pub fn page(parent: &str, title: &str) -> Value {
    json!({"parent":{"page_id":parent},"properties":{"title":{"title":[{"text":{"content":title}}]}}})
}
/// `None` appends at the end, the API default.
pub fn append(children: Vec<Value>, after: Option<&str>, start: bool) -> Value {
    let mut body = json!({"children": children});
    if start {
        body["position"] = json!({"type":"start"});
    } else if let Some(id) = after {
        body["position"] = json!({"type":"after_block","after_block":{"id":id}});
    }
    body
}

/// One listed child, reduced to what projection and ordering need.
#[derive(Debug, Clone)]
pub struct Remote {
    pub id: String,
    pub kind: String,
    pub text: String,
    pub title: Option<String>,
    pub has_children: bool,
    /// No rich-text item carries an annotation, a non-default color or a link: copying the block
    /// as plain text loses nothing.
    pub plain: bool,
    /// A code block's language as Notion reports it.
    pub lang: Option<String>,
}
pub(crate) fn is_plain(rich: &Value) -> bool {
    rich.as_array().is_none_or(|items| {
        items.iter().all(|i| {
            let a = &i["annotations"];
            let flags = ["bold", "italic", "strikethrough", "underline", "code"];
            flags.iter().all(|f| !a[*f].as_bool().unwrap_or(false))
                && a["color"].as_str().is_none_or(|c| c == "default")
                && i["href"].is_null()
                // Mentions and equations would be flattened to their text by a copy.
                && i["type"].as_str().is_none_or(|t| t == "text")
        })
    })
}
/// Block-level settings a plain copy would drop: color, a toggleable heading and a code caption.
pub(crate) fn block_is_plain(k: &Value) -> bool {
    is_plain(&k["rich_text"])
        && k["color"].as_str().is_none_or(|c| c == "default")
        && !k["is_toggleable"].as_bool().unwrap_or(false)
        && k["caption"].as_array().is_none_or(Vec::is_empty)
}
pub(crate) fn plain(v: &Value) -> String {
    v.as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|i| {
                    i.get("plain_text")
                        .or_else(|| i.get("text").and_then(|t| t.get("content")))
                        .and_then(Value::as_str)
                })
                .collect()
        })
        .unwrap_or_default()
}
pub fn parse_page(v: &Value) -> Result<(Vec<Remote>, Option<String>), Error> {
    let results = v
        .get("results")
        .and_then(Value::as_array)
        .ok_or(Error::Malformed)?;
    let mut out = Vec::new();
    for r in results {
        let kind = r
            .get("type")
            .and_then(Value::as_str)
            .ok_or(Error::Malformed)?;
        let id = r
            .get("id")
            .and_then(Value::as_str)
            .ok_or(Error::Malformed)?;
        let lang = (kind == "code")
            .then(|| r[kind]["language"].as_str().map(str::to_string))
            .flatten();
        out.push(Remote {
            id: id.into(),
            kind: kind.into(),
            text: r
                .get(kind)
                .map(|k| plain(&k["rich_text"]))
                .unwrap_or_default(),
            title: (kind == "child_page")
                .then(|| r[kind]["title"].as_str().map(str::to_string))
                .flatten(),
            has_children: r
                .get("has_children")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            plain: r.get(kind).is_none_or(block_is_plain)
                && lang.as_deref().is_none_or(|l| {
                    l == "plain text" || LANGUAGES.contains(&l.to_ascii_lowercase().as_str())
                }),
            lang,
        });
    }
    let more = v.get("has_more").and_then(Value::as_bool).unwrap_or(false);
    let cursor = if more {
        Some(
            v.get("next_cursor")
                .and_then(Value::as_str)
                .ok_or(Error::Malformed)?
                .to_string(),
        )
    } else {
        None
    };
    Ok((out, cursor))
}
/// The ID of the day heading titled `title` in an append reply that is not already `known`. The
/// reply may list neighbours, including an older heading with the same title.
pub fn new_heading(v: &Value, title: &str, known: &HashSet<String>) -> Option<String> {
    let (items, _) = parse_page(v).ok()?;
    items
        .into_iter()
        .find(|r| r.kind == "heading_2" && r.text.trim() == title && !known.contains(&r.id))
        .map(|r| r.id)
}
/// Reply blocks not seen before, in reply order. Real replies may list neighbours too.
pub fn new_ids(v: &Value, known: &HashSet<String>) -> Vec<String> {
    parse_page(v).map_or_else(
        |_| Vec::new(),
        |(items, _)| {
            items
                .into_iter()
                .map(|r| r.id)
                .filter(|id| !known.contains(id))
                .collect()
        },
    )
}
/// The date of a day heading: a `heading_2` whose text starts with an ISO date and then ends or
/// continues after whitespace (the weekday suffix is written but not required).
pub fn day_of(r: &Remote) -> Option<DiaryDate> {
    if r.kind != "heading_2" {
        return None;
    }
    let t = r.text.trim();
    let date = DiaryDate::parse(t.get(..10)?)?;
    let rest = &t[10..];
    (rest.is_empty() || rest.starts_with(char::is_whitespace)).then_some(date)
}
/// Indexes of the days to move so the page ends ascending: everything outside the longest
/// strictly ascending run, which keeps the earliest-positioned days on ties. `dates` are unique
/// ISO dates in page order.
pub fn plan_moves(dates: &[String]) -> Vec<usize> {
    let n = dates.len();
    let (mut len, mut prev) = (vec![1usize; n], vec![usize::MAX; n]);
    for i in 0..n {
        for j in 0..i {
            if dates[j] < dates[i] && len[j] + 1 > len[i] {
                len[i] = len[j] + 1;
                prev[i] = j;
            }
        }
    }
    let mut kept = vec![false; n];
    // The first index with the longest run, so ties keep the earlier days.
    let mut end = (0..n).fold(None, |best: Option<usize>, i| match best {
        Some(b) if len[b] >= len[i] => Some(b),
        _ => Some(i),
    });
    while let Some(i) = end.filter(|i| *i != usize::MAX) {
        kept[i] = true;
        end = Some(prev[i]);
    }
    (0..n).filter(|i| !kept[*i]).collect()
}
/// The old daily toggle shape; it is not migrated.
pub fn is_legacy_day(r: &Remote) -> bool {
    r.kind == "toggle" && DiaryDate::parse(r.text.trim()).is_some()
}
/// One day: its heading index and the exclusive end index of its range in a month listing.
#[derive(Debug, PartialEq, Eq)]
pub struct DayRange {
    pub iso: String,
    pub heading: usize,
    pub end: usize,
}
/// Splits a month listing at day headings. Blocks before the first heading belong to no day.
pub fn partition(items: &[Remote]) -> Vec<DayRange> {
    let mut out: Vec<DayRange> = Vec::new();
    for (i, r) in items.iter().enumerate() {
        if let Some(date) = day_of(r) {
            if let Some(last) = out.last_mut() {
                last.end = i;
            }
            out.push(DayRange {
                iso: date.to_string(),
                heading: i,
                end: items.len(),
            });
        }
    }
    out
}
pub const fn page_limit() -> usize {
    MAX_PAGES
}
pub const fn depth_limit() -> usize {
    MAX_DEPTH
}

/// Remote section content as comparable blocks plus the stable top-level IDs.
pub struct Section {
    pub blocks: Vec<Block>,
    pub top_ids: Vec<String>,
    lines: String,
}
impl Section {
    pub fn new(blocks: Vec<Block>, top_ids: Vec<String>, lines: String) -> Self {
        Self {
            blocks,
            top_ids,
            lines,
        }
    }
    pub fn digest(&self) -> String {
        hex_sha(canonical(&self.blocks).as_bytes())
    }
    /// Changes when any child's identity, type, text or order changes.
    pub fn fingerprint(&self, heading_id: &str) -> String {
        hex_sha(format!("{heading_id}\n{}", self.lines).as_bytes())
    }
}
pub fn to_block(r: &Remote, children: Vec<Block>) -> Block {
    match Kind::from_tag(&r.kind) {
        Some(Kind::Code(_)) => Block {
            kind: Kind::Code(r.lang.clone().unwrap_or_default()),
            text: r.text.clone(),
            children,
        },
        Some(kind) => Block {
            kind,
            text: r.text.clone(),
            children,
        },
        // An unknown block type must never compare equal to projected content.
        None => Block {
            kind: Kind::Paragraph,
            text: format!("\u{0}unsupported:{}", r.kind),
            children,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(kind: &str, text: &str) -> Remote {
        Remote {
            id: format!("{kind}:{text}"),
            kind: kind.into(),
            text: text.into(),
            title: None,
            has_children: false,
            plain: true,
            lang: None,
        }
    }

    #[test]
    fn partition_splits_on_dated_heading_two_only() {
        let items = [
            r("paragraph", "pre"),
            r("heading_2", "2026-11-01 (SUN)"),
            r("paragraph", "a"),
            r("heading_2", "notes"),
            r("paragraph", "b"),
            r("heading_2", "2026-11-03"),
        ];
        assert_eq!(
            partition(&items),
            [
                DayRange {
                    iso: "2026-11-01".into(),
                    heading: 1,
                    end: 5
                },
                DayRange {
                    iso: "2026-11-03".into(),
                    heading: 5,
                    end: 6
                },
            ]
        );
    }

    #[test]
    fn day_of_ignores_the_weekday_suffix() {
        let day = |kind: &str, text: &str| day_of(&r(kind, text)).map(|d| d.to_string());
        assert_eq!(
            day("heading_2", "2026-11-04 (FRI)").as_deref(),
            Some("2026-11-04")
        );
        assert_eq!(
            day("heading_2", "2026-11-04").as_deref(),
            Some("2026-11-04")
        );
        assert_eq!(day("heading_2", "2026-11-04x"), None);
        assert_eq!(day("heading_2", "2026-13-40"), None);
        assert_eq!(day("heading_3", "2026-11-04"), None);
        assert!(is_legacy_day(&r("toggle", "2026-11-04")));
        assert!(!is_legacy_day(&r("toggle", "notes")));
    }

    fn dates(days: &[&str]) -> Vec<String> {
        days.iter().map(|d| format!("2026-11-{d}")).collect()
    }

    #[test]
    fn plan_moves_leaves_an_ascending_page_alone() {
        assert!(plan_moves(&dates(&["01", "02", "09"])).is_empty());
        assert!(plan_moves(&[]).is_empty());
    }

    #[test]
    fn plan_moves_moves_one_misplaced_day() {
        // 05 and 03 tie for a place in the run; the earlier-positioned 05 stays.
        assert_eq!(plan_moves(&dates(&["01", "05", "03", "07"])), [2]);
        assert_eq!(plan_moves(&dates(&["02", "01", "03"])), [1]);
    }

    #[test]
    fn plan_moves_on_a_reversed_page_moves_all_but_one() {
        assert_eq!(plan_moves(&dates(&["05", "04", "03", "02", "01"])).len(), 4);
    }

    #[test]
    fn plan_moves_prefers_the_longer_run() {
        assert_eq!(plan_moves(&dates(&["03", "01", "02"])), [0]);
    }

    #[test]
    fn plan_moves_keeps_the_earliest_day_on_equal_length_ties() {
        assert_eq!(plan_moves(&dates(&["02", "01"])), [1]);
    }

    #[test]
    fn parse_page_marks_formatted_text_not_plain() {
        let item = |rich: Value| json!({"id": "x", "type": "paragraph", "has_children": false, "paragraph": {"rich_text": rich}});
        let text = |extra: Value| {
            let mut t = json!({"type":"text","text":{"content":"a"},"plain_text":"a","annotations":{"bold":false,"italic":false,"strikethrough":false,"underline":false,"code":false,"color":"default"},"href":null});
            for (k, v) in extra.as_object().unwrap() {
                if k == "href" {
                    t["href"] = v.clone();
                } else {
                    t["annotations"][k] = v.clone();
                }
            }
            t
        };
        let page = json!({"results":[
            item(json!([text(json!({"bold": true}))])),
            item(json!([text(json!({"href": "https://example.com"}))])),
            item(json!([text(json!({"color": "red"}))])),
            item(json!([text(json!({}))])),
            item(json!([])),
            json!({"id": "y", "type": "paragraph", "has_children": false, "paragraph": {"rich_text": [], "color": "blue"}}),
            json!({"id": "z", "type": "heading_2", "has_children": false, "heading_2": {"rich_text": [], "color": "default", "is_toggleable": true}}),
        ],"has_more":false,"next_cursor":null});
        let (items, _) = parse_page(&page).unwrap();
        let plain: Vec<bool> = items.iter().map(|i| i.plain).collect();
        assert_eq!(plain, [false, false, false, true, true, false, false]);
    }
}

//! JSON shapes for the Notion API: request bodies from projected blocks and remote responses
//! back into comparable blocks. Remote text is never logged.
use super::client::Error;
use crate::export::hex_sha;
use diary_domain::notion::{Block, Kind, canonical, split_rich};
use serde_json::{Map, Value, json};

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
pub fn toggle(title: &str) -> Value {
    typed("toggle", json!({"rich_text": rich(title)}))
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
}
fn plain(v: &Value) -> String {
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
/// The ID of the toggle titled `title` in an append reply. The reply may list more than the new
/// block, so position is never trusted.
pub fn new_toggle(v: &Value, title: &str, old: Option<&str>) -> Option<String> {
    let (items, _) = parse_page(v).ok()?;
    items
        .into_iter()
        .find(|r| r.kind == "toggle" && r.text.trim() == title && Some(r.id.as_str()) != old)
        .map(|r| r.id)
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
    pub fn fingerprint(&self, toggle_id: &str) -> String {
        hex_sha(format!("{toggle_id}\n{}", self.lines).as_bytes())
    }
}
pub fn to_block(r: &Remote, children: Vec<Block>) -> Block {
    match Kind::from_tag(&r.kind) {
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

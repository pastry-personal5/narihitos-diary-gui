//! Illustrated day pages: positional hierarchy, native JPEGs, stable identity and byte evidence.
use super::{
    adapter::normalize_page_id,
    client::{Client, Error},
    http::Method,
    steps::Steps,
    wire,
};
use crate::{
    export::{
        hex_sha,
        illustrated_snapshot::{Manifest, Node, digest},
    },
    sqlite::Db,
};
use diary_application::export::*;
use diary_domain::{
    authentic::DiaryDate,
    notion::{self, Block, Kind},
};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex, OnceLock};

pub struct Illustrated {
    client: Arc<Client>,
    root: String,
    workspace: OnceLock<String>,
    steps: Steps,
    lock: Mutex<()>,
}
struct Body {
    image_bytes: usize,
    nodes: Vec<Node>,
    ids: Vec<String>,
    evidence: Vec<String>,
}
impl Illustrated {
    pub fn new(client: Arc<Client>, root: &str, db: Arc<Db>) -> Option<Self> {
        let root = normalize_page_id(root)?;
        Some(Self {
            client,
            steps: Steps::new(db, hex_sha(format!("notion:{root}").as_bytes())),
            root,
            workspace: OnceLock::new(),
            lock: Mutex::new(()),
        })
    }
    fn list(&self, parent: &str) -> Result<Vec<Value>, Error> {
        let mut out = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..50 {
            let suffix = match &cursor {
                None => String::new(),
                Some(c)
                    if c.bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"-_=".contains(&b)) =>
                {
                    format!("&start_cursor={c}")
                }
                _ => return Err(Error::Malformed),
            };
            let page = self.client.call(
                Method::Get,
                &format!("/v1/blocks/{parent}/children?page_size=100{suffix}"),
                None,
            )?;
            out.extend(
                page["results"]
                    .as_array()
                    .ok_or(Error::Malformed)?
                    .iter()
                    .cloned(),
            );
            if !page["has_more"].as_bool().ok_or(Error::Malformed)? {
                return Ok(out);
            }
            cursor = Some(page["next_cursor"].as_str().ok_or(Error::Malformed)?.into());
        }
        Err(Error::Malformed)
    }
    /// Resolve a unique, already-sorted dated sibling set and the insertion predecessor.
    fn slot(
        &self,
        parent: &str,
        title: &str,
        level: usize,
    ) -> Result<(Option<String>, Option<String>), Error> {
        let mut previous: Option<String> = None;
        let mut found = None;
        let mut anchor = None;
        for b in self.list(parent)? {
            if b["type"] != "child_page" {
                continue;
            }
            let t = b["child_page"]["title"].as_str().ok_or(Error::Malformed)?;
            let key = match level {
                0 if t.len() == 4 && t.bytes().all(|c| c.is_ascii_digit()) => t,
                1 if t.len() == 7 && DiaryDate::parse(&format!("{t}-01")).is_some() => t,
                2 if t.get(..10).is_some_and(|s| DiaryDate::parse(s).is_some()) => &t[..10],
                _ => continue,
            };
            if previous.as_deref().is_some_and(|p| p >= key) {
                return Err(Error::Rejected);
            }
            previous = Some(key.into());
            let id = b["id"]
                .as_str()
                .filter(|id| normalize_page_id(id).is_some())
                .ok_or(Error::Malformed)?
                .to_string();
            let wanted = if level == 2 { &title[..10] } else { title };
            if key == wanted {
                found = Some(id.clone());
            }
            if key < wanted {
                anchor = Some(id);
            }
        }
        Ok((found, anchor))
    }
    fn locate(&self, date: &DiaryDate) -> Result<Option<String>, Error> {
        let mut parent = self.root.clone();
        for (level, title) in titles(date).iter().enumerate() {
            let (page, _) = self.slot(&parent, title, level)?;
            let Some(page) = page else {
                return Ok(None);
            };
            parent = page;
        }
        Ok(Some(parent))
    }
    fn body(&self, page: &str) -> Result<Body, Error> {
        let mut bytes = 0;
        let mut evidence = Vec::new();
        let (nodes, ids) = self.read_nodes(page, 0, &mut bytes, &mut evidence)?;
        Ok(Body {
            image_bytes: bytes,
            nodes,
            ids,
            evidence,
        })
    }
    fn read_nodes(
        &self,
        page: &str,
        depth: usize,
        total: &mut usize,
        evidence: &mut Vec<String>,
    ) -> Result<(Vec<Node>, Vec<String>), Error> {
        if depth > 4 {
            return Err(Error::Rejected);
        }
        let mut nodes = Vec::new();
        let mut ids = Vec::new();
        for b in self.list(page)? {
            let id = b["id"]
                .as_str()
                .filter(|id| normalize_page_id(id).is_some())
                .ok_or(Error::Malformed)?;
            let kind = b["type"].as_str().ok_or(Error::Malformed)?;
            ids.push(id.into());
            evidence.push(format!(
                "{id}:{}",
                b["last_edited_time"].as_str().unwrap_or("")
            ));
            if kind == "image" {
                if b[kind]["type"] != "file"
                    || b["has_children"] == true
                    || !wire::is_plain(&b[kind]["caption"])
                {
                    return Err(Error::Rejected);
                }
                let image = self
                    .client
                    .download(b[kind]["file"]["url"].as_str().ok_or(Error::Malformed)?)?;
                *total += image.len();
                // A replacement temporarily holds both bounded 64 MiB bodies.
                if *total > 128 * 1024 * 1024 {
                    return Err(Error::TooLarge);
                }
                nodes.push(Node::Photo {
                    index: 0,
                    sha256: hex_sha(&image),
                    caption: wire::plain(&b[kind]["caption"]),
                });
            } else {
                let mut block_kind = Kind::from_tag(kind).ok_or(Error::Rejected)?;
                if !wire::block_is_plain(&b[kind]) {
                    return Err(Error::Rejected);
                }
                if let Kind::Code(lang) = &mut block_kind {
                    *lang = b[kind]["language"].as_str().ok_or(Error::Malformed)?.into();
                }
                let mut block = wire::block(&Block {
                    kind: block_kind,
                    text: wire::plain(&b[kind]["rich_text"]),
                    children: vec![],
                });
                if b["has_children"] == true {
                    let (children, _) = self.read_nodes(id, depth + 1, total, evidence)?;
                    let mut inner = Vec::new();
                    for child in children {
                        match child {
                            Node::Text { block } => inner.push(block),
                            _ => return Err(Error::Rejected),
                        }
                    }
                    block[kind]["children"] = Value::Array(inner);
                }
                nodes.push(Node::Text { block });
            }
        }
        Ok((nodes, ids))
    }
    fn seen(page: &str, body: &Body) -> Observation {
        let semantic = digest(&body.nodes);
        Observation {
            present: true,
            available: true,
            fingerprint: hex_sha(
                format!("{page}:{semantic}:{}", body.evidence.join(":")).as_bytes(),
            ),
            digest: semantic,
            ids: page.into(),
            unavailable: None,
        }
    }
    fn observe_inner(&self, date: &DiaryDate) -> Observation {
        match self.locate(date).and_then(|page| match page {
            Some(id) => Ok(Self::seen(&id, &self.body(&id)?)),
            None => Ok(Observation::absent()),
        }) {
            Ok(o) => o,
            Err(e) => Observation::unavailable(match e {
                Error::Rejected => Unavailable::Structural,
                Error::Unauthorized | Error::NotFound => Unavailable::Unauthorized,
                Error::Throttled(_) => Unavailable::Throttled,
                Error::Network => Unavailable::Network,
                _ => Unavailable::Unreadable,
            }),
        }
    }
}
fn titles(date: &DiaryDate) -> [String; 3] {
    [
        format!("{:04}", date.year()),
        format!("{:04}-{:02}", date.year(), date.month()),
        notion::day_title(*date),
    ]
}

/// Each step commits intent before sending; no write is replayed after ambiguity.
struct Run<'a> {
    adapter: &'a Illustrated,
    date: String,
    wrote: bool,
    cancelled: &'a dyn Fn() -> bool,
}
impl Run<'_> {
    fn call(
        &mut self,
        kind: &str,
        parent: &str,
        body: Option<&Value>,
        method: Method,
        path: &str,
    ) -> Result<Value, Error> {
        if (self.cancelled)() {
            return Err(Error::Rejected);
        }
        let sha = body
            .map(|b| hex_sha(&serde_json::to_vec(b).unwrap()))
            .unwrap_or_default();
        let seq = self
            .adapter
            .steps
            .begin(&self.date, kind, parent, &sha)
            .map_err(|_| Error::Rejected)?;
        let response = self.adapter.client.call(method, path, body);
        self.finish(seq, &response)?;
        response
    }
    fn finish(&mut self, seq: i64, response: &Result<Value, Error>) -> Result<(), Error> {
        if response.is_ok() || matches!(response, Err(Error::Ambiguous)) {
            self.wrote = true;
        }
        let ids = response.as_ref().ok().and_then(|v| {
            v["id"].as_str().map(str::to_string).or_else(|| {
                v["results"].as_array().map(|v| {
                    v.iter()
                        .filter_map(|r| r["id"].as_str())
                        .collect::<Vec<_>>()
                        .join(",")
                })
            })
        });
        self.adapter
            .steps
            .finish_checked(&self.date, seq, response.is_ok(), ids.as_deref())
            .map_err(|_| Error::Ambiguous)
    }
    fn hierarchy(&mut self, date: &DiaryDate) -> Result<String, Error> {
        let mut parent = self.adapter.root.clone();
        for (level, title) in titles(date).iter().enumerate() {
            let (found, after) = self.adapter.slot(&parent, title, level)?;
            parent = match found {
                Some(id) => id,
                None => {
                    let mut body = wire::page(&parent, title);
                    body["position"] = match after {
                        Some(id) => json!({"type":"after_block","after_block":{"id":id}}),
                        None => json!({"type":"page_start"}),
                    };
                    let reply = self.call(
                        "create_page",
                        &parent,
                        Some(&body),
                        Method::Post,
                        "/v1/pages",
                    )?;
                    let id = reply["id"]
                        .as_str()
                        .filter(|id| normalize_page_id(id).is_some())
                        .ok_or(Error::Ambiguous)?
                        .to_string();
                    if self.adapter.slot(&parent, title, level)?.0.as_deref() != Some(&id) {
                        return Err(Error::Ambiguous);
                    }
                    id
                }
            };
        }
        Ok(parent)
    }
    fn upload(&mut self, jpeg: &[u8]) -> Result<String, Error> {
        let created = self.call(
            "create_upload",
            "",
            Some(&json!({"filename":"photo.jpg","content_type":"image/jpeg"})),
            Method::Post,
            "/v1/file_uploads",
        )?;
        let id = created["id"]
            .as_str()
            .filter(|id| normalize_page_id(id).is_some())
            .ok_or(Error::Ambiguous)?
            .to_string();
        if (self.cancelled)() {
            return Err(Error::Rejected);
        }
        let seq = self
            .adapter
            .steps
            .begin(&self.date, "send_upload", &id, &hex_sha(jpeg))
            .map_err(|_| Error::Rejected)?;
        let response = self.adapter.client.upload(&id, jpeg);
        self.finish(seq, &response)?;
        match response {
            Ok(value) if value["status"] == "uploaded" => {}
            Err(Error::Ambiguous) => {
                // A known upload has an exact read endpoint. Uploaded is definitive for this
                // immutable upload object; pending/expired/failed never authorize a replay.
                let state = self.adapter.client.call(
                    Method::Get,
                    &format!("/v1/file_uploads/{id}"),
                    None,
                )?;
                if state["status"] != "uploaded" {
                    return Err(Error::Ambiguous);
                }
                self.adapter
                    .steps
                    .finish_checked(&self.date, seq, true, Some(&id))
                    .map_err(|_| Error::Ambiguous)?;
            }
            Ok(_) => return Err(Error::Ambiguous),
            Err(error) => return Err(error),
        }
        let state =
            self.adapter
                .client
                .call(Method::Get, &format!("/v1/file_uploads/{id}"), None)?;
        if state["status"] != "uploaded" {
            return Err(Error::Ambiguous);
        }
        Ok(id)
    }
    fn go(
        &mut self,
        snapshot: &Snapshot,
        manifest: &Manifest,
        expected: &Observation,
    ) -> Result<(), Error> {
        let page = self.hierarchy(&snapshot.date)?;
        let old = self.adapter.body(&page)?;
        // Larger partial/foreign bodies need external repair rather than growing a third copy.
        if old.image_bytes > 64 * 1024 * 1024 {
            return Err(Error::TooLarge);
        }
        if expected.present && Self::fingerprint(&page, &old) != expected.fingerprint {
            return Err(Error::Rejected);
        }
        if !expected.present && !old.nodes.is_empty() {
            return Err(Error::Rejected);
        }
        let mut fresh = Vec::new();
        // Each JPEG is attached immediately after upload, so a 99-photo run cannot consume
        // an upload's one-hour unattached lifetime while processing the other photos.
        for node in &manifest.nodes {
            let block = match node {
                Node::Text { block } => block.clone(),
                Node::Photo {
                    index,
                    sha256,
                    caption,
                } => {
                    let jpeg = snapshot
                        .photos
                        .get(*index)
                        .filter(|b| hex_sha(b) == *sha256)
                        .ok_or(Error::Rejected)?;
                    let id = self.upload(jpeg)?;
                    json!({"object":"block","type":"image","image":{"type":"file_upload","file_upload":{"id":id},"caption":wire::rich(caption)}})
                }
            };
            let reply = self.call(
                "append",
                &page,
                Some(&json!({"children":[block]})),
                Method::Patch,
                &format!("/v1/blocks/{page}/children"),
            )?;
            let added = reply["results"].as_array().ok_or(Error::Ambiguous)?;
            let mut candidates = added
                .iter()
                .filter_map(|b| b["id"].as_str())
                .filter(|id| !old.ids.iter().chain(fresh.iter()).any(|known| known == id));
            let id = candidates
                .next()
                .filter(|id| normalize_page_id(id).is_some())
                .ok_or(Error::Ambiguous)?;
            if candidates.next().is_some() {
                return Err(Error::Ambiguous);
            }
            fresh.push(id.to_string());
        }
        let current = self.adapter.body(&page)?;
        let split = old.ids.len();
        if current.ids.len() != split + fresh.len()
            || current.ids[..split] != old.ids
            || current.ids[split..] != fresh
            || digest(&current.nodes[split..]) != digest(&manifest.nodes)
        {
            return Err(Error::Ambiguous);
        }
        // Verify new bytes and accepted old bytes before any cleanup. Unsupported blocks
        // were rejected on read. A Notion edit racing deletion remains a documented limit.
        if digest(&current.nodes[..split]) != digest(&old.nodes)
            || current.evidence[..old.evidence.len()] != old.evidence
        {
            return Err(Error::Ambiguous);
        }
        for id in &old.ids {
            self.call(
                "delete",
                id,
                None,
                Method::Delete,
                &format!("/v1/blocks/{id}"),
            )?;
        }
        let final_body = self.adapter.body(&page)?;
        if final_body.ids != fresh || digest(&final_body.nodes) != digest(&manifest.nodes) {
            return Err(Error::Ambiguous);
        }
        let seen = Illustrated::seen(&page, &final_body);
        let seq = self
            .adapter
            .steps
            .begin(&self.date, "verify", &page, &seen.fingerprint)
            .map_err(|_| Error::Ambiguous)?;
        self.adapter
            .steps
            .finish_checked(&self.date, seq, true, Some(&page))
            .map_err(|_| Error::Ambiguous)?;
        Ok(())
    }
    fn fingerprint(page: &str, body: &Body) -> String {
        Illustrated::seen(page, body).fingerprint
    }
}
impl ExportAdapter for Illustrated {
    fn progress(&self, date: &DiaryDate) -> Option<ExportProgress> {
        self.steps.progress(&date.to_string())
    }
    fn content_kind(&self) -> ContentKind {
        ContentKind::Illustrated
    }
    fn target(&self) -> Target {
        Target::Notion
    }
    fn root(&self) -> String {
        format!(
            "{}@{}",
            self.root,
            self.workspace.get().map_or("pending", String::as_str)
        )
    }
    fn ready(&self) -> Result<(), Unavailable> {
        if self.workspace.get().is_some() {
            return Ok(());
        }
        let v = self
            .client
            .call(Method::Get, "/v1/users/me", None)
            .map_err(|_| Unavailable::Unauthorized)?;
        let id = v["bot"]["workspace_id"]
            .as_str()
            .filter(|s| !s.is_empty() && s.len() <= 64)
            .ok_or(Unavailable::Unreadable)?;
        let _ = self.workspace.set(id.into());
        Ok(())
    }
    fn observe(&self, date: &DiaryDate) -> Observation {
        let Ok(_guard) = self.lock.lock() else {
            return Observation::unavailable(Unavailable::Unreadable);
        };
        self.observe_inner(date)
    }
    fn read_back(&self, date: &DiaryDate) -> Observation {
        self.observe(date)
    }
    fn verified(&self, date: &DiaryDate, seen: &Observation) -> bool {
        self.steps
            .verified(&date.to_string(), &seen.ids, &seen.fingerprint)
    }
    fn apply(&self, _: &DiaryDate, _: &[u8], _: Write, _: &Observation) -> Applied {
        Applied::Failed
    }
    fn apply_snapshot(
        &self,
        snapshot: &Snapshot,
        _: Write,
        expected: &Observation,
        cancelled: &dyn Fn() -> bool,
    ) -> Applied {
        let Ok(_guard) = self.lock.lock() else {
            return Applied::Failed;
        };
        let Ok(manifest) = serde_json::from_slice::<Manifest>(&snapshot.bytes) else {
            return Applied::Failed;
        };
        if manifest.nodes.iter().any(|n| matches!(n, Node::Photo { index, sha256, .. } if snapshot.photos.get(*index).is_none_or(|p| hex_sha(p) != *sha256))) { return Applied::Failed; }
        if manifest.version != 1 || manifest.nodes.iter().any(|n| matches!(n, Node::Text { block } if serde_json::to_vec(block).map_or(true, |b| b.len() > 390_000))) { return Applied::Failed; }
        let now = self.observe_inner(&snapshot.date);
        if !now.available
            || now.present != expected.present
            || now.fingerprint != expected.fingerprint
        {
            return Applied::Failed;
        }
        if self.steps.clear(&snapshot.date.to_string()).is_err() {
            return Applied::Failed;
        }
        let mut run = Run {
            adapter: self,
            date: snapshot.date.to_string(),
            wrote: false,
            cancelled,
        };
        match run.go(snapshot, &manifest, expected) {
            Ok(()) => Applied::Done,
            Err(_) if run.wrote => Applied::Unknown,
            Err(_) => Applied::Failed,
        }
    }
    fn warnings(&self, snapshot: &[u8]) -> u32 {
        serde_json::from_slice::<Manifest>(snapshot).map_or(0, |m| m.warnings)
    }
    fn digest_of(&self, snapshot: &[u8]) -> Option<String> {
        let m: Manifest = serde_json::from_slice(snapshot).ok()?;
        Some(digest(&m.nodes))
    }
}

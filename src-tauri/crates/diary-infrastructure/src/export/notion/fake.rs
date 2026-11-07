//! An in-memory Notion for tests and the fault-injection build. It implements the pagination,
//! positional insert, delete and page-create calls the adapter uses, plus scripted faults.
//! With a state file it survives a server restart so a script can inspect or edit the "remote".
use super::http::{Failure, Http, Method, Request, Response};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, VecDeque},
    path::PathBuf,
    sync::Mutex,
};

#[derive(Debug, Clone)]
pub enum Fault {
    /// A 429 with `Retry-After` seconds; nothing committed.
    Throttle(u64),
    /// A refusal status; nothing committed.
    Status(u16),
    /// The request is committed but the reply is lost.
    LoseReply,
    /// The request is lost before it commits.
    Drop,
    NotSent,
}

#[derive(Clone)]
struct Node {
    parent: String,
    kind: String,
    payload: Value,
    children: Vec<String>,
    title: Option<String>,
}
struct State {
    nodes: BTreeMap<String, Node>,
    next: u64,
}

pub struct FakeNotion {
    state: Mutex<State>,
    faults: Mutex<VecDeque<(String, usize, Fault)>>,
    calls: Mutex<Vec<String>>,
    page_size: usize,
    /// Append replies list every child of the parent, as the real service may.
    chatty: bool,
    file: Option<PathBuf>,
}

fn with_plain(mut payload: Value) -> Value {
    if let Some(items) = payload.get_mut("rich_text").and_then(Value::as_array_mut) {
        for i in items {
            let c = i["text"]["content"].clone();
            i["plain_text"] = c;
        }
    }
    payload
}

impl FakeNotion {
    pub fn new(root: &str) -> Self {
        let mut nodes = BTreeMap::new();
        nodes.insert(
            root.to_string(),
            Node {
                parent: String::new(),
                kind: "page".into(),
                payload: json!({}),
                children: vec![],
                title: Some("root".into()),
            },
        );
        Self {
            state: Mutex::new(State { nodes, next: 1 }),
            faults: Mutex::new(VecDeque::new()),
            calls: Mutex::new(vec![]),
            page_size: 100,
            chatty: false,
            file: None,
        }
    }
    /// State persists in `file`; an existing file is loaded.
    pub fn persistent(root: &str, file: PathBuf) -> Self {
        let mut fake = Self::new(root);
        if let Ok(text) = std::fs::read_to_string(&file)
            && let Ok(v) = serde_json::from_str::<Value>(&text)
        {
            let mut st = fake.state.lock().expect("fresh lock");
            st.next = v["next"].as_u64().unwrap_or(1);
            st.nodes = v["nodes"]
                .as_object()
                .map(|m| {
                    m.iter()
                        .map(|(k, n)| {
                            (
                                k.clone(),
                                Node {
                                    parent: n["parent"].as_str().unwrap_or("").into(),
                                    kind: n["kind"].as_str().unwrap_or("").into(),
                                    payload: n["payload"].clone(),
                                    children: n["children"]
                                        .as_array()
                                        .map(|a| {
                                            a.iter()
                                                .filter_map(|c| c.as_str().map(String::from))
                                                .collect()
                                        })
                                        .unwrap_or_default(),
                                    title: n["title"].as_str().map(String::from),
                                },
                            )
                        })
                        .collect()
                })
                .unwrap_or_default();
            drop(st);
        }
        fake.file = Some(file);
        fake
    }
    pub fn chatty(mut self) -> Self {
        self.chatty = true;
        self
    }
    pub fn language_of(&self, id: &str) -> Option<String> {
        let st = self.state.lock().unwrap();
        st.nodes
            .get(id)
            .and_then(|n| n.payload["language"].as_str().map(str::to_string))
    }
    pub fn with_page_size(mut self, size: usize) -> Self {
        self.page_size = size;
        self
    }
    pub fn fault(&self, op: &str, fault: Fault) {
        self.fault_after(op, 0, fault);
    }
    /// Applies the fault to the call after `skip` earlier calls of the same kind.
    pub fn fault_after(&self, op: &str, skip: usize, fault: Fault) {
        self.faults
            .lock()
            .unwrap()
            .push_back((op.into(), skip, fault));
    }
    pub fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
    pub fn count(&self, op: &str) -> usize {
        self.calls().iter().filter(|c| *c == op).count()
    }
    fn save(&self, st: &State) {
        if let Some(path) = &self.file {
            let nodes: serde_json::Map<String, Value> = st
                .nodes
                .iter()
                .map(|(k, n)| {
                    (
                        k.clone(),
                        json!({"parent":n.parent,"kind":n.kind,"payload":n.payload,"children":n.children,"title":n.title}),
                    )
                })
                .collect();
            let _ = std::fs::write(path, json!({"next":st.next,"nodes":nodes}).to_string());
        }
    }
    fn render(st: &State, id: &str) -> Value {
        let n = &st.nodes[id];
        let mut v =
            json!({"object":"block","id":id,"type":n.kind,"has_children":!n.children.is_empty()});
        v[n.kind.as_str()] = if n.kind == "child_page" {
            json!({"title": n.title})
        } else {
            n.payload.clone()
        };
        v
    }
    fn insert(st: &mut State, parent: &str, block: &Value) -> String {
        let kind = block["type"].as_str().unwrap_or("paragraph").to_string();
        let mut payload = block[kind.as_str()].clone();
        let kids = payload
            .as_object_mut()
            .and_then(|o| o.remove("children"))
            .and_then(|c| c.as_array().cloned())
            .unwrap_or_default();
        let id = format!("{:032x}", st.next);
        st.next += 1;
        st.nodes.insert(
            id.clone(),
            Node {
                parent: parent.into(),
                kind,
                payload: with_plain(payload),
                children: vec![],
                title: None,
            },
        );
        for k in kids {
            let c = Self::insert(st, &id, &k);
            st.nodes
                .get_mut(&id)
                .expect("just inserted")
                .children
                .push(c);
        }
        id
    }
    fn remove(st: &mut State, id: &str) {
        if let Some(n) = st.nodes.remove(id) {
            for c in n.children {
                Self::remove(st, &c);
            }
        }
    }
    /// Test helper: an edit made outside the app.
    pub fn edit_text(&self, id: &str, text: &str) {
        let mut st = self.state.lock().unwrap();
        if let Some(n) = st.nodes.get_mut(id) {
            n.payload["rich_text"] = with_plain(
                json!({"rich_text":[{"type":"text","text":{"content":text}}]}),
            )["rich_text"]
                .clone();
        }
        self.save(&st);
    }
    pub fn children(&self, id: &str) -> Vec<String> {
        self.state
            .lock()
            .unwrap()
            .nodes
            .get(id)
            .map(|n| n.children.clone())
            .unwrap_or_default()
    }
    pub fn kind_of(&self, id: &str) -> String {
        let st = self.state.lock().unwrap();
        st.nodes.get(id).map_or(String::new(), |n| n.kind.clone())
    }
    pub fn text_of(&self, id: &str) -> String {
        let st = self.state.lock().unwrap();
        st.nodes.get(id).map_or(String::new(), |n| {
            n.payload["rich_text"]
                .as_array()
                .map_or(String::new(), |a| {
                    a.iter().filter_map(|i| i["plain_text"].as_str()).collect()
                })
        })
    }
    pub fn title_of(&self, id: &str) -> Option<String> {
        self.state
            .lock()
            .unwrap()
            .nodes
            .get(id)
            .and_then(|n| n.title.clone())
    }
    /// Test helper: a page or block created outside the app, appended to `parent`.
    pub fn external_page(&self, parent: &str, title: &str) -> String {
        let mut st = self.state.lock().unwrap();
        let id = format!("{:032x}", st.next);
        st.next += 1;
        st.nodes.insert(
            id.clone(),
            Node {
                parent: parent.into(),
                kind: "child_page".into(),
                payload: json!({}),
                children: vec![],
                title: Some(title.into()),
            },
        );
        st.nodes.get_mut(parent).unwrap().children.push(id.clone());
        self.save(&st);
        id
    }
    pub fn external_block(&self, parent: &str, block: &Value, index: Option<usize>) -> String {
        let mut st = self.state.lock().unwrap();
        let id = Self::insert(&mut st, parent, block);
        let kids = &mut st.nodes.get_mut(parent).unwrap().children;
        kids.insert(index.unwrap_or(kids.len()), id.clone());
        self.save(&st);
        id
    }

    fn op(request: &Request) -> &'static str {
        match (request.method, request.path.as_str()) {
            (Method::Get, p) if p.starts_with("/v1/users/me") => "me",
            (Method::Get, _) => "list",
            (Method::Post, _) => "create_page",
            (Method::Patch, _) => "append",
            (Method::Delete, _) => "delete",
        }
    }
    fn reply(status: u16, body: Value) -> Result<Response, Failure> {
        Ok(Response {
            status,
            retry_after: None,
            body: body.to_string().into_bytes(),
        })
    }
    fn run(&self, request: &Request) -> Result<Response, Failure> {
        let (path, query) = request.path.split_once('?').unwrap_or((&request.path, ""));
        let body: Value = request
            .body
            .as_deref()
            .and_then(|b| serde_json::from_slice(b).ok())
            .unwrap_or(Value::Null);
        let mut st = self.state.lock().unwrap();
        let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();
        match (request.method, parts.as_slice()) {
            (Method::Get, ["v1", "users", "me"]) => Self::reply(
                200,
                json!({"object":"user","id":"bot-1","type":"bot","bot":{"workspace_id":"workspace-1"}}),
            ),
            (Method::Get, ["v1", "blocks", id, "children"]) => {
                let Some(node) = st.nodes.get(*id) else {
                    return Self::reply(404, json!({"code":"object_not_found"}));
                };
                let q = |k: &str| {
                    query
                        .split('&')
                        .find_map(|p| p.strip_prefix(&format!("{k}=")).map(String::from))
                };
                let size = q("page_size")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(100usize)
                    .min(self.page_size)
                    .max(1);
                let start: usize = q("start_cursor").and_then(|s| s.parse().ok()).unwrap_or(0);
                let end = (start + size).min(node.children.len());
                let results: Vec<Value> = node.children[start.min(end)..end]
                    .iter()
                    .map(|c| Self::render(&st, c))
                    .collect();
                let more = end < node.children.len();
                Self::reply(
                    200,
                    json!({"object":"list","results":results,"has_more":more,"next_cursor": more.then(|| end.to_string())}),
                )
            }
            (Method::Patch, ["v1", "blocks", id, "children"]) => {
                if !st.nodes.contains_key(*id) {
                    return Self::reply(404, json!({"code":"object_not_found"}));
                }
                let blocks = body["children"].as_array().cloned().unwrap_or_default();
                if blocks.len() > 100 {
                    return Self::reply(400, json!({"code":"validation_error"}));
                }
                let kids_len = st.nodes[*id].children.len();
                let at = match body["position"]["type"].as_str() {
                    Some("start") => 0,
                    Some("after_block") => {
                        let after = body["position"]["after_block"]["id"].as_str().unwrap_or("");
                        match st.nodes[*id].children.iter().position(|c| c == after) {
                            Some(i) => i + 1,
                            None => return Self::reply(400, json!({"code":"validation_error"})),
                        }
                    }
                    _ => kids_len,
                };
                let ids: Vec<String> = blocks
                    .iter()
                    .map(|b| Self::insert(&mut st, id, b))
                    .collect();
                st.nodes
                    .get_mut(*id)
                    .unwrap()
                    .children
                    .splice(at..at, ids.iter().cloned());
                let listed = if self.chatty {
                    st.nodes[*id].children.clone()
                } else {
                    ids
                };
                let results: Vec<Value> = listed.iter().map(|i| Self::render(&st, i)).collect();
                self.save(&st);
                Self::reply(
                    200,
                    json!({"object":"list","results":results,"has_more":false,"next_cursor":null}),
                )
            }
            (Method::Delete, ["v1", "blocks", id]) => {
                let Some(node) = st.nodes.get(*id).cloned() else {
                    return Self::reply(404, json!({"code":"object_not_found"}));
                };
                if let Some(p) = st.nodes.get_mut(&node.parent) {
                    p.children.retain(|c| c != id);
                }
                Self::remove(&mut st, id);
                self.save(&st);
                Self::reply(200, json!({"object":"block","id":id,"archived":true}))
            }
            (Method::Post, ["v1", "pages"]) => {
                let parent = body["parent"]["page_id"].as_str().unwrap_or("").to_string();
                if !st.nodes.contains_key(&parent) {
                    return Self::reply(404, json!({"code":"object_not_found"}));
                }
                let title = body["properties"]["title"]["title"][0]["text"]["content"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                let id = format!("{:032x}", st.next);
                st.next += 1;
                st.nodes.insert(
                    id.clone(),
                    Node {
                        parent: parent.clone(),
                        kind: "child_page".into(),
                        payload: json!({}),
                        children: vec![],
                        title: Some(title),
                    },
                );
                st.nodes.get_mut(&parent).unwrap().children.push(id.clone());
                self.save(&st);
                Self::reply(200, json!({"object":"page","id":id}))
            }
            _ => Self::reply(400, json!({"code":"invalid_request_url"})),
        }
    }
}

impl Http for FakeNotion {
    fn send(&self, request: &Request) -> Result<Response, Failure> {
        let op = Self::op(request);
        self.calls.lock().unwrap().push(op.into());
        let fault = {
            let mut f = self.faults.lock().unwrap();
            match f.iter().position(|(o, _, _)| o == op) {
                Some(i) if f[i].1 > 0 => {
                    f[i].1 -= 1;
                    None
                }
                Some(i) => f.remove(i).map(|(_, _, fault)| fault),
                None => None,
            }
        };
        match fault {
            None => self.run(request),
            Some(Fault::Throttle(s)) => Ok(Response {
                status: 429,
                retry_after: Some(s),
                body: b"{}".to_vec(),
            }),
            Some(Fault::Status(code)) => Self::reply(code, json!({"code":"fault"})),
            Some(Fault::Drop) => Err(Failure::Ambiguous),
            Some(Fault::NotSent) => Err(Failure::NotSent),
            Some(Fault::LoseReply) => {
                let _ = self.run(request);
                Err(Failure::Ambiguous)
            }
        }
    }
}

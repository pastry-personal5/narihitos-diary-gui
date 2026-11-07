//! The Notion destination: year page, month page and one toggle section per ISO date.
//!
//! Every remote write is preceded by a durable step row. An ambiguous or partially applied write
//! returns `Applied::Unknown`; the service then reads the remote back and never replays `apply`.
//! Notion has no compare-and-swap, so drift is detected by re-observing before and after writes,
//! not prevented.
use super::{
    client::{Client, Error},
    http::Method,
    steps::Steps,
    wire::{self, Section},
};
use crate::{export::hex_sha, sqlite::Db};
use diary_application::export::{Applied, ExportAdapter, Observation, Target, Unavailable, Write};
use diary_domain::{
    authentic::DiaryDate,
    notion::{self as projection, Block, canonical},
};
use serde_json::Value;
use std::sync::{Arc, Mutex, OnceLock};

/// Conservative request budget beneath the documented 500 KB payload cap.
const CHUNK_BYTES: usize = 400_000;

pub struct Notion {
    client: Client,
    root_page: String,
    /// Notion's `bot.workspace_id`, fetched on first use (O9).
    workspace: OnceLock<String>,
    pub(super) steps: Steps,
    lock: Mutex<()>,
}

enum Fail {
    Api(Error),
    Structural,
}
impl From<Error> for Fail {
    fn from(e: Error) -> Self {
        Self::Api(e)
    }
}
enum Stop {
    /// Nothing was committed.
    Failed,
    /// Something may have been committed.
    Unknown,
}

#[derive(Default)]
struct Located {
    year: Option<String>,
    month: Option<String>,
    /// Dated toggles of the month page in page order.
    dated: Vec<(String, String)>,
    section: Option<(String, Section)>,
}

/// The Markdown after the front matter of a rendered Authentic file.
fn body_of(bytes: &[u8]) -> Option<&str> {
    let rest = std::str::from_utf8(bytes).ok()?.strip_prefix("---\n")?;
    let end = if rest.starts_with("---\n") {
        0
    } else {
        rest.find("\n---\n")? + 1
    };
    rest.get(end + 4..)
}

pub fn normalize_page_id(id: &str) -> Option<String> {
    let id: String = id
        .chars()
        .filter(|c| *c != '-')
        .collect::<String>()
        .to_ascii_lowercase();
    (id.len() == 32 && id.bytes().all(|b| b.is_ascii_hexdigit())).then_some(id)
}

impl Notion {
    /// Registers the destination for a valid root page ID. The workspace identity is fetched
    /// lazily by [`ExportAdapter::ready`], so an unreachable service never blocks startup.
    pub fn new(client: Client, root_page_id: &str, db: Arc<Db>) -> Option<Self> {
        let root_page = normalize_page_id(root_page_id)?;
        let destination = hex_sha(format!("notion:{root_page}").as_bytes());
        Some(Self {
            client,
            root_page,
            workspace: OnceLock::new(),
            steps: Steps::new(db, destination),
            lock: Mutex::new(()),
        })
    }
    fn identity(&self) -> Result<&str, Error> {
        if let Some(w) = self.workspace.get() {
            return Ok(w);
        }
        let me = self.client.call(Method::Get, "/v1/users/me", None)?;
        let id = me["bot"]["workspace_id"]
            .as_str()
            .filter(|w| !w.is_empty() && w.len() <= 64)
            .ok_or(Error::Malformed)?;
        Ok(self.workspace.get_or_init(|| id.to_string()))
    }

    fn list(&self, id: &str) -> Result<Vec<wire::Remote>, Error> {
        let mut out = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..wire::page_limit() {
            let path = match &cursor {
                Some(c)
                    if c.bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"-_=".contains(&b)) =>
                {
                    format!("/v1/blocks/{id}/children?page_size=100&start_cursor={c}")
                }
                Some(_) => return Err(Error::Malformed),
                None => format!("/v1/blocks/{id}/children?page_size=100"),
            };
            let (items, next) = wire::parse_page(&self.client.call(Method::Get, &path, None)?)?;
            out.extend(items);
            match next {
                Some(c) => cursor = Some(c),
                None => return Ok(out),
            }
        }
        // More children than the bound: treat as unreadable instead of guessing.
        Err(Error::Malformed)
    }
    fn unique_page(&self, parent: &str, title: &str) -> Result<Option<String>, Fail> {
        let mut found = self
            .list(parent)?
            .into_iter()
            .filter(|r| r.kind == "child_page" && r.title.as_deref() == Some(title));
        let first = found.next().map(|r| r.id);
        if found.next().is_some() {
            return Err(Fail::Structural);
        }
        Ok(first)
    }
    /// Section content, or a structural conflict for any block outside the projection's output
    /// set (O12): a nested page or database must never be trashed by a replace.
    fn read_blocks(&self, id: &str, depth: usize) -> Result<Section, Fail> {
        let items = self.list(id)?;
        let (mut blocks, mut lines) = (Vec::new(), String::new());
        for r in &items {
            if projection::Kind::from_tag(&r.kind).is_none() {
                return Err(Fail::Structural);
            }
            let (children, sub) = if r.has_children {
                if depth >= wire::depth_limit() {
                    return Err(Fail::Api(Error::Malformed));
                }
                let s = self.read_blocks(&r.id, depth + 1)?;
                (s.blocks.clone(), s.fingerprint(""))
            } else {
                (Vec::new(), String::new())
            };
            lines.push_str(&format!("{}|{}|{}|{{{sub}}}\n", r.id, r.kind, r.text));
            blocks.push(wire::to_block(r, children));
        }
        Ok(Section::new(
            blocks,
            items.into_iter().map(|r| r.id).collect(),
            lines,
        ))
    }
    fn look(&self, date: &DiaryDate) -> Result<Located, Fail> {
        let year = format!("{:04}", date.year());
        let month = format!("{year}-{:02}", date.month());
        let mut loc = Located {
            year: self.unique_page(&self.root_page, &year)?,
            ..Located::default()
        };
        let Some(y) = loc.year.clone() else {
            return Ok(loc);
        };
        loc.month = self.unique_page(&y, &month)?;
        let Some(m) = loc.month.clone() else {
            return Ok(loc);
        };
        let iso = date.to_string();
        for r in self
            .list(&m)?
            .into_iter()
            .filter(|r| r.kind == "toggle" && DiaryDate::parse(r.text.trim()).is_some())
        {
            let day = r.text.trim().to_string();
            // Strictly ascending also rejects duplicates.
            if loc.dated.last().is_some_and(|(prev, _)| day <= *prev) {
                return Err(Fail::Structural);
            }
            loc.dated.push((day, r.id));
        }
        if let Some((_, id)) = loc.dated.iter().find(|(d, _)| *d == iso) {
            let section = self.read_blocks(id, 0)?;
            loc.section = Some((id.clone(), section));
        }
        Ok(loc)
    }
    fn observation(loc: &Located) -> Observation {
        match &loc.section {
            None => Observation::absent(),
            Some((id, s)) => Observation {
                present: true,
                available: true,
                fingerprint: s.fingerprint(id),
                digest: s.digest(),
                ids: id.clone(),
                unavailable: None,
            },
        }
    }
    fn failed(f: Fail) -> Observation {
        Observation::unavailable(match f {
            Fail::Structural => Unavailable::Structural,
            Fail::Api(Error::Unauthorized | Error::NotFound) => Unavailable::Unauthorized,
            Fail::Api(Error::Throttled(_)) => Unavailable::Throttled,
            Fail::Api(Error::Network) => Unavailable::Network,
            Fail::Api(_) => Unavailable::Unreadable,
        })
    }
    fn observe_locked(&self, date: &DiaryDate) -> Observation {
        if let Err(reason) = self.ready() {
            return Observation::unavailable(reason);
        }
        let Ok(_guard) = self.lock.lock() else {
            return Observation::unavailable(Unavailable::Unreadable);
        };
        self.look(date)
            .map_or_else(Self::failed, |l| Self::observation(&l))
    }
}

struct Run<'a> {
    n: &'a Notion,
    date: &'a str,
    wrote: bool,
}
impl Run<'_> {
    fn stop(&self) -> Stop {
        if self.wrote {
            Stop::Unknown
        } else {
            Stop::Failed
        }
    }
    fn begin(&self, kind: &str, parent: &str, digest: &str) -> Result<i64, Stop> {
        self.n
            .steps
            .begin(self.date, kind, parent, digest)
            .map_err(|_| self.stop())
    }
    fn after(
        &mut self,
        seq: i64,
        result: Result<Value, Error>,
        pick: impl FnOnce(&Value) -> Option<String>,
    ) -> Result<String, Stop> {
        match result {
            Ok(v) => {
                self.wrote = true;
                match pick(&v) {
                    Some(id) => {
                        self.n.steps.finish(self.date, seq, true, Some(&id));
                        Ok(id)
                    }
                    None => {
                        self.n.steps.finish(self.date, seq, false, None);
                        Err(Stop::Unknown)
                    }
                }
            }
            Err(e) => {
                self.n.steps.finish(self.date, seq, false, None);
                if matches!(e, Error::Ambiguous | Error::Malformed) {
                    self.wrote = true;
                    return Err(Stop::Unknown);
                }
                Err(self.stop())
            }
        }
    }
    fn page(&mut self, parent: &str, title: &str) -> Result<String, Stop> {
        let seq = self.begin("create_page", parent, "")?;
        let r = self
            .n
            .client
            .call(Method::Post, "/v1/pages", Some(&wire::page(parent, title)));
        self.after(seq, r, |v| v["id"].as_str().map(str::to_string))
    }
    fn section(
        &mut self,
        month: &str,
        date: &DiaryDate,
        dated: &[(String, String)],
        old: Option<&str>,
    ) -> Result<String, Stop> {
        let iso = date.to_string();
        // During a replace the new toggle goes directly after the old one (O10).
        let previous = old.or_else(|| {
            dated
                .iter()
                .rev()
                .find(|(d, _)| *d < iso)
                .map(|(_, id)| id.as_str())
        });
        let body = wire::append(
            vec![wire::toggle(&iso)],
            previous,
            previous.is_none() && !dated.is_empty(),
        );
        let seq = self.begin("create_section", month, "")?;
        let r = self.n.client.call(
            Method::Patch,
            &format!("/v1/blocks/{month}/children"),
            Some(&body),
        );
        self.after(seq, r, |v| wire::new_toggle(v, &iso, old))
    }
    fn append(
        &mut self,
        toggle: &str,
        chunk: &[Block],
        after: Option<&str>,
    ) -> Result<Option<String>, Stop> {
        let digest = hex_sha(canonical(chunk).as_bytes());
        let seq = self.begin("append", toggle, &digest)?;
        let body = wire::append(chunk.iter().map(wire::block).collect(), after, false);
        let r = self.n.client.call(
            Method::Patch,
            &format!("/v1/blocks/{toggle}/children"),
            Some(&body),
        );
        self.after(seq, r, |v| {
            v["results"].as_array()?.last()?["id"]
                .as_str()
                .map(str::to_string)
        })
        .map(Some)
    }
    fn delete(&mut self, id: &str) -> Result<(), Stop> {
        let seq = self.begin("delete", id, "")?;
        match self
            .n
            .client
            .call(Method::Delete, &format!("/v1/blocks/{id}"), None)
        {
            Err(Error::NotFound) => {
                self.n.steps.finish(self.date, seq, true, Some(id));
                Ok(())
            }
            r => self.after(seq, r, |_| Some(id.to_string())).map(|_| ()),
        }
    }
    fn go(
        &mut self,
        date: &DiaryDate,
        loc: Located,
        chunks: &[Vec<Block>],
        want: &str,
    ) -> Result<(), Stop> {
        let n = self.n;
        let old = loc.section.as_ref().map(|(id, _)| id.clone());
        let month = match &loc.month {
            Some(m) => m.clone(),
            None => {
                let year = match &loc.year {
                    Some(y) => y.clone(),
                    None => self.page(&n.root_page, &format!("{:04}", date.year()))?,
                };
                self.page(&year, &format!("{:04}-{:02}", date.year(), date.month()))?
            }
        };
        let toggle = self.section(&month, date, &loc.dated, old.as_deref())?;
        let mut after: Option<String> = None;
        for chunk in chunks {
            if let Some(id) = self.append(&toggle, chunk, after.as_deref())? {
                after = Some(id);
            }
        }
        if let Some(old) = old {
            // The old section stays until the new one is complete and verified, so a crash
            // never loses text; the two same-date sections are then a structural conflict.
            match n.read_blocks(&toggle, 0) {
                Ok(s) if s.digest() == want => {}
                _ => return Err(Stop::Unknown),
            }
            self.delete(&old)?;
        }
        Ok(())
    }
}

impl ExportAdapter for Notion {
    fn target(&self) -> Target {
        Target::Notion
    }
    fn root(&self) -> String {
        format!(
            "{}@{}",
            self.root_page,
            self.workspace.get().map_or("pending", String::as_str)
        )
    }
    fn ready(&self) -> Result<(), Unavailable> {
        self.identity().map(|_| ()).map_err(|e| match e {
            Error::Unauthorized | Error::NotFound => Unavailable::Unauthorized,
            Error::Throttled(_) => Unavailable::Throttled,
            Error::Network => Unavailable::Network,
            _ => Unavailable::Unreadable,
        })
    }
    fn observe(&self, date: &DiaryDate) -> Observation {
        self.observe_locked(date)
    }
    fn read_back(&self, date: &DiaryDate) -> Observation {
        self.observe_locked(date)
    }
    fn apply(
        &self,
        date: &DiaryDate,
        bytes: &[u8],
        write: Write,
        expected: &Observation,
    ) -> Applied {
        let Ok(_guard) = self.lock.lock() else {
            return Applied::Failed;
        };
        let Some(body) = body_of(bytes) else {
            return Applied::Failed;
        };
        let projected = projection::project(body);
        let want = hex_sha(canonical(&projected.blocks).as_bytes());
        let Ok(loc) = self.look(date) else {
            return Applied::Failed;
        };
        let seen = Self::observation(&loc);
        // A changed remote needs a new preview; nothing has been written yet.
        if seen.present != expected.present
            || seen.fingerprint != expected.fingerprint
            || (write == Write::Create) == loc.section.is_some()
        {
            return Applied::Failed;
        }
        let chunks = projection::chunk(&projected.blocks, wire::encoded_size, CHUNK_BYTES);
        if chunks
            .iter()
            .any(|c| c.len() == 1 && wire::encoded_size(&c[0]) > CHUNK_BYTES)
        {
            return Applied::Failed;
        }
        let iso = date.to_string();
        if self.steps.clear(&iso).is_err() {
            return Applied::Failed;
        }
        let mut run = Run {
            n: self,
            date: &iso,
            wrote: false,
        };
        match run.go(date, loc, &chunks, &want) {
            Err(Stop::Failed) => Applied::Failed,
            Err(Stop::Unknown) => Applied::Unknown,
            Ok(()) => match self.look(date) {
                Ok(l) if l.section.is_some() && Self::observation(&l).digest == want => {
                    Applied::Done
                }
                _ => Applied::Unknown,
            },
        }
    }
    fn digest_of(&self, snapshot: &[u8]) -> Option<String> {
        let projected = projection::project(body_of(snapshot)?);
        Some(hex_sha(canonical(&projected.blocks).as_bytes()))
    }
    fn warnings(&self, snapshot: &[u8]) -> u32 {
        body_of(snapshot).map_or(0, |b| projection::project(b).warnings)
    }
}

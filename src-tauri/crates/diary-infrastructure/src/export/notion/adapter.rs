//! The Notion destination: year page, month page and one heading-2 day range per ISO date.
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
use std::{
    collections::HashSet,
    sync::{Arc, Mutex, OnceLock},
};

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

/// Where the heading of a date that is not on the page yet goes in the month page.
#[derive(Default)]
enum Slot {
    Start,
    After(String),
    #[default]
    End,
}

/// One day of a month page: its date, heading and body block IDs.
struct Day {
    iso: String,
    heading: String,
    body: Vec<String>,
}

impl Day {
    fn of(items: &[wire::Remote], r: &wire::DayRange) -> Self {
        Self {
            iso: r.iso.clone(),
            heading: items[r.heading].id.clone(),
            body: items[r.heading + 1..r.end]
                .iter()
                .map(|i| i.id.clone())
                .collect(),
        }
    }
}

/// A month page split into days. Duplicate dates that are only an unfinished move (one range is
/// a prefix or suffix of the other) are reported as `twin`; any other duplicate is structural.
struct View {
    items: Vec<wire::Remote>,
    /// Days in page order, without the twin.
    ranges: Vec<wire::DayRange>,
    days: Vec<Day>,
    /// Indexes into `days` that must move to leave the page ascending.
    moves: Vec<usize>,
    twin: Option<Day>,
}

#[derive(Default)]
struct Located {
    year: Option<String>,
    month: Option<String>,
    /// Days out of order and a leftover half of an interrupted move, both repaired by `apply`.
    moves: Vec<usize>,
    twin: Option<Day>,
    /// The target day: its heading ID and body.
    section: Option<(String, Section)>,
    /// The last block of the target day (its heading when the body is empty).
    tail: Option<String>,
    slot: Slot,
    /// Every child ID of the month page when it was listed.
    ids: HashSet<String>,
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
    fn read_blocks(&self, id: &str, depth: usize, strict: bool) -> Result<Section, Fail> {
        self.section_of(&self.list(id)?, depth, strict)
    }
    /// `strict` also refuses formatted text, which a copy would silently flatten.
    fn section_of(
        &self,
        items: &[wire::Remote],
        depth: usize,
        strict: bool,
    ) -> Result<Section, Fail> {
        let (mut blocks, mut lines) = (Vec::new(), String::new());
        for r in items {
            if projection::Kind::from_tag(&r.kind).is_none() || (strict && !r.plain) {
                return Err(Fail::Structural);
            }
            let (children, sub) = if r.has_children {
                if depth >= wire::depth_limit() {
                    return Err(Fail::Api(Error::Malformed));
                }
                let s = self.read_blocks(&r.id, depth + 1, strict)?;
                (s.blocks.clone(), s.fingerprint(""))
            } else {
                (Vec::new(), String::new())
            };
            lines.push_str(&format!("{}|{}|{}|{{{sub}}}\n", r.id, r.kind, r.text));
            blocks.push(wire::to_block(r, children));
        }
        Ok(Section::new(
            blocks,
            items.iter().map(|r| r.id.clone()).collect(),
            lines,
        ))
    }
    /// The body of the day whose heading is `heading`, read from a fresh listing of the month.
    fn read_range(&self, month: &str, heading: &str) -> Result<Section, Fail> {
        let items = self.list(month)?;
        let range = wire::partition(&items)
            .into_iter()
            .find(|d| items[d.heading].id == heading)
            .ok_or(Fail::Api(Error::Malformed))?;
        self.section_of(&items[range.heading + 1..range.end], 0, false)
    }
    fn view(&self, month: &str) -> Result<View, Fail> {
        let items = self.list(month)?;
        // Old daily toggles are not migrated (owner, 2026-11-07).
        if items.iter().any(wire::is_legacy_day) {
            return Err(Fail::Structural);
        }
        let mut ranges = wire::partition(&items);
        let mut seen = HashSet::new();
        let dup: Vec<String> = ranges
            .iter()
            .filter(|r| !seen.insert(r.iso.clone()))
            .map(|r| r.iso.clone())
            .collect();
        let mut twin = None;
        match dup.as_slice() {
            [] => {}
            [iso] => {
                let at: Vec<usize> = (0..ranges.len())
                    .filter(|i| ranges[*i].iso == *iso)
                    .collect();
                let [a, b] = at[..] else {
                    return Err(Fail::Structural);
                };
                // Strict: a duplicate with formatting must never be dropped as a "copy".
                let read =
                    |r: &wire::DayRange| self.section_of(&items[r.heading + 1..r.end], 0, true);
                let (sa, sb) = (read(&ranges[a])?, read(&ranges[b])?);
                // Only a byte-identical copy or a bare heading (a copy whose body never landed)
                // is an unfinished move. A prefix or suffix could just as well be the old text of
                // an interrupted replace, so it is never guessed at.
                let drop = if sa.blocks == sb.blocks || sb.blocks.is_empty() {
                    b
                } else if sa.blocks.is_empty() {
                    a
                } else {
                    return Err(Fail::Structural);
                };
                twin = Some(Day::of(&items, &ranges.remove(drop)));
            }
            _ => return Err(Fail::Structural),
        }
        let days: Vec<Day> = ranges.iter().map(|r| Day::of(&items, r)).collect();
        let dates: Vec<String> = days.iter().map(|d| d.iso.clone()).collect();
        let moves = wire::plan_moves(&dates);
        Ok(View {
            items,
            ranges,
            days,
            moves,
            twin,
        })
    }
    /// Where the heading of `iso` goes: before the first later day, else at the end.
    fn slot_for(v: &View, iso: &str) -> Slot {
        match v.ranges.iter().find(|d| d.iso.as_str() > iso) {
            Some(d) if d.heading == 0 => Slot::Start,
            Some(d) => Slot::After(v.items[d.heading - 1].id.clone()),
            None => Slot::End,
        }
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
        let v = self.view(&m)?;
        loc.ids = v.items.iter().map(|r| r.id.clone()).collect();
        let iso = date.to_string();
        loc.slot = Self::slot_for(&v, &iso);
        if let Some(d) = v.ranges.iter().find(|d| d.iso == iso) {
            let id = v.items[d.heading].id.clone();
            let section = self.section_of(&v.items[d.heading + 1..d.end], 0, false)?;
            loc.tail = Some(v.items[d.end - 1].id.clone());
            loc.section = Some((id, section));
        }
        loc.moves = v.moves;
        loc.twin = v.twin;
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
    /// Month-page children seen so far, so a reply's new blocks can be told from neighbours.
    known: HashSet<String>,
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
    /// Inserts the day heading. A replace puts it right after the old day's last block (O10).
    fn heading(
        &mut self,
        month: &str,
        date: &DiaryDate,
        slot: &Slot,
        old_tail: Option<&str>,
    ) -> Result<String, Stop> {
        let title = projection::day_title(*date);
        let (after, start) = match (old_tail, slot) {
            (Some(tail), _) => (Some(tail), false),
            (None, Slot::Start) => (None, true),
            (None, Slot::After(id)) => (Some(id.as_str()), false),
            (None, Slot::End) => (None, false),
        };
        let body = wire::append(vec![wire::heading(&title)], after, start);
        let seq = self.begin("create_section", month, "")?;
        let r = self.n.client.call(
            Method::Patch,
            &format!("/v1/blocks/{month}/children"),
            Some(&body),
        );
        let known = self.known.clone();
        let id = self.after(seq, r, |v| wire::new_heading(v, &title, &known))?;
        self.known.insert(id.clone());
        Ok(id)
    }
    /// Appends one chunk to the month page after `after` and returns the chunk's last block.
    fn append(&mut self, month: &str, chunk: &[Block], after: &str) -> Result<String, Stop> {
        let digest = hex_sha(canonical(chunk).as_bytes());
        let seq = self.begin("append", month, &digest)?;
        let body = wire::append(chunk.iter().map(wire::block).collect(), Some(after), false);
        let r = self.n.client.call(
            Method::Patch,
            &format!("/v1/blocks/{month}/children"),
            Some(&body),
        );
        let known = self.known.clone();
        let mut fresh = Vec::new();
        let last = self.after(seq, r, |v| {
            fresh = wire::new_ids(v, &known);
            fresh.last().cloned()
        })?;
        self.known.extend(fresh);
        Ok(last)
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
    /// Moves every out-of-order day to its place (copy, verify, then delete the original) and
    /// removes a leftover half of an interrupted move. Every day to move is read strictly before
    /// the first write, so the usual refusals write nothing.
    fn reorder(&mut self, month: &str) -> Result<(), Stop> {
        let n = self.n;
        let v = n.view(month).map_err(|_| self.stop())?;
        for &i in &v.moves {
            let r = &v.ranges[i];
            n.section_of(&v.items[r.heading + 1..r.end], 0, true)
                .map_err(|_| self.stop())?;
        }
        let v_moves = v.moves.len();
        if let Some(t) = v.twin {
            self.delete_day(&t)?;
        }
        // shortcut: a later tie flip can pick a day not checked above; its strict read then fails
        // after earlier moves and the export returns `Unknown` (every copy was verified before
        // its original was deleted, so nothing is lost). Upgrade if that is ever seen.
        // Each pass places one day, so a page that is still unordered after this many passes is
        // not converging; stop rather than write forever.
        for _ in 0..=2 * v_moves + 2 {
            let v = n.view(month).map_err(|_| self.stop())?;
            let Some(i) = v.moves.iter().copied().max_by_key(|i| &v.days[*i].iso) else {
                return Ok(());
            };
            let (day, range) = (&v.days[i], &v.ranges[i]);
            let section = n
                .section_of(&v.items[range.heading + 1..range.end], 0, true)
                .map_err(|_| self.stop())?;
            let chunks = projection::chunk(&section.blocks, wire::encoded_size, CHUNK_BYTES);
            if chunks
                .iter()
                .any(|c| c.len() == 1 && wire::encoded_size(&c[0]) > CHUNK_BYTES)
            {
                return Err(self.stop());
            }
            let date = DiaryDate::parse(&day.iso).ok_or_else(|| self.stop())?;
            self.known = v.items.iter().map(|r| r.id.clone()).collect();
            let heading = self.heading(month, &date, &Notion::slot_for(&v, &day.iso), None)?;
            let mut after = heading.clone();
            for chunk in &chunks {
                after = self.append(month, chunk, &after)?;
            }
            match n.read_range(month, &heading) {
                Ok(s) if s.digest() == section.digest() => {}
                _ => return Err(Stop::Unknown),
            }
            self.delete_day(day)?;
        }
        Err(Stop::Unknown)
    }
    /// Body blocks in order, the heading last, so a half-deleted day still has its heading.
    fn delete_day(&mut self, day: &Day) -> Result<(), Stop> {
        for id in &day.body {
            self.delete(id)?;
        }
        self.delete(&day.heading)
    }
    fn go(
        &mut self,
        date: &DiaryDate,
        mut loc: Located,
        chunks: &[Vec<Block>],
        want: &str,
    ) -> Result<(), Stop> {
        let n = self.n;
        if let Some(month) = loc
            .month
            .clone()
            .filter(|_| !loc.moves.is_empty() || loc.twin.is_some())
        {
            self.reorder(&month)?;
            // The page changed, so the layout read before the reorder is stale.
            loc = n.look(date).map_err(|_| self.stop())?;
        }
        let old = loc
            .section
            .as_ref()
            .map(|(id, s)| (id.clone(), s.top_ids.clone()));
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
        self.known = loc.ids.clone();
        let old_tail = old.as_ref().and(loc.tail.as_deref());
        let heading = self.heading(&month, date, &loc.slot, old_tail)?;
        let mut after = heading.clone();
        for chunk in chunks {
            after = self.append(&month, chunk, &after)?;
        }
        if let Some((old_heading, old_body)) = old {
            // The old day stays until the new one is complete and verified, so a crash never
            // loses text; the two same-date days are then a structural conflict.
            match n.read_range(&month, &heading) {
                Ok(s) if s.digest() == want => {}
                _ => return Err(Stop::Unknown),
            }
            for id in &old_body {
                self.delete(id)?;
            }
            self.delete(&old_heading)?;
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
            known: HashSet::new(),
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

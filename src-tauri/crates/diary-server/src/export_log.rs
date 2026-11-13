//! Redacted debug logging for exports: one record per destination call (both targets) and one
//! per Notion HTTP exchange. Only tags, status codes and durations are logged, never content,
//! paths, page IDs or the credential.
use diary_application::{
    diagnostics::{Diagnostics, EventCode, ExportDetail, Level, Record, Token},
    export::{
        Applied, ExportAdapter, ExportDate as DiaryDate, Observation, Target, Unavailable, Write,
    },
};
use diary_infrastructure::export::notion::http::{Failure, Http, Method, Request, Response};
use std::{sync::Arc, time::Instant};

fn ms(t: Instant) -> u32 {
    u32::try_from(t.elapsed().as_millis()).unwrap_or(u32::MAX)
}

pub struct LoggedAdapter {
    inner: Arc<dyn ExportAdapter>,
    diag: Arc<dyn Diagnostics>,
}
impl LoggedAdapter {
    pub fn new(inner: Arc<dyn ExportAdapter>, diag: Arc<dyn Diagnostics>) -> Self {
        Self { inner, diag }
    }
    fn emit(&self, level: Level, d: ExportDetail) {
        let d = d.target(self.inner.target().as_str());
        self.diag
            .emit(Record::new(EventCode::ExportAdapterCall, level).export(d));
    }
    fn seen(&self, stage: &str, t: Instant, o: &Observation) {
        let state = match (o.available, o.present) {
            (false, _) => "unavailable",
            (true, true) => "present",
            (true, false) => "absent",
        };
        let mut d = ExportDetail::new(stage).state(state).ms(ms(t));
        if let Some(u) = o.unavailable {
            d = d.reason(u.as_str());
        }
        self.emit(
            if o.available {
                Level::Info
            } else {
                Level::Warn
            },
            d,
        );
    }
}
impl ExportAdapter for LoggedAdapter {
    fn progress(&self, date: &DiaryDate) -> Option<diary_application::export::ExportProgress> {
        self.inner.progress(date)
    }
    fn content_kind(&self) -> diary_application::export::ContentKind {
        self.inner.content_kind()
    }
    fn verified(&self, date: &DiaryDate, seen: &Observation) -> bool {
        self.inner.verified(date, seen)
    }
    fn apply_snapshot(
        &self,
        snapshot: &diary_application::export::Snapshot,
        write: Write,
        expected: &Observation,
        cancelled: &dyn Fn() -> bool,
    ) -> Applied {
        let t = Instant::now();
        let applied = self
            .inner
            .apply_snapshot(snapshot, write, expected, cancelled);
        let (state, level) = match applied {
            Applied::Done => ("done", Level::Info),
            Applied::Failed => ("failed", Level::Warn),
            Applied::Unknown => ("unknown", Level::Warn),
        };
        self.emit(
            level,
            ExportDetail::new("apply")
                .state(state)
                .code(write.as_str())
                .ms(ms(t)),
        );
        applied
    }
    fn target(&self) -> Target {
        self.inner.target()
    }
    fn root(&self) -> String {
        self.inner.root()
    }
    fn observe(&self, date: &DiaryDate) -> Observation {
        let t = Instant::now();
        let o = self.inner.observe(date);
        self.seen("observe", t, &o);
        o
    }
    fn apply(
        &self,
        date: &DiaryDate,
        bytes: &[u8],
        write: Write,
        expected: &Observation,
    ) -> Applied {
        let t = Instant::now();
        let a = self.inner.apply(date, bytes, write, expected);
        let (state, level) = match a {
            Applied::Done => ("done", Level::Info),
            Applied::Failed => ("failed", Level::Warn),
            Applied::Unknown => ("unknown", Level::Warn),
        };
        self.emit(
            level,
            ExportDetail::new("apply")
                .state(state)
                .code(write.as_str())
                .ms(ms(t)),
        );
        a
    }
    fn read_back(&self, date: &DiaryDate) -> Observation {
        let t = Instant::now();
        let o = self.inner.read_back(date);
        self.seen("read_back", t, &o);
        o
    }
    fn ready(&self) -> Result<(), Unavailable> {
        let t = Instant::now();
        let r = self.inner.ready();
        let mut d = ExportDetail::new("ready").ms(ms(t));
        d = match r {
            Ok(()) => d.state("ready"),
            Err(u) => d.state("unavailable").reason(u.as_str()),
        };
        self.emit(if r.is_ok() { Level::Info } else { Level::Warn }, d);
        r
    }
    fn digest_of(&self, snapshot: &[u8]) -> Option<String> {
        self.inner.digest_of(snapshot)
    }
    fn warnings(&self, snapshot: &[u8]) -> u32 {
        self.inner.warnings(snapshot)
    }
}

/// Wraps the Notion transport: method, status, the API's error `code` tag and timing.
pub struct LoggedHttp<H> {
    pub inner: H,
    pub diag: Arc<dyn Diagnostics>,
}
impl<H: Http> Http for LoggedHttp<H> {
    fn download(&self, url: &str, limit: usize) -> Result<Vec<u8>, Failure> {
        self.inner.download(url, limit)
    }
    fn send(&self, request: &Request) -> Result<Response, Failure> {
        let t = Instant::now();
        let result = self.inner.send(request);
        let method = match request.method {
            Method::Get => "get",
            Method::Post => "post",
            Method::Patch => "patch",
            Method::Delete => "delete",
        };
        let d = ExportDetail::new(method).target("notion").ms(ms(t));
        let (level, d) = match &result {
            Ok(r) => {
                let d = d.status(r.status);
                if r.status >= 400 {
                    // Notion's error `code` (e.g. object_not_found) is a fixed vocabulary.
                    let code =
                        error_code(&r.body).map_or(d.clone(), |c| d.clone().code(c.as_str()));
                    let d = match r.retry_after {
                        Some(s) => code.checkpoint(s),
                        None => code,
                    };
                    (Level::Warn, d)
                } else {
                    (Level::Info, d)
                }
            }
            Err(Failure::NotSent) => (Level::Warn, d.code("not_sent")),
            Err(Failure::Ambiguous) => (Level::Warn, d.code("ambiguous")),
        };
        self.diag
            .emit(Record::new(EventCode::NotionHttp, level).export(d));
        result
    }
}

/// The `"code":"..."` value of a Notion error body, kept only if it is a plain tag.
fn error_code(body: &[u8]) -> Option<Token> {
    let text = std::str::from_utf8(body).ok()?;
    let rest = &text[text.find("\"code\"")? + 6..];
    let rest = rest
        .trim_start()
        .strip_prefix(':')?
        .trim_start()
        .strip_prefix('"')?;
    Token::parse(&rest[..rest.find('"')?])
}

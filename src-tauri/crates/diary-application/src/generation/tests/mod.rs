//! Shared generation fixtures and fakes; tests live in the feature modules.
use super::*;
use crate::{
    Limits,
    authentic::*,
    imports::*,
    operations::{OperationId, Record},
    source::*,
};
use diary_domain::source::{
    front_matter::Newline,
    stamp::{Dating, Instant, Precision, StampError},
    window::ImportZone,
};
use std::{
    cell::RefCell,
    sync::atomic::{AtomicBool, Ordering},
};
const DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
fn date() -> DiaryDate {
    DiaryDate::parse("2026-11-06").unwrap()
}
fn profile() -> ProfileHandle {
    ProfileHandle {
        alias: "p".into(),
        kind: AgentKind::Codex,
    }
}
fn frozen(count: u8) -> GenerationContext {
    GenerationContext {
        prompt: crate::agent::test_prompts::definition(PromptSelection {
            provider: AgentKind::Codex,
            action: PromptAction::GenerateCandidate,
        }),
        date: date(),
        profile: profile(),
        locale: Locale::parse("en-US").unwrap(),
        count,
        baseline: None,
        items: vec![SnippetItem {
            id: "snippet-1".into(),
            instant_utc: "2026-11-06T00:00:00Z".into(),
            civil_date: date().to_string(),
            offset: "+09:00".into(),
            body: "PRIVATE_SOURCE".into(),
        }],
    }
}
fn coordinator() -> Coordinator {
    let mut c = Coordinator::new("i".into(), Limits::default());
    c.open("s", 0).unwrap();
    c
}
fn admit(store: &mut Store, coord: &mut Coordinator, id: &str, f: GenerationContext) -> Shared {
    let (job, g) = store
        .admit(
            coord,
            Admission {
                session: "s",
                now: 0,
                request_id: id,
                digest: DIGEST,
                job_id: id,
            },
            f,
        )
        .unwrap();
    coord
        .transition("s", 0, &job.id, JobState::Running)
        .unwrap();
    g
}
fn candidate() -> Result<AgentOutput, AgentError> {
    Ok(AgentOutput::Candidate(Candidate {
        draft_markdown: "PRIVATE_DRAFT".into(),
        claim_support: vec![],
        warnings: vec![],
    }))
}
struct Fake {
    calls: RefCell<Vec<AgentRequest>>,
    results: RefCell<Vec<Result<AgentOutput, AgentError>>>,
    cancel: Option<Run>,
}
impl Fake {
    fn new(results: Vec<Result<AgentOutput, AgentError>>) -> Self {
        Self {
            calls: Default::default(),
            results: RefCell::new(results.into_iter().rev().collect()),
            cancel: None,
        }
    }
}
impl PromptResolver for Fake {
    fn resolve(&self, selection: PromptSelection) -> Result<ResolvedPrompt, AgentError> {
        Ok(crate::agent::test_prompts::definition(selection))
    }
}
impl AgentRunner for Fake {
    fn check(&self, _: &ProfileHandle) -> ProfileCheck {
        ProfileStatus::Ready.into()
    }
    fn run_prepared(
        &self,
        prepared: PreparedAgentRun,
        sink: &mut dyn EventSink,
        _: &CancelToken,
    ) -> Result<AgentOutput, AgentError> {
        let r = prepared.into_request();
        self.calls.borrow_mut().push(r);
        sink.event(AgentEvent::Delta("PRIVATE_PREVIEW".into()));
        if let Some(run) = &self.cancel {
            run.request_cancel();
        }
        self.results
            .borrow_mut()
            .pop()
            .expect("no hidden extra call")
    }
}
fn terminal(store: &Store, c: &mut Coordinator, g: &Shared) {
    let g = g.lock().unwrap();
    let state = g.terminal.unwrap();
    let id = g.job_id.clone();
    drop(g);
    c.transition("s", 0, &id, state).unwrap();
    store.settle(c, "s", 0, &id).unwrap();
}
struct Imports {
    changed: bool,
    wrong_date: bool,
    stale: bool,
    reads: std::sync::atomic::AtomicUsize,
}
impl Imports {
    fn new() -> Self {
        Self {
            changed: false,
            wrong_date: false,
            stale: false,
            reads: Default::default(),
        }
    }
}
fn row(id: &str, t: i64) -> IncludedSource {
    IncludedSource {
        instant: Instant {
            unix_ms: t,
            precision: Precision::Milli,
        },
        source: SourceListing {
            id: id.into(),
            rel: format!("PRIVATE_PATH/{id}"),
            size: 10,
            dating: Dating::Agreed(Instant {
                unix_ms: t,
                precision: Precision::Milli,
            }),
            token: ObservationToken {
                root_namespace: [0; 32],
                key: id.into(),
                sha256: [0; 32],
                dev: 0,
                ino: 0,
                size: 10,
                mtime_ns: 0,
                ctime_ns: 0,
                config_generation: 1,
                import_id: Some("import".into()),
            },
            warnings: vec![],
        },
    }
}
impl ImportStore for Imports {
    fn list(&self, _: &str, _: &dyn Fn() -> bool) -> Result<ImportListing, ImportError> {
        let included = vec![row("b", 1000), row("a", 0)];
        let mut nav = Navigation::default();
        for s in &included {
            nav.sources.insert(
                s.source.id.clone(),
                SourceDisplay {
                    group_id: "g".into(),
                    header: None,
                    filename: None,
                    included: Some(LocalTime {
                        date: date(),
                        offset_seconds: 32400,
                        datetime: "captured".into(),
                    }),
                    authority: None,
                },
            );
        }
        let mut unknown = row("unknown", 0).source;
        unknown.dating = Dating::Undated {
            header: StampError::Format,
            filename: StampError::Format,
        };
        Ok(ImportListing {
            context: ImportContext {
                id: "import".into(),
                date: if self.wrong_date {
                    date().next().unwrap()
                } else {
                    date()
                },
                zone: ImportZone {
                    name: "Asia/Seoul".into(),
                    database_version: "x".into(),
                },
                config_generation: if self.stale { 2 } else { 1 },
                completed_at: Some(0),
            },
            included,
            warnings: vec![unknown],
            misplaced: vec![],
            scan: Default::default(),
            navigation: Some(nav),
        })
    }
    fn read(
        &self,
        _: &str,
        id: &str,
        _: u64,
        _: &dyn Fn() -> bool,
    ) -> Result<SourceView, ViewError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        if self.changed {
            return Err(ViewError::Stale);
        };
        Ok(SourceView {
            listing: row(id, if id == "a" { 0 } else { 1000 }).source,
            body: format!("body {id}"),
            raw_front_matter: "PRIVATE_HEADER".into(),
            fields: vec![],
            newline: Newline::Lf,
            bom: false,
            write: Ok(()),
            trash: Ok(()),
        })
    }
    fn import_date(
        &self,
        _: &str,
        _: DiaryDate,
        _: u64,
        _: &dyn Fn() -> bool,
    ) -> Result<ImportListing, ImportError> {
        unreachable!()
    }
    fn resolve(
        &self,
        _: &str,
        _: &str,
        _: &[u8; 32],
        _: Authority,
        _: u64,
        _: &dyn Fn() -> bool,
    ) -> Result<(), ImportError> {
        unreachable!()
    }
}
struct Authentic {
    present: AtomicBool,
}
impl AuthenticStore for Authentic {
    fn load(&self, d: &DiaryDate) -> Result<Loaded, StoreError> {
        if self.present.load(Ordering::SeqCst) {
            Err(StoreError::Unavailable)
        } else {
            Ok(Loaded::Virtual(*d))
        }
    }
    fn list(&self, _: Option<DiaryDate>, _: usize) -> Result<Page, StoreError> {
        unreachable!()
    }
    fn save(&self, _: SaveRequest) -> Result<SaveOutcome, StoreError> {
        unreachable!()
    }
    fn create_empty(&self, _: CreateRequest) -> Result<SaveOutcome, StoreError> {
        unreachable!()
    }
    fn refresh(&self) -> Result<RebuildReport, StoreError> {
        unreachable!()
    }
    fn operation(&self, _: &OperationId) -> Result<Option<Record>, StoreError> {
        unreachable!()
    }
}
fn input(ids: &[&str]) -> Capture {
    Capture {
        prompt: crate::agent::test_prompts::definition(PromptSelection {
            provider: AgentKind::Codex,
            action: PromptAction::GenerateCandidate,
        }),
        date: date(),
        selections: ids
            .iter()
            .map(|id| Selection {
                import_id: "import".into(),
                source_id: (*id).into(),
            })
            .collect(),
        profile: profile(),
        locale: Locale::parse("ko-KR").unwrap(),
        count: 3,
        config_generation: 1,
    }
}

mod capture;
mod run;
mod workflow;

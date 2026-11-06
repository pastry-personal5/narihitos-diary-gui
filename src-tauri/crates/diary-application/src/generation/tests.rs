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
fn frozen(count: u8) -> Frozen {
    Frozen {
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
fn admit(store: &mut Store, coord: &mut Coordinator, id: &str, f: Frozen) -> Shared {
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
impl AgentRunner for Fake {
    fn check(&self, _: &ProfileHandle) -> ProfileCheck {
        ProfileStatus::Ready.into()
    }
    fn run(
        &self,
        r: AgentRequest,
        sink: &mut dyn EventSink,
        _: &CancelToken,
    ) -> Result<AgentOutput, AgentError> {
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
#[test]
fn styles_in_order_and_one_call_per_candidate() {
    let mut c = coordinator();
    let mut s = Store::default();
    let g = admit(&mut s, &mut c, "j", frozen(5));
    let fake = Fake::new((0..5).map(|_| candidate()).collect());
    execute(&g, &fake, &|| 0, &mut || {});
    assert_eq!(fake.calls.borrow().len(), 5);
    for (i, r) in fake.calls.borrow().iter().enumerate() {
        let Mode::Candidate {
            index,
            count,
            packet,
        } = r.mode()
        else {
            panic!()
        };
        assert_eq!((*index, *count), (i as u8 + 1, 5));
        assert_eq!(packet.style(), Style::ALL[i]);
    }
    assert_eq!(g.lock().unwrap().counts().ready, 5);
}
#[test]
fn partial_failure_keeps_ready_and_zero_valid_is_failure() {
    for (results, expected, ready) in [
        (
            vec![
                candidate(),
                Err(AgentErrorCode::Timeout.into()),
                candidate(),
            ],
            JobState::Succeeded,
            2,
        ),
        (
            vec![
                Err(AgentErrorCode::Context.into()),
                Err(AgentErrorCode::Failed.into()),
                Err(AgentErrorCode::Timeout.into()),
            ],
            JobState::Failed,
            0,
        ),
    ] {
        let mut c = coordinator();
        let mut s = Store::default();
        let g = admit(&mut s, &mut c, "j", frozen(3));
        let fake = Fake::new(results);
        execute(&g, &fake, &|| 0, &mut || {});
        let g = g.lock().unwrap();
        assert_eq!(g.terminal, Some(expected));
        assert_eq!(g.counts().ready, ready);
        assert_eq!(g.counts().finished, 3);
        assert_eq!(fake.calls.borrow().len(), 3);
    }
}
#[test]
fn cancel_keeps_ready_and_counts_slots_late_result_dropped() {
    let mut c = coordinator();
    let mut s = Store::default();
    let g = admit(&mut s, &mut c, "j", frozen(4));
    let fake = Fake::new(vec![candidate(), candidate()]);
    let token = g.lock().unwrap().run.token();
    let cancel = Run::from(token);
    execute(&g, &fake, &|| 0, &mut || {
        if fake.calls.borrow().len() == 2 {
            cancel.request_cancel();
        }
    });
    let g = g.lock().unwrap();
    assert_eq!(
        g.slots,
        vec![
            Slot::Ready,
            Slot::Cancelled,
            Slot::NotStarted,
            Slot::NotStarted
        ]
    );
    assert!(g.candidates[1].is_none());
    assert_eq!(g.terminal, Some(JobState::Cancelled));
    assert_eq!(
        g.counts(),
        Counts {
            requested: 4,
            finished: 2,
            ready: 1,
            failed: 0,
            cancelled: 1,
            not_started: 2
        }
    );
}
#[test]
fn busy_between_candidates_and_cancel_between_candidates() {
    let mut c = coordinator();
    let mut s = Store::default();
    let g = admit(&mut s, &mut c, "j", frozen(3));
    let fake = Fake::new(vec![candidate()]);
    execute(&g, &fake, &|| 0, &mut || {
        let g = g.lock().unwrap();
        let snap = g.shared.lock().unwrap();
        if snap.phase == Some(Phase::BetweenCandidates) {
            assert_eq!(
                c.admit_agent("s", 0, "other", DIGEST, "other", 0),
                Err(crate::Error::Busy)
            );
            assert!(snap.text.is_empty());
            g.cancel();
        }
    });
    assert_eq!(fake.calls.borrow().len(), 1);
    let g = g.lock().unwrap();
    assert_eq!(g.counts().not_started, 2);
    assert_eq!(g.terminal, Some(JobState::Cancelled));
}
#[test]
fn lease_end_cancels_generation() {
    let mut c = coordinator();
    let mut s = Store::default();
    let g = admit(&mut s, &mut c, "j", frozen(3));
    s.clear();
    let fake = Fake::new(vec![]);
    execute(&g, &fake, &|| 0, &mut || {});
    assert!(fake.calls.borrow().is_empty());
    assert_eq!(g.lock().unwrap().terminal, Some(JobState::Cancelled));
    assert!(s.get("s", "j").is_none());
}
#[test]
fn store_cap_refuses_admission_and_discard_keeps_admission_record() {
    let mut c = coordinator();
    let mut s = Store::default();
    for i in 0..MAX_JOBS {
        let id = format!("j{i}");
        let g = admit(&mut s, &mut c, &id, frozen(1));
        execute(&g, &Fake::new(vec![candidate()]), &|| 0, &mut || {});
        terminal(&s, &mut c, &g);
    }
    let a = Admission {
        session: "s",
        now: 0,
        request_id: "overflow",
        digest: DIGEST,
        job_id: "overflow",
    };
    assert!(matches!(
        s.admit(&mut c, a, frozen(1)),
        Err(Error::DiscardRequired)
    ));
    assert!(s.get("foreign", "j0").is_none());
    let a = Admission {
        session: "s",
        now: 0,
        request_id: "j0",
        digest: DIGEST,
        job_id: "unused",
    };
    assert_eq!(s.admit(&mut c, a, frozen(1)).unwrap().0.id, "j0");
    s.discard(&mut c, "s", 0, "j0").unwrap();
    assert!(s.get("s", "j0").is_none());
    assert_eq!(
        c.lookup_request("s", 0, "j0", DIGEST).unwrap().unwrap().id,
        "j0"
    );
    let a = Admission {
        session: "s",
        now: 0,
        request_id: "j0",
        digest: DIGEST,
        job_id: "unused",
    };
    assert!(matches!(
        s.admit(&mut c, a, frozen(1)),
        Err(Error::NotFound)
    ));
    let g = admit(&mut s, &mut c, "new", frozen(1));
    assert_eq!(g.lock().unwrap().job_id, "new");
}
#[test]
fn output_budget_failure_keeps_ready() {
    let mut c = coordinator();
    let mut s = Store::default();
    let g = admit(&mut s, &mut c, "j", frozen(3));
    let mut big = Candidate {
        draft_markdown: "x".repeat(MAX_TEXT_BYTES),
        claim_support: vec![],
        warnings: vec![],
    };
    big.warnings = vec!["w".repeat(MAX_STORE_BYTES)];
    let fake = Fake::new(vec![
        candidate(),
        Ok(AgentOutput::Candidate(big)),
        candidate(),
    ]);
    execute(&g, &fake, &|| 0, &mut || {});
    let g = g.lock().unwrap();
    assert_eq!(
        g.slots,
        vec![
            Slot::Ready,
            Slot::Failed(AgentErrorCode::Context),
            Slot::Ready
        ]
    );
    assert_eq!(g.terminal, Some(JobState::Succeeded));
}
#[test]
fn discard_releases_content_bytes() {
    let mut c = coordinator();
    let mut s = Store::default();
    let g = admit(&mut s, &mut c, "j", frozen(1));
    execute(&g, &Fake::new(vec![candidate()]), &|| 0, &mut || {});
    terminal(&s, &mut c, &g);
    assert_eq!(
        c.settle_result("s", 0, "j", MAX_STORE_BYTES),
        Err(crate::Error::Limit)
    );
    s.discard(&mut c, "s", 0, "j").unwrap();
    assert_eq!(c.settle_result("s", 0, "j", 1), Err(crate::Error::Limit));
    assert!(c.get("s", 0, "j").is_ok());
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
#[test]
fn empty_selection_rejected_and_ineligible_snippets_excluded() {
    let i = Imports::new();
    let a = Authentic {
        present: AtomicBool::new(false),
    };
    assert!(matches!(
        capture(input(&[]), &i, &a, &|| false),
        Err(Error::EmptySelection)
    ));
    assert!(matches!(
        capture(input(&["unknown"]), &i, &a, &|| false),
        Err(Error::EmptySelection)
    ));
    assert_eq!(i.reads.load(Ordering::SeqCst), 0);
    let f = capture(input(&["unknown", "a"]), &i, &a, &|| false).unwrap();
    assert_eq!(f.items.len(), 1);
}
#[test]
fn duplicate_source_rejected_within_and_across_import_snapshots() {
    let a = Authentic {
        present: AtomicBool::new(false),
    };
    for second_import in ["import", "another-import"] {
        let i = Imports::new();
        let mut request = input(&["a", "a"]);
        request.selections[1].import_id = second_import.into();
        assert!(matches!(
            capture(request, &i, &a, &|| false),
            Err(Error::InvalidInput)
        ));
        assert_eq!(i.reads.load(Ordering::SeqCst), 1);
    }
}
#[test]
fn changed_source_refused_wrong_import_date_refused_stale_config_refused() {
    let a = Authentic {
        present: AtomicBool::new(false),
    };
    for i in [
        Imports {
            changed: true,
            ..Imports::new()
        },
        Imports {
            wrong_date: true,
            ..Imports::new()
        },
        Imports {
            stale: true,
            ..Imports::new()
        },
    ] {
        assert!(matches!(
            capture(input(&["a"]), &i, &a, &|| false),
            Err(Error::ChangedSource)
        ));
    }
}
#[test]
fn packet_order_and_labels_context_is_frozen_absent_baseline_allowed() {
    let i = Imports::new();
    let a = Authentic {
        present: AtomicBool::new(false),
    };
    let f = capture(input(&["b", "a"]), &i, &a, &|| false).unwrap();
    assert!(f.baseline.is_none());
    assert_eq!(f.locale.as_str(), "ko-KR");
    assert_eq!(f.date, date());
    assert_eq!(f.count, 3);
    let p = f.packet(0).unwrap();
    assert_eq!(p.snippets()[0].body, "body a");
    assert_eq!(p.snippets()[1].id, "snippet-2");
    assert_eq!(p.snippets()[0].offset, "+09:00");
    assert_eq!(p.snippets()[0].instant_utc, "1970-01-01T00:00:00.000Z");
    assert_eq!(p.snippets()[0].civil_date, date().to_string());
    assert!(!format!("{p:?}").contains("PRIVATE"));
    a.present.store(true, Ordering::SeqCst);
    assert_eq!(f.packet(0).unwrap(), p);
}

#[test]
fn shared_transient_cap_charges_generation_and_preserves_metadata() {
    let mut c = Coordinator::new(
        "i".into(),
        Limits {
            retained_bytes: MAX_STORE_BYTES - 1,
            ..Limits::default()
        },
    );
    c.open("s", 0).unwrap();
    let mut store = Store::default();
    let a = Admission {
        session: "s",
        now: 0,
        request_id: "j",
        digest: DIGEST,
        job_id: "j",
    };
    assert!(matches!(
        store.admit(&mut c, a, frozen(1)),
        Err(Error::Coordination(crate::Error::Limit))
    ));
    assert!(store.get("s", "j").is_none());
    assert!(c.lookup_request("s", 0, "j", DIGEST).unwrap().is_none());
    let mut c = coordinator();
    let g = admit(&mut store, &mut c, "j", frozen(1));
    assert_eq!(
        c.settle_result("s", 0, "j", 0),
        Err(crate::Error::InvalidTransition)
    );
    execute(&g, &Fake::new(vec![candidate()]), &|| 0, &mut || {});
    terminal(&store, &mut c, &g);
    store.discard(&mut c, "s", 0, "j").unwrap();
    // Admission metadata stays charged even after content is discarded.
    assert_eq!(
        c.admit("s", 0, "x", DIGEST, "x", crate::MAX_TRANSIENT_BYTES),
        Err(crate::Error::Limit)
    );
}

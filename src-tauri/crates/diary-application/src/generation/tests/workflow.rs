//! Profile lookup and the start workflow.
use super::*;

struct WorkflowProfiles {
    ready: bool,
    kind: AgentKind,
}
impl ProfileDirectory for WorkflowProfiles {
    fn list(&self) -> Vec<ProfileView> {
        self.check("p").into_iter().collect()
    }
    fn check(&self, alias: &str) -> Option<ProfileView> {
        Some(ProfileView {
            alias: alias.into(),
            kind: self.kind,
            status: Some(if self.ready {
                ProfileStatus::Ready
            } else {
                ProfileStatus::Unavailable
            }),
            reason: None,
            chat: Capability::Ready,
            generate: Capability::Ready,
            illustrated: Capability::Ready,
        })
    }
}
fn workflow_request() -> StartRequest {
    StartRequest {
        request_id: "request".into(),
        date: date(),
        selections: input(&["a"]).selections,
        profile: "p".into(),
        locale: Locale::parse("ko-KR").unwrap(),
        count: 1,
    }
}
#[test]
fn workflow_replays_before_fresh_checks_and_does_not_execute_simultaneous_winner_twice() {
    let mut c = coordinator();
    let mut store = Store::default();
    let request = workflow_request();
    let digest = request.fingerprint();
    let reviews = crate::review::Reviews::default();
    assert!(
        request
            .preflight(&mut c, &store, &reviews, "s", 0)
            .unwrap()
            .is_none()
    );
    let first = store
        .start(
            &mut c,
            Admission {
                session: "s",
                now: 0,
                request_id: "request",
                digest: &digest,
                job_id: "winner",
            },
            frozen(1),
        )
        .unwrap();
    assert!(first.execute);
    assert_eq!(first.job.state, JobState::Running);
    assert_eq!(
        request
            .preflight(&mut c, &store, &reviews, "s", 0)
            .unwrap()
            .unwrap()
            .id,
        "winner"
    );
    let second = store
        .start(
            &mut c,
            Admission {
                session: "s",
                now: 0,
                request_id: "request",
                digest: &digest,
                job_id: "loser",
            },
            frozen(1),
        )
        .unwrap();
    assert!(!second.execute);
    assert!(Arc::ptr_eq(&first.generation, &second.generation));
    let mut changed = workflow_request();
    changed.count = 2;
    assert!(matches!(
        changed.preflight(&mut c, &store, &reviews, "s", 0),
        Err(Error::Coordination(crate::Error::RequestMismatch))
    ));
    changed.request_id = "other".into();
    assert!(matches!(
        changed.preflight(&mut c, &store, &reviews, "s", 0),
        Err(Error::Busy)
    ));
}
#[test]
fn workflow_capture_rejects_reload_during_profile_lookup() {
    struct ReloadingProfiles(std::sync::atomic::AtomicU64);
    impl ProfileDirectory for ReloadingProfiles {
        fn list(&self) -> Vec<ProfileView> {
            vec![]
        }
        fn check(&self, alias: &str) -> Option<ProfileView> {
            let old = WorkflowProfiles {
                ready: true,
                kind: AgentKind::Codex,
            }
            .check(alias);
            self.0.store(1, Ordering::SeqCst);
            old
        }
    }
    let profiles = ReloadingProfiles(std::sync::atomic::AtomicU64::new(0));
    let runner = Fake::new(vec![]);
    assert!(matches!(
        workflow_request().capture(CaptureServices {
            profiles: &profiles,
            runner: &runner,
            imports: &Imports::new(),
            authentic: &Authentic {
                present: AtomicBool::new(false)
            },
            configuration: &|| profiles.0.load(Ordering::SeqCst),
            cancelled: &|| false,
        }),
        Err(Error::ChangedSource)
    ));
    assert!(runner.calls.borrow().is_empty());
}
#[test]
fn workflow_capture_checks_profiles_configuration_and_cancellation_without_inference() {
    let imports = Imports::new();
    let authentic = Authentic {
        present: AtomicBool::new(false),
    };
    let runner = Fake::new(vec![]);
    let profiles = WorkflowProfiles {
        ready: true,
        kind: AgentKind::Codex,
    };
    let configuration_reads = std::cell::Cell::new(0);
    let configuration = || {
        let n = configuration_reads.get();
        configuration_reads.set(n + 1);
        if n == 0 { 1 } else { 2 }
    };
    assert!(matches!(
        workflow_request().capture(CaptureServices {
            profiles: &profiles,
            runner: &runner,
            imports: &imports,
            authentic: &authentic,
            configuration: &configuration,
            cancelled: &|| false
        }),
        Err(Error::ChangedSource)
    ));
    let blocked = WorkflowProfiles {
        ready: false,
        kind: AgentKind::Codex,
    };
    let reads = imports.reads.load(Ordering::SeqCst);
    assert!(matches!(
        workflow_request().capture(CaptureServices {
            profiles: &blocked,
            runner: &runner,
            imports: &imports,
            authentic: &authentic,
            configuration: &|| 1,
            cancelled: &|| false
        }),
        Err(Error::ProfileNotReady)
    ));
    assert_eq!(imports.reads.load(Ordering::SeqCst), reads);
    assert!(matches!(
        workflow_request().capture(CaptureServices {
            profiles: &profiles,
            runner: &runner,
            imports: &imports,
            authentic: &authentic,
            configuration: &|| 1,
            cancelled: &|| true
        }),
        Err(Error::Unavailable)
    ));
    assert!(runner.calls.borrow().is_empty());
}
#[test]
fn workflow_uses_same_capture_contract_with_an_alternative_provider() {
    let imports = Imports::new();
    let authentic = Authentic {
        present: AtomicBool::new(false),
    };
    let runner = Fake::new(vec![candidate()]);
    let profiles = WorkflowProfiles {
        ready: true,
        kind: AgentKind::Claude,
    };
    let context = workflow_request()
        .capture(CaptureServices {
            profiles: &profiles,
            runner: &runner,
            imports: &imports,
            authentic: &authentic,
            configuration: &|| 1,
            cancelled: &|| false,
        })
        .unwrap();
    assert_eq!(context.profile().kind, AgentKind::Claude);
    assert_eq!(context.prompt().selection().provider, AgentKind::Claude);
    let mut c = coordinator();
    let mut store = Store::default();
    let digest = workflow_request().fingerprint();
    let started = store
        .start(
            &mut c,
            Admission {
                session: "s",
                now: 0,
                request_id: "request",
                digest: &digest,
                job_id: "j",
            },
            context,
        )
        .unwrap();
    execute(&started.generation, &runner, &|| 0, &mut || {});
    assert_eq!(
        store
            .finish(&mut c, "s", 0, "j", &started.generation)
            .unwrap()
            .state,
        JobState::Succeeded
    );
    assert_eq!(runner.calls.borrow().len(), 1);
    assert_eq!(runner.calls.borrow()[0].profile().kind, AgentKind::Claude);
}
#[test]
fn workflow_close_and_cancel_remain_authoritative_after_capture() {
    let mut c = coordinator();
    let mut store = Store::default();
    let digest = workflow_request().fingerprint();
    c.begin_close("s", 0).unwrap();
    assert!(matches!(
        workflow_request().preflight(&mut c, &store, &crate::review::Reviews::default(), "s", 0),
        Err(Error::Busy)
    ));
    assert!(matches!(
        store.start(
            &mut c,
            Admission {
                session: "s",
                now: 0,
                request_id: "request",
                digest: &digest,
                job_id: "j"
            },
            frozen(1)
        ),
        Err(Error::Coordination(crate::Error::Busy))
    ));
    let mut c = coordinator();
    let started = store
        .start(
            &mut c,
            Admission {
                session: "s",
                now: 0,
                request_id: "request",
                digest: &digest,
                job_id: "j",
            },
            frozen(1),
        )
        .unwrap();
    assert!(store.cancel_job("s", "j"));
    let runner = Fake::new(vec![]);
    execute(&started.generation, &runner, &|| 0, &mut || {});
    assert_eq!(
        store
            .finish(&mut c, "s", 0, "j", &started.generation)
            .unwrap()
            .state,
        JobState::Cancelled
    );
    assert!(runner.calls.borrow().is_empty());
}

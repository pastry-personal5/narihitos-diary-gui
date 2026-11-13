use super::*;
use crate::{
    config::{NotionKind, NotionPage},
    export::notion::{
        client::Client,
        fake::{FakeNotion, Fault},
    },
};
use diary_application::illustrated::Store as _;
use diary_domain::illustrated::{Entry, Photo};
const ROOT: &str = "0123456789abcdef0123456789abcdef";
const PNG: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../src/assets/toolbar/illustrated-create.png"
));

fn publish(
    env: &Env,
    store: &crate::illustrated::Store,
    version: u8,
    date: &str,
    text: &str,
) -> Entry {
    let entry = Entry {
        id: op(version).as_str().into(),
        date: d(date),
        created_at: "2026-11-13T00:00:00Z".into(),
        source_id: op(99).as_str().into(),
        source_revision: 1,
        source_sha256: "a".repeat(64),
        agent_profile: "unused".into(),
        image_engine: "unused".into(),
        trigger: "private-trigger".into(),
        directives: vec!["private-directive".into()],
        theme_count: 1,
        photos: vec![Photo {
            seed: 99,
            prompt: "private-trigger private-prompt".into(),
        }],
        body: format!("{text}\n\n![한😀 photo]({date}/01.png)\n\nAfter photo.\n"),
    };
    let file = env._temp.path().join("fixture.png");
    std::fs::write(&file, PNG).unwrap();
    store.publish(&entry, &[file], &Default::default()).unwrap();
    entry
}
fn setup(
    env: &Env,
) -> (
    Arc<crate::illustrated::Store>,
    Arc<FakeNotion>,
    ExportCoordinator,
) {
    let store = Arc::new(
        crate::illustrated::Store::open(env._temp.path().canonicalize().unwrap().join("ws"))
            .unwrap(),
    );
    let fake = Arc::new(FakeNotion::new(ROOT).with_page_size(1));
    let adapters = env.ws.notion_adapters(
        Arc::new(Client::new(fake.clone(), Arc::new(|_| {}), Duration::ZERO)),
        &[NotionPage {
            kind: NotionKind::Illustrated,
            root_page_id: ROOT.into(),
        }],
    );
    let svc = env
        .ws
        .exports_with(env.store.clone(), adapters)
        .unwrap()
        .with_illustrated(store.clone());
    svc.recover().unwrap();
    (store, fake, svc)
}
fn start(svc: &ExportCoordinator, operation: u8, entry: &Entry) -> Result<Started, ExportError> {
    svc.start_illustrated(
        "s",
        1,
        IllustratedStartRequest {
            operation_id: op(operation),
            date: entry.date,
            expected_version: entry.id.clone(),
        },
    )
}
fn wait(svc: &ExportCoordinator, operation: u8, expected: ExportState) {
    for _ in 0..500 {
        let r = svc.operation(&op(operation)).unwrap().unwrap();
        if r.state == expected {
            return;
        }
        if r.state.is_terminal() || r.state == ExportState::Uncertain {
            panic!("unexpected result: {r:?}");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("operation did not settle");
}
fn page(fake: &FakeNotion, date: &str) -> String {
    let y = fake
        .children(ROOT)
        .into_iter()
        .find(|p| fake.title_of(p).as_deref() == Some(&date[..4]))
        .unwrap();
    let m = fake
        .children(&y)
        .into_iter()
        .find(|p| fake.title_of(p).as_deref() == Some(&date[..7]))
        .unwrap();
    fake.children(&m)
        .into_iter()
        .find(|p| fake.title_of(p).is_some_and(|t| t.starts_with(date)))
        .unwrap()
}

#[test]
fn published_illustrated_upload_noop_replace_and_same_minute_image_drift() {
    let env = env();
    let (store, fake, svc) = setup(&env);
    assert!(svc.available_kind(ContentKind::Illustrated, Target::Notion));
    assert!(!svc.available(Target::Notion));
    assert!(!svc.available_kind(ContentKind::Illustrated, Target::Icloud));
    let entry = publish(&env, &store, 1, "2026-11-13", "First text.");
    start(&svc, 20, &entry).unwrap();
    wait(&svc, 20, ExportState::Succeeded);
    let day = page(&fake, "2026-11-13");
    let ids = fake.children(&day);
    assert_eq!(
        ids.iter().map(|id| fake.kind_of(id)).collect::<Vec<_>>(),
        ["paragraph", "image", "paragraph"]
    );
    assert_eq!(fake.text_of(&ids[0]), "First text.");
    assert_eq!(fake.text_of(&ids[2]), "After photo.");
    assert_eq!(store.photo(entry.date, &entry.id, 1).unwrap(), PNG);
    let writes = fake.count("create_page")
        + fake.count("append")
        + fake.count("create_upload")
        + fake.count("send_upload");
    start(&svc, 21, &entry).unwrap();
    wait(&svc, 21, ExportState::Succeeded);
    assert_eq!(
        writes,
        fake.count("create_page")
            + fake.count("append")
            + fake.count("create_upload")
            + fake.count("send_upload")
    );
    let changed = publish(&env, &store, 2, "2026-11-13", "Changed text.");
    start(&svc, 22, &changed).unwrap();
    wait(&svc, 22, ExportState::Succeeded);
    assert_eq!(page(&fake, "2026-11-13"), day);
    assert_eq!(fake.children(&day).len(), 3);
    let image = fake.children(&day)[1].clone();
    fake.replace_image_bytes(&image, b"externally replaced bytes, same ID and timestamp");
    assert!(matches!(
        start(&svc, 23, &changed).unwrap(),
        Started::Preview(Preview {
            reason: Reason::Changed,
            ..
        })
    ));
    assert_eq!(fake.count("delete"), 3);
    assert!(
        store
            .read(entry.date)
            .unwrap()
            .unwrap()
            .render()
            .unwrap()
            .windows(b"private-prompt".len())
            .any(|w| w == b"private-prompt")
    );
    assert!(
        fake.children(&day)
            .iter()
            .all(|id| !fake.text_of(id).contains("private-"))
    );
}

#[test]
fn backdated_pages_are_sorted_at_all_three_levels_without_recreating_pages() {
    let env = env();
    let (store, fake, svc) = setup(&env);
    for (i, date) in [
        "2026-05-15",
        "2025-03-01",
        "2027-01-01",
        "2026-01-01",
        "2026-03-01",
        "2026-12-01",
        "2026-05-01",
        "2026-05-10",
        "2026-05-30",
    ]
    .into_iter()
    .enumerate()
    {
        let entry = publish(&env, &store, i as u8 + 1, date, "Fixture.");
        start(&svc, i as u8 + 30, &entry).unwrap();
        wait(&svc, i as u8 + 30, ExportState::Succeeded);
    }
    fn sorted(fake: &FakeNotion, parent: &str, depth: usize) {
        let children = fake.children(parent);
        let titles: Vec<_> = children.iter().filter_map(|id| fake.title_of(id)).collect();
        assert!(titles.windows(2).all(|w| w[0] < w[1]));
        if depth < 2 {
            for child in children {
                sorted(fake, &child, depth + 1);
            }
        }
    }
    sorted(&fake, ROOT, 0);
    assert_eq!(fake.count("delete"), 0);
}

#[test]
fn ambiguous_append_retains_snapshot_and_recovers_without_replaying_or_adopting_content() {
    let env = env();
    let (store, fake, svc) = setup(&env);
    let entry = publish(&env, &store, 1, "2026-11-13", "Fixture.");
    fake.fault("append", Fault::LoseReply);
    start(&svc, 20, &entry).unwrap();
    wait(&svc, 20, ExportState::Uncertain);
    let calls = fake.calls();
    let adapters = env.ws.notion_adapters(
        Arc::new(Client::new(fake.clone(), Arc::new(|_| {}), Duration::ZERO)),
        &[NotionPage {
            kind: NotionKind::Illustrated,
            root_page_id: ROOT.into(),
        }],
    );
    let recovered = env
        .ws
        .exports_with(env.store.clone(), adapters)
        .unwrap()
        .with_illustrated(store);
    recovered.recover().unwrap();
    assert_eq!(
        recovered.operation(&op(20)).unwrap().unwrap().state,
        ExportState::Uncertain
    );
    assert_eq!(
        fake.count("append"),
        calls.iter().filter(|s| *s == "append").count()
    );
    assert_eq!(fake.count("create_upload"), 0);
}

#[test]
fn a_stale_version_is_rejected_before_any_remote_write() {
    let env = env();
    let (store, fake, svc) = setup(&env);
    let old = publish(&env, &store, 1, "2026-11-13", "Old.");
    publish(&env, &store, 2, "2026-11-13", "New.");
    assert!(matches!(
        start(&svc, 20, &old),
        Err(ExportError::StaleRevision)
    ));
    assert_eq!(fake.count("create_page"), 0);
}

#[test]
fn captured_photos_survive_source_deletion_and_restart_until_confirmed_or_terminal() {
    let env = env();
    let (store, fake, svc) = setup(&env);
    let entry = publish(&env, &store, 1, "2026-11-13", "Captured version.");
    *svc.0.fault.lock().unwrap() = Some("export:after_admission:io".into());
    start(&svc, 20, &entry).unwrap();
    store.delete(entry.date, &entry.id, true).unwrap();
    assert_eq!(svc.0.db.export_photos(&op(20)).unwrap().len(), 1);
    let adapters = env.ws.notion_adapters(
        Arc::new(Client::new(fake.clone(), Arc::new(|_| {}), Duration::ZERO)),
        &[NotionPage {
            kind: NotionKind::Illustrated,
            root_page_id: ROOT.into(),
        }],
    );
    let recovered = env
        .ws
        .exports_with(env.store.clone(), adapters)
        .unwrap()
        .with_illustrated(store);
    recovered.recover().unwrap();
    let Started::Preview(preview) = start(&recovered, 20, &entry).unwrap() else {
        panic!("fresh confirmation required");
    };
    recovered
        .confirm(
            "s",
            1,
            ConfirmRequest {
                operation_id: op(20),
                step_id: step(80),
                expected_checkpoint: preview.expected_checkpoint,
                token: preview.token,
            },
        )
        .unwrap();
    wait(&recovered, 20, ExportState::Succeeded);
    let day = page(&fake, "2026-11-13");
    assert_eq!(fake.text_of(&fake.children(&day)[0]), "Captured version.");
    assert!(recovered.0.db.export_photos(&op(20)).unwrap().is_empty());
}

#[test]
fn failure_at_each_mutation_keeps_honest_outcomes_without_replay() {
    for kind in ["create_page", "create_upload", "append", "delete"] {
        let env = env();
        let (store, fake, svc) = setup(&env);
        let first = publish(&env, &store, 1, "2026-11-13", "Original.");
        start(&svc, 20, &first).unwrap();
        wait(&svc, 20, ExportState::Succeeded);
        let changed = publish(&env, &store, 2, "2026-11-13", "Replacement.");
        // A create-page failure is exercised on another absent date; all other failures
        // exercise replacement of a known owned page, preserving old body until verified.
        let input = if kind == "create_page" {
            publish(&env, &store, 3, "2027-01-01", "Another day.")
        } else {
            changed
        };
        fake.fault(kind, Fault::LoseReply);
        start(&svc, 21, &input).unwrap();
        wait(&svc, 21, ExportState::Uncertain);
        let writes = fake.count(kind);
        let _ = svc.abandon("s", 1, &op(21)).unwrap();
        assert_eq!(
            fake.count(kind),
            writes,
            "read-only recovery must not replay {kind}"
        );
        assert!(!svc.0.db.export_photos(&op(21)).unwrap().is_empty());
    }
}

#[test]
fn lost_upload_send_is_reconciled_by_exact_upload_id_without_resending() {
    let env = env();
    let (store, fake, svc) = setup(&env);
    let entry = publish(&env, &store, 1, "2026-11-13", "Fixture.");
    fake.fault("send_upload", Fault::LoseReply);
    start(&svc, 20, &entry).unwrap();
    wait(&svc, 20, ExportState::Succeeded);
    assert_eq!(fake.count("send_upload"), 1);
    assert_eq!(fake.count("create_upload"), 1);
}

#[test]
fn duplicate_or_foreign_hierarchy_refuses_before_destructive_cleanup() {
    for foreign in [false, true] {
        let env = env();
        let (store, fake, svc) = setup(&env);
        let entry = publish(&env, &store, 1, "2026-11-13", "Fixture.");
        start(&svc, 20, &entry).unwrap();
        wait(&svc, 20, ExportState::Succeeded);
        if foreign {
            fake.external_page(&page(&fake, "2026-11-13"), "Foreign child page");
        } else {
            fake.external_page(ROOT, "2026");
        }
        assert!(matches!(
            start(&svc, 21, &entry),
            Err(ExportError::CloudUnavailable(Unavailable::Structural))
        ));
        assert_eq!(fake.count("delete"), 0);
    }
}

struct CaptureStore {
    entry: Entry,
    reads: std::sync::atomic::AtomicUsize,
    stale_photo: bool,
    stale_end: bool,
}
impl diary_application::illustrated::Store for CaptureStore {
    fn list(&self) -> Result<Vec<DiaryDate>, diary_application::illustrated::Error> {
        Ok(vec![self.entry.date])
    }
    fn read(&self, _: DiaryDate) -> Result<Option<Entry>, diary_application::illustrated::Error> {
        let reads = self.reads.fetch_add(1, Ordering::SeqCst);
        Ok(if self.stale_end && reads > 0 {
            None
        } else {
            Some(self.entry.clone())
        })
    }
    fn photo(
        &self,
        _: DiaryDate,
        version: &str,
        n: usize,
    ) -> Result<Vec<u8>, diary_application::illustrated::Error> {
        assert_eq!(version, self.entry.id);
        if self.stale_photo && n > 1 {
            return Err(diary_application::illustrated::Error::Stale);
        }
        Ok(PNG.to_vec())
    }
    fn publish(
        &self,
        _: &Entry,
        _: &[PathBuf],
        _: &diary_application::agent::CancelToken,
    ) -> Result<(), diary_application::illustrated::Error> {
        unreachable!()
    }
    fn delete(
        &self,
        _: DiaryDate,
        _: &str,
        _: bool,
    ) -> Result<(), diary_application::illustrated::Error> {
        unreachable!()
    }
}

#[test]
fn capture_never_admits_mixed_versions_and_prepares_the_full_99_photo_limit() {
    for (count, stale_photo, stale_end) in [(2, true, false), (2, false, true), (99, false, false)]
    {
        let env = env();
        let (store, fake, _) = setup(&env);
        let mut entry = publish(&env, &store, 1, "2026-11-13", "Fixture.");
        entry.theme_count = count;
        entry.photos = vec![entry.photos[0].clone(); count];
        entry.body = (1..=count)
            .map(|n| format!("Paragraph {n}.\n\n![Photo {n}](2026-11-13/{n:02}.png)\n\n"))
            .collect();
        let capture = Arc::new(CaptureStore {
            entry: entry.clone(),
            reads: Default::default(),
            stale_photo,
            stale_end,
        });
        let adapters = env.ws.notion_adapters(
            Arc::new(Client::new(fake.clone(), Arc::new(|_| {}), Duration::ZERO)),
            &[NotionPage {
                kind: NotionKind::Illustrated,
                root_page_id: ROOT.into(),
            }],
        );
        let service = env
            .ws
            .exports_with(env.store.clone(), adapters)
            .unwrap()
            .with_illustrated(capture);
        let snapshot = service.0.illustrated_snapshot(entry.date, &entry.id);
        if stale_photo || stale_end {
            assert!(matches!(snapshot, Err(ExportError::StaleRevision)));
        } else {
            let snapshot = snapshot.unwrap();
            assert_eq!(snapshot.photos.len(), 99);
            assert!(snapshot.photos.iter().all(|p| p.len() <= 5 * 1024 * 1024));
        }
        assert_eq!(fake.count("create_page"), 0);
        assert!(service.0.db.export_photos(&op(20)).unwrap().is_empty());
    }
}

#[test]
fn cancellation_after_upload_and_expiry_never_delete_the_old_body_or_claim_rollback() {
    for expired in [false, true] {
        let env = env();
        let (store, fake, svc) = setup(&env);
        let entry = publish(&env, &store, 1, "2026-11-13", "Original.");
        start(&svc, 20, &entry).unwrap();
        wait(&svc, 20, ExportState::Succeeded);
        let day = page(&fake, "2026-11-13");
        let old_ids = fake.children(&day);
        let changed = publish(&env, &store, 2, "2026-11-13", "Changed.");
        let snapshot = svc
            .0
            .illustrated_snapshot(changed.date, &changed.id)
            .unwrap();
        let adapter = svc
            .0
            .adapter(ContentKind::Illustrated, Target::Notion)
            .unwrap();
        let expected = adapter.observe(&changed.date);
        let sent_before = fake.count("send_upload");
        if expired {
            fake.fault("send_upload", Fault::ExpiredUpload);
        }
        let applied = adapter.apply_snapshot(&snapshot, Write::Replace, &expected, &|| {
            !expired && fake.count("send_upload") > sent_before
        });
        assert_eq!(applied, Applied::Unknown);
        assert_eq!(&fake.children(&day)[..old_ids.len()], old_ids);
        assert_eq!(fake.count("delete"), 0);
        assert_eq!(fake.count("send_upload"), sent_before + 1);
    }
}

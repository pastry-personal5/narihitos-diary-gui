use super::*;
use crate::{
    source::tests::{AYUMI, Tree, ayumi},
    sqlite::{DB_FILE, Db},
};
use diary_logging::private_fs::PrivateDir;
use std::{collections::BTreeSet, fs, os::unix::fs::PermissionsExt};

fn setup(tree: &Tree) -> (tempfile::TempDir, Arc<Db>, Store) {
    let temp = tempfile::tempdir().unwrap();
    fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let dir = PrivateDir::open(&temp.path().canonicalize().unwrap()).unwrap();
    let db =
        Arc::new(Db::create(&dir, &temp.path().canonicalize().unwrap().join(DB_FILE)).unwrap());
    let sources = vec![Source {
        alias: "test".into(),
        kind: crate::config::SourceKind::Ayumi,
        root: tree.temp.path().into(),
    }];
    let mut store = Store::new(sources, db.clone());
    store.capture = Arc::new(|| {
        SystemZone::from_database(std::path::Path::new("/var/db/timezone/zoneinfo"), "UTC")
    });
    (temp, db, store)
}
fn date() -> DiaryDate {
    DiaryDate::parse("2024-02-29").unwrap()
}

#[test]
fn import_resolution_stale_bytes_and_restart_without_source_writes() {
    let tree = Tree::new();
    let conflict = String::from_utf8(ayumi())
        .unwrap()
        .replace("09:00:00.123+09:00", "09:00:01.123+09:00")
        .into_bytes();
    tree.write(AYUMI, &conflict);
    let (temp, db, store) = setup(&tree);
    let report = store.import_date("import-a", date(), 7, &|| false).unwrap();
    assert!(report.included.is_empty());
    assert_eq!(report.warnings.len(), 1);
    let s = &report.warnings[0];
    assert_eq!(s.token.import_id.as_deref(), Some("import-a"));
    store
        .resolve(
            "import-a",
            &s.id,
            &s.token.sha256,
            Authority::Filename,
            7,
            &|| false,
        )
        .unwrap();
    let report = store.list("import-a", &|| false).unwrap();
    assert!(report.included.is_empty());
    assert_eq!(report.warnings.len(), 1);
    assert_eq!(
        store
            .import_date("refresh", date(), 7, &|| false)
            .unwrap()
            .included
            .len(),
        1
    );
    assert_eq!(fs::read(tree.temp.path().join(AYUMI)).unwrap(), conflict);
    let id = s.id.clone();
    let digest = s.token.sha256;
    drop(store);
    drop(db);
    let dir = PrivateDir::open(&temp.path().canonicalize().unwrap()).unwrap();
    let db = Arc::new(
        Db::open_existing(
            &dir,
            &temp.path().canonicalize().unwrap().join(DB_FILE),
            false,
        )
        .unwrap(),
    );
    assert!(db.resolution(&id, &digest).unwrap().is_some());
    assert!(db.resolution(&id, &[9; 32]).unwrap().is_none());
    let store = Store::new(
        vec![Source {
            alias: "new alias".into(),
            kind: crate::config::SourceKind::Ayumi,
            root: tree.temp.path().into(),
        }],
        db,
    );
    assert_eq!(
        store.list("import-a", &|| false).unwrap().warnings[0].id,
        id
    );
    let mut changed = conflict.clone();
    changed.push(b' ');
    tree.write(AYUMI, &changed);
    assert_eq!(
        store.resolve("import-a", &id, &digest, Authority::Header, 7, &|| false),
        Err(ImportError::Stale)
    );
    assert_eq!(store.list("import-a", &|| false).unwrap().warnings.len(), 1);
}

#[test]
fn mixed_scan_order_misplaced_warnings_and_outside_directories() {
    let tree = Tree::new();
    tree.write(AYUMI, &ayumi());
    let text = String::from_utf8(ayumi()).unwrap();
    let conflict = text
        .replace("09:00:00.123+09:00", "09:00:01.123+09:00")
        .replace("00-00-00-123", "00-00-00-124");
    tree.write("2024/02/29/2024-02-29_00-00-00-124.md", conflict.as_bytes());
    let undated = text
        .replace("2024-02-29_00-00-00-123", "2024-02-30_00-00-00-123")
        .replace("2024-02-29T09:00:00.123+09:00", "bad");
    tree.write("2024/02/29/2024-02-30_00-00-00-123.md", undated.as_bytes());
    tree.write("2024/02/28/2024-02-29_00-00-00-123.md", &ayumi());
    tree.write("2020/01/01/invalid.md", b"bad");
    let (_temp, _db, store) = setup(&tree);
    let r = store.import_date("mixed", date(), 1, &|| false).unwrap();
    assert_eq!(r.included.len(), 1);
    assert_eq!(r.warnings.len(), 3);
    assert_eq!(r.misplaced.len(), 1);
    assert_eq!(r.scan.failures.len(), 1);
    let s = r
        .warnings
        .iter()
        .find(|s| r.misplaced.contains(&s.id))
        .unwrap();
    assert_eq!(
        store.resolve(
            "mixed",
            &s.id,
            &s.token.sha256,
            Authority::Filename,
            1,
            &|| false
        ),
        Err(ImportError::FilenameDirectoryMismatch)
    );
    store
        .resolve(
            "mixed",
            &s.id,
            &s.token.sha256,
            Authority::Header,
            1,
            &|| false,
        )
        .unwrap();
    assert_eq!(store.list("mixed", &|| false).unwrap().included.len(), 1);
    let r = store
        .import_date("mixed-refresh", date(), 1, &|| false)
        .unwrap();
    assert_eq!(r.included.len(), 2);
    assert_eq!(
        r.included[0].source.rel,
        "2024/02/28/2024-02-29_00-00-00-123.md"
    );
    assert_eq!(r.included[1].source.rel, AYUMI);
}

#[test]
fn zone_changes_leave_provenance_and_cancelled_rows_incomplete() {
    let tree = Tree::new();
    tree.write(AYUMI, &ayumi());
    let (_temp, db, mut store) = setup(&tree);
    let first = store
        .import_date("first", date(), 1, &|| false)
        .unwrap()
        .context;
    store.capture = Arc::new(|| {
        SystemZone::from_database(
            std::path::Path::new("/var/db/timezone/zoneinfo"),
            "Asia/Seoul",
        )
    });
    let second = store
        .import_date("second", date(), 1, &|| false)
        .unwrap()
        .context;
    assert_ne!(first.zone.name, second.zone.name);
    assert_eq!(db.context("first").unwrap(), first);
    assert_eq!(db.context("second").unwrap(), second);
    let calls = std::cell::Cell::new(0);
    let cancelled = || {
        calls.set(calls.get() + 1);
        calls.get() > 1
    };
    assert_eq!(
        store.import_date("cancelled", date(), 1, &cancelled).err(),
        Some(ImportError::Cancelled)
    );
    assert_eq!(db.context("cancelled"), Err(ImportError::NotFound));
    assert!(
        db.complete(&store.list("first", &|| false).unwrap(), 0)
            .is_err()
    );
    assert!(
        db.with(|c| c
            .execute(
                "UPDATE import_contexts SET zone='UTC' WHERE import_id='second'",
                []
            )
            .map(|_| ())
            .map_err(|_| crate::sqlite::DbError::Storage))
            .is_err()
    );
    assert!(
        db.save_resolution(
            "missing-source",
            &[0; 32],
            &Resolution {
                authority: Authority::Header,
                instant: diary_domain::source::stamp::Instant {
                    unix_ms: 0,
                    precision: diary_domain::source::stamp::Precision::Milli
                },
                decided_at: 0
            }
        )
        .is_err()
    );
}

#[test]
fn capture_errors_store_no_context_and_skipped_selected_date_stays_incomplete() {
    let tree = Tree::new();
    let (_temp, db, mut store) = setup(&tree);
    store.capture = Arc::new(|| Err(ZoneError::Unnamed));
    assert_eq!(
        store.import_date("unnamed", date(), 1, &|| false).err(),
        Some(ImportError::UnnamedZone)
    );
    assert_eq!(
        db.with(|c| c
            .query_row("SELECT count(*) FROM import_contexts", [], |r| r
                .get::<_, i64>(0))
            .map_err(|_| crate::sqlite::DbError::Storage))
            .unwrap(),
        0
    );
    store.capture = Arc::new(|| {
        SystemZone::from_database(
            std::path::Path::new("/var/db/timezone/zoneinfo"),
            "Pacific/Apia",
        )
    });
    assert_eq!(
        store
            .import_date(
                "skipped",
                DiaryDate::parse("2011-12-30").unwrap(),
                1,
                &|| false
            )
            .err(),
        Some(ImportError::InvalidDiaryDate)
    );
    assert_eq!(db.context("skipped"), Err(ImportError::NotFound));
}

#[test]
fn shared_scan_budgets_propagate_incomplete_and_keep_other_dates_unread() {
    let tree = Tree::new();
    for i in 0..3 {
        tree.write(&format!("2024/02/29/{i}.md"), b"malformed");
    }
    tree.write("2020/01/01/outside.md", b"malformed");
    let dirs = BTreeSet::from(["2024/02/29".into()]);
    let r = scan_selected(
        &tree.root,
        &crate::config::SourceKind::Ayumi,
        1,
        100,
        2,
        Some(&dirs),
        &|| false,
    );
    assert!(r.incomplete);
    assert_eq!(r.candidates_seen, 2);
    assert_eq!(r.failures.len(), 2);
    assert!(r.failures.iter().all(|(p, _)| p.starts_with("2024/02/29/")));
    let r = scan_selected(
        &tree.root,
        &crate::config::SourceKind::Ayumi,
        1,
        1,
        10,
        Some(&dirs),
        &|| false,
    );
    assert!(r.incomplete);
    assert_eq!(r.entries_seen, 1);
}

#[test]
fn ordering_uses_instant_then_canonical_root_and_path_across_sources() {
    let a = Tree::new();
    let b = Tree::new();
    for tree in [&a, &b] {
        tree.write(AYUMI, &ayumi());
    }
    let early_rel = "2024/02/28/2024-02-28_00-00-00-123.md";
    let late_rel = "2024/03/01/2024-03-01_00-00-00-123.md";
    a.write(
        early_rel,
        String::from_utf8(ayumi())
            .unwrap()
            .replace("2024-02-29", "2024-02-28")
            .as_bytes(),
    );
    b.write(
        late_rel,
        String::from_utf8(ayumi())
            .unwrap()
            .replace("2024-02-29", "2024-03-01")
            .as_bytes(),
    );
    let (_temp, _db, mut store) = setup(&a);
    store.sources.push(Source {
        alias: "second".into(),
        kind: crate::config::SourceKind::Ayumi,
        root: b.temp.path().into(),
    });
    // Configuration order cannot determine ties.
    store.sources.sort_by(|a, b| b.root.cmp(&a.root));
    let r = store.import_date("ordering", date(), 1, &|| false).unwrap();
    assert_eq!(r.included.len(), 4);
    let id = |tree: &Tree, rel: &str| {
        super::super::root::source_id(
            &tree.root,
            &diary_domain::source::path::normalize(rel, tree.root.insensitive).unwrap(),
        )
    };
    let tied = if a.root.canonical < b.root.canonical {
        [id(&a, AYUMI), id(&b, AYUMI)]
    } else {
        [id(&b, AYUMI), id(&a, AYUMI)]
    };
    assert_eq!(
        r.included
            .iter()
            .map(|s| s.source.id.clone())
            .collect::<Vec<_>>(),
        vec![
            id(&a, early_rel),
            tied[0].clone(),
            tied[1].clone(),
            id(&b, late_rel)
        ]
    );
    assert!(
        r.included
            .windows(2)
            .all(|pair| pair[0].instant.unix_ms <= pair[1].instant.unix_ms)
    );
}

#[test]
fn misplaced_header_inside_window_is_discovered_outside_directory_window() {
    let tree = Tree::new();
    tree.write("2020/01/01/2024-02-29_00-00-00-123.md", &ayumi());
    let (_temp, _db, store) = setup(&tree);
    let r = store.import_date("outside", date(), 1, &|| false).unwrap();
    assert_eq!(r.warnings.len(), 1);
    assert_eq!(r.misplaced, vec![r.warnings[0].id.clone()]);
    assert!(matches!(
        r.warnings[0].dating,
        Dating::HeaderOnly {
            filename: StampError::DirMismatch,
            ..
        }
    ));
    assert!(!r.scan.incomplete);
}

#[test]
fn snapshot_survives_tree_changes_restart_and_resolution_without_rescanning() {
    let tree = Tree::new();
    for i in 0..120 {
        let stem = format!("2024-02-29_00-00-00-{i:03}");
        let bytes = String::from_utf8(ayumi())
            .unwrap()
            .replace("2024-02-29_00-00-00-123", &stem)
            .replace("09:00:00.123+09:00", "09:00:01.123+09:00");
        tree.write(&format!("2024/02/29/{stem}.md"), bytes.as_bytes());
    }
    tree.write("2020/01/01/bad.md", b"bad");
    let (temp, db, store) = setup(&tree);
    let before = store.import_date("snapshot", date(), 7, &|| false).unwrap();
    assert_eq!(before.warnings.len(), 120);
    let chosen = &before.warnings[110];
    let polls = std::cell::Cell::new(0);
    store
        .resolve(
            "snapshot",
            &chosen.id,
            &chosen.token.sha256,
            Authority::Header,
            7,
            &|| {
                polls.set(polls.get() + 1);
                false
            },
        )
        .unwrap();
    // A whole-tree traversal would poll hundreds of times, including per entry.
    assert!(polls.get() < 10);
    tree.write(&before.warnings[0].rel, b"changed source");
    fs::remove_file(tree.temp.path().join(&before.warnings[1].rel)).unwrap();
    tree.write(AYUMI, &ayumi());
    let after = store.list("snapshot", &|| false).unwrap();
    assert_eq!(after.warnings, before.warnings);
    assert_eq!(after.scan.failures, before.scan.failures);
    assert_eq!(after.context, before.context);
    drop(store);
    drop(db);
    let root = temp.path().canonicalize().unwrap();
    let db = Arc::new(
        Db::open_existing(
            &PrivateDir::open(&root).unwrap(),
            &root.join(DB_FILE),
            false,
        )
        .unwrap(),
    );
    let mut store = Store::new(
        vec![Source {
            alias: "renamed".into(),
            kind: crate::config::SourceKind::Ayumi,
            root: tree.temp.path().into(),
        }],
        db,
    );
    store.capture = Arc::new(|| {
        SystemZone::from_database(std::path::Path::new("/var/db/timezone/zoneinfo"), "UTC")
    });
    let restarted = store.list("snapshot", &|| false).unwrap();
    assert_eq!(restarted.warnings, before.warnings);
    assert_eq!(restarted.scan.failures, before.scan.failures);
    assert_eq!(restarted.context, before.context);
    for changed in &before.warnings[..2] {
        assert_eq!(
            store.resolve(
                "snapshot",
                &changed.id,
                &changed.token.sha256,
                Authority::Header,
                7,
                &|| false
            ),
            Err(ImportError::Stale)
        );
    }
    let refreshed = store
        .import_date("snapshot-refresh", date(), 7, &|| false)
        .unwrap();
    assert_eq!(refreshed.included.len(), 2);
    assert_eq!(refreshed.warnings.len(), 117);
    // Missing source directories must not affect durable listing reads.
    let moved = tree.temp.path().join("2024");
    fs::rename(&moved, tree.temp.path().join("hidden")).unwrap();
    assert_eq!(
        store.list("snapshot", &|| false).unwrap().warnings,
        before.warnings
    );
    assert!(matches!(
        store.list("snapshot", &|| true),
        Err(ImportError::Cancelled)
    ));
}

#[test]
fn navigation_descriptors_round_trip_restart_and_read_only_warning_bodies() {
    let tree = Tree::new();
    let original = String::from_utf8(ayumi()).unwrap();
    tree.write(AYUMI, original.as_bytes());
    let unsupported_path = "2024/02/29/unsupported.md";
    let unsupported = original
        .replace("schema_version: 1", "schema_version: 99")
        .replace("00-00-00-123", "00-00-00-124");
    tree.write(unsupported_path, unsupported.as_bytes());
    let (_temp, db, store) = setup(&tree);
    let r = store.import_date("nav", date(), 4, &|| false).unwrap();
    let nav = r.navigation.as_ref().unwrap();
    assert_eq!(nav.groups[0].alias, "test");
    assert_ne!(nav.groups[0].id, r.scan.listings[0].id);
    assert_eq!(nav.failure_groups.len(), r.scan.failures.len());
    assert_eq!(r.scan.readable.len(), 1);
    let warning = r.scan.readable[0].clone();
    let stored = store.list("nav", &|| false).unwrap();
    assert_eq!(stored.navigation, r.navigation);
    assert_eq!(stored.scan.readable, r.scan.readable);
    let restarted = Store::new(store.sources.clone(), db);
    let view = restarted.read("nav", &warning.id, 4, &|| false).unwrap();
    assert_eq!(view.listing.token, warning.token);
    assert!(!view.body.contains("schema_version:"));
    assert!(matches!(
        restarted.read("nav", &warning.id, 5, &|| false),
        Err(ViewError::Stale)
    ));
    assert!(matches!(
        restarted.read("nav", &warning.id, 4, &|| true),
        Err(ViewError::Stale)
    ));
    tree.write(unsupported_path, format!("{unsupported}changed").as_bytes());
    assert!(matches!(
        restarted.read("nav", &warning.id, 4, &|| false),
        Err(ViewError::Stale)
    ));
    assert_eq!(
        fs::read(tree.temp.path().join(AYUMI)).unwrap(),
        original.as_bytes()
    );
}

#[test]
fn identity_conflicting_body_remains_ineligible_and_missing_is_distinct() {
    let tree = Tree::new();
    let original = String::from_utf8(ayumi()).unwrap();
    tree.write(
        AYUMI,
        original
            .replace("2024-02-29_00-00-00-123", "wrong")
            .as_bytes(),
    );
    let (_temp, _db, store) = setup(&tree);
    let r = store
        .import_date("conflict-body", date(), 1, &|| false)
        .unwrap();
    assert!(r.included.is_empty());
    assert!(r.scan.listings.is_empty());
    assert_eq!(r.scan.readable.len(), 1);
    let id = &r.scan.readable[0].id;
    assert!(store.read("conflict-body", id, 1, &|| false).is_ok());
    fs::remove_file(tree.temp.path().join(AYUMI)).unwrap();
    assert!(matches!(
        store.read("conflict-body", id, 1, &|| false),
        Err(ViewError::Outcome(SourceOutcome::Unavailable(
            Unavailable::Missing
        )))
    ));
}

#[test]
fn groups_separate_equal_paths_and_local_display_survives_zone_changes() {
    let first = Tree::new();
    let second = Tree::new();
    first.write(AYUMI, &ayumi());
    second.write(AYUMI, &ayumi());
    let (_temp, db, mut store) = setup(&first);
    store.sources.push(Source {
        alias: "other".into(),
        kind: crate::config::SourceKind::Ayumi,
        root: second.temp.path().into(),
    });
    let r = store.import_date("groups", date(), 1, &|| false).unwrap();
    assert_eq!(r.included.len(), 2);
    let nav = r.navigation.unwrap();
    assert_ne!(
        nav.sources[&r.included[0].source.id].group_id,
        nav.sources[&r.included[1].source.id].group_id
    );
    for display in nav.sources.values() {
        assert_eq!(display.included.as_ref().unwrap().date, date());
        assert_eq!(display.included.as_ref().unwrap().offset_seconds, 0);
    }
    store.capture = Arc::new(|| {
        SystemZone::from_database(
            std::path::Path::new("/var/db/timezone/zoneinfo"),
            "America/Los_Angeles",
        )
    });
    assert_eq!(
        store.list("groups", &|| false).unwrap().navigation.unwrap(),
        nav
    );
    // A new store's current rules also cannot reinterpret the persisted display.
    assert_eq!(
        Store::new(store.sources, db)
            .list("groups", &|| false)
            .unwrap()
            .navigation
            .unwrap(),
        nav
    );
}

#[test]
fn resolved_authority_is_captured_without_reclassifying_old_snapshot() {
    let tree = Tree::new();
    let conflict = String::from_utf8(ayumi())
        .unwrap()
        .replace("09:00:00.123+09:00", "09:00:01.123+09:00");
    tree.write(AYUMI, conflict.as_bytes());
    let (_temp, _db, store) = setup(&tree);
    let old = store
        .import_date("before-choice", date(), 1, &|| false)
        .unwrap();
    let source = &old.warnings[0];
    store
        .resolve(
            "before-choice",
            &source.id,
            &source.token.sha256,
            Authority::Header,
            1,
            &|| false,
        )
        .unwrap();
    let new = store
        .import_date("after-choice", date(), 1, &|| false)
        .unwrap();
    assert_eq!(new.included.len(), 1);
    assert_eq!(
        new.navigation.as_ref().unwrap().sources[&source.id].authority,
        Some(Authority::Header)
    );
    assert_eq!(
        store.list("before-choice", &|| false).unwrap().navigation,
        old.navigation
    );
    assert_eq!(
        fs::read(tree.temp.path().join(AYUMI)).unwrap(),
        conflict.as_bytes()
    );
}

#[test]
fn unpublished_scan_candidates_cannot_be_read_or_resolved() {
    let tree = Tree::new();
    tree.write(AYUMI, &ayumi());
    let (_temp, _db, store) = setup(&tree);
    let report = store
        .import_date(
            "other-date",
            DiaryDate::parse("2024-03-02").unwrap(),
            1,
            &|| false,
        )
        .unwrap();
    assert!(report.included.is_empty());
    assert!(report.warnings.is_empty());
    let candidate = &report.scan.listings[0];
    assert!(matches!(
        store.read("other-date", &candidate.id, 1, &|| false),
        Err(ViewError::Stale)
    ));
    assert_eq!(
        store.resolve(
            "other-date",
            &candidate.id,
            &candidate.token.sha256,
            Authority::Header,
            1,
            &|| false
        ),
        Err(ImportError::NotFound)
    );
}

#[test]
fn readable_descriptor_requires_a_matching_published_failure_and_root() {
    let tree = Tree::new();
    tree.write("2024/02/29/unsupported.md", &ayumi());
    let (_temp, _db, store) = setup(&tree);
    let mut report = store
        .import_date("descriptor", date(), 1, &|| false)
        .unwrap();
    let source = report.scan.readable[0].clone();
    assert!(published_readable(&report, &source).is_some());
    report.navigation.as_mut().unwrap().failure_groups[0] = "other-root".into();
    assert!(published_readable(&report, &source).is_none());
    report.scan.failures.clear();
    assert!(published_readable(&report, &source).is_none());
}

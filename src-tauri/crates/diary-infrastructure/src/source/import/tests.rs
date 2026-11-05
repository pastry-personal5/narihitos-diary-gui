use super::*;
use crate::{
    source::tests::{AYUMI, Tree, ayumi},
    sqlite::{DB_FILE, Db},
};
use diary_logging::private_fs::PrivateDir;
use std::{fs, os::unix::fs::PermissionsExt};

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
    assert_eq!(report.included.len(), 1);
    assert!(report.warnings.is_empty());
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
        store.list("import-a", &|| false).unwrap().included[0]
            .source
            .id,
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
    assert!(r.scan.failures.is_empty());
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
    let r = store.list("mixed", &|| false).unwrap();
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
    assert!(db.complete("first", 0).is_err());
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

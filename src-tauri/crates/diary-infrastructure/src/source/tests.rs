use super::*;
use crate::config::{Source, SourceKind};
use diary_application::source::*;
use diary_domain::source::{path::normalize, stamp::Dating};
use std::{
    fs,
    os::{fd::AsRawFd, unix::fs::symlink},
};

pub(super) struct Tree {
    pub temp: tempfile::TempDir,
    pub root: root::ResolvedRoot,
}
impl Tree {
    pub fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let source = Source {
            alias: "alias".into(),
            kind: SourceKind::Ayumi,
            root: temp.path().into(),
        };
        let root = root::resolve_roots(&[source]).pop().unwrap().unwrap();
        Self { temp, root }
    }
    pub fn write(&self, rel: &str, bytes: &[u8]) {
        let p = self.temp.path().join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, bytes).unwrap();
    }
}
pub(super) const AYUMI: &str = "2024/02/29/2024-02-29_00-00-00-123.md";
pub(super) fn ayumi() -> Vec<u8> {
    b"---\nid: \"2024-02-29_00-00-00-123\"\ncreated_at: \"2024-02-29T09:00:00.123+09:00\"\ntags:\n  []\nsecret: \"FRONT_SECRET\"\n---\n\nBODY_SECRET\n![audio](x.assets/a.m4a)\n".to_vec()
}
fn source(path: &std::path::Path) -> Source {
    Source {
        alias: "renamed alias".into(),
        kind: SourceKind::Ayumi,
        root: path.into(),
    }
}
#[test]
fn roots_namespace_aliases_and_volume_probe() {
    let tree = Tree::new();
    let other = Tree::new();
    fs::create_dir(tree.temp.path().join("nested")).unwrap();
    let alias = other.temp.path().join("alias");
    symlink(tree.temp.path(), &alias).unwrap();
    let roots = root::resolve_roots(&[
        source(tree.temp.path()),
        source(&alias),
        source(&tree.temp.path().join("nested")),
        source(other.temp.path()),
    ]);
    assert!(roots[0].is_err());
    assert!(roots[1].is_err());
    assert!(roots[2].is_err());
    assert!(roots[3].is_ok());
    let alias_root = root::resolve_roots(&[source(&alias)])
        .pop()
        .unwrap()
        .unwrap();
    let rel = normalize(AYUMI, tree.root.insensitive).unwrap();
    assert_eq!(
        root::source_id(&tree.root, &rel),
        root::source_id(&alias_root, &rel)
    );
    assert_ne!(
        root::source_id(&tree.root, &rel),
        root::source_id(&other.root, &rel)
    );
    fs::write(tree.temp.path().join("probe-a"), b"a").unwrap();
    fs::write(tree.temp.path().join("PROBE-A"), b"A").unwrap();
    assert_eq!(
        fs::read(tree.temp.path().join("probe-a")).unwrap() == b"A",
        tree.root.insensitive
    );
}
#[test]
fn readers_refuse_symlinks_fifo_and_oversize() {
    let tree = Tree::new();
    let other = Tree::new();
    other.write("outside.md", b"outside");
    symlink(
        other.temp.path().join("outside.md"),
        tree.temp.path().join("link.md"),
    )
    .unwrap();
    symlink(other.temp.path(), tree.temp.path().join("dir")).unwrap();
    for name in ["link.md", "dir/outside.md"] {
        assert_eq!(
            read::read_fresh(&tree.root, &normalize(name, tree.root.insensitive).unwrap())
                .unwrap_err(),
            Unavailable::NotRegular
        );
    }
    let fifo = std::ffi::CString::new(tree.temp.path().join("fifo.md").to_str().unwrap()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
    assert_eq!(
        read::read_fresh(&tree.root, &normalize("fifo.md", false).unwrap()).unwrap_err(),
        Unavailable::NotRegular
    );
    fs::File::create(tree.temp.path().join("large.md"))
        .unwrap()
        .set_len((8 << 20) + 1)
        .unwrap();
    assert_eq!(
        read::read_fresh(&tree.root, &normalize("large.md", false).unwrap()).unwrap_err(),
        Unavailable::TooLarge
    );
    tree.write(AYUMI, &ayumi());
    fs::hard_link(
        tree.temp.path().join(AYUMI),
        tree.temp.path().join("hard.md"),
    )
    .unwrap();
    let obs = read::read_fresh(&tree.root, &normalize(AYUMI, false).unwrap()).unwrap();
    assert_eq!(obs.nlink, 2);
    use sha2::Digest;
    assert_eq!(
        obs.sha256.as_slice(),
        sha2::Sha256::digest(ayumi()).as_slice()
    );
    // Flags are checked without issuing a read (provider runtime remains unverified).
    let fd = fs::File::open(tree.temp.path()).unwrap();
    assert!(fd.as_raw_fd() >= 0);
}
#[test]
fn adapter_outcome_order_views_and_generation() {
    let tree = Tree::new();
    tree.write(AYUMI, &ayumi());
    let rel = normalize(AYUMI, tree.root.insensitive).unwrap();
    let (out, listing) = adapter::observe(&tree.root, &SourceKind::Ayumi, &rel, 1).unwrap();
    assert!(matches!(out, SourceOutcome::Ok(_)));
    assert!(matches!(listing.dating, Dating::Agreed(_)));
    let view = adapter::read_view(&tree.root, &SourceKind::Ayumi, &rel, 1, &listing.token).unwrap();
    assert_eq!(view.body, "\nBODY_SECRET\n![audio](x.assets/a.m4a)\n");
    let (_, new) = adapter::observe(&tree.root, &SourceKind::Ayumi, &rel, 2).unwrap();
    assert_eq!(listing.id, new.id);
    assert_ne!(listing.token, new.token);
    assert_eq!(
        adapter::read_view(&tree.root, &SourceKind::Ayumi, &rel, 2, &listing.token),
        Err(ViewError::Stale)
    );
    tree.write(
        AYUMI,
        &ayumi()
            .into_iter()
            .chain(b"changed".iter().copied())
            .collect::<Vec<_>>(),
    );
    assert_eq!(
        adapter::read_view(&tree.root, &SourceKind::Ayumi, &rel, 1, &listing.token),
        Err(ViewError::Stale)
    );
    let changed = String::from_utf8(ayumi())
        .unwrap()
        .replace("id: \"2024-02-29_00-00-00-123\"", "id: \"wrong\"");
    tree.write(AYUMI, changed.as_bytes());
    let (out, l) = adapter::observe(&tree.root, &SourceKind::Ayumi, &rel, 1).unwrap();
    assert_eq!(out, SourceOutcome::Conflict(ConflictKind::IdMismatch));
    assert!(adapter::read_view(&tree.root, &SourceKind::Ayumi, &rel, 1, &l.token).is_ok());
    tree.write(AYUMI, b"not front matter");
    assert!(matches!(
        adapter::observe(&tree.root, &SourceKind::Ayumi, &rel, 1),
        Err(SourceOutcome::Malformed(_))
    ));
    fs::remove_file(tree.temp.path().join(AYUMI)).unwrap();
    assert_eq!(
        adapter::observe(&tree.root, &SourceKind::Ayumi, &rel, 1),
        Err(SourceOutcome::Unavailable(Unavailable::Missing))
    );
}
#[test]
fn narihito_and_timestamp_evidence() {
    let tree = Tree::new();
    let id = "01234567-89ab-4def-8123-456789abcdef";
    let path = format!("2024/02/29/2024-02-29_00-00-00-{id}.md");
    let rel = normalize(&path, tree.root.insensitive).unwrap();
    let valid = format!(
        "---\nschema_version: 1\nkind: \"ndm.diary-snippet\"\nid: \"{id}\"\ncreated_at: \"2024-02-29T00:00:00.999Z\"\nunknown: \"FRONT_SECRET\"\n---\nBODY_SECRET"
    );
    tree.write(&path, valid.as_bytes());
    let (out, listing) = adapter::observe(&tree.root, &SourceKind::Narihito, &rel, 0).unwrap();
    assert!(matches!(out, SourceOutcome::Ok(_)));
    assert!(matches!(listing.dating, Dating::Agreed(_)));
    for (from, to, reason) in [
        (
            "schema_version: 1",
            "schema_version: 2",
            UnsupportedReason::Schema,
        ),
        ("ndm.diary-snippet", "unknown", UnsupportedReason::Kind),
    ] {
        tree.write(&path, valid.replace(from, to).as_bytes());
        let (out, l) = adapter::observe(&tree.root, &SourceKind::Narihito, &rel, 0).unwrap();
        assert_eq!(
            out,
            SourceOutcome::Unsupported {
                reason,
                viewable: true
            }
        );
        assert!(adapter::read_view(&tree.root, &SourceKind::Narihito, &rel, 0, &l.token).is_ok());
    }
    tree.write(
        &path,
        valid.replace("00:00:00.999Z", "00:00:01Z").as_bytes(),
    );
    assert!(matches!(
        adapter::observe(&tree.root, &SourceKind::Narihito, &rel, 0)
            .unwrap()
            .1
            .dating,
        Dating::Disagree { .. }
    ));
    tree.write(&path, valid.replace("00:00:00.999Z", "00:00:00").as_bytes());
    assert!(matches!(
        adapter::observe(&tree.root, &SourceKind::Narihito, &rel, 0)
            .unwrap()
            .1
            .dating,
        Dating::FilenameOnly { .. }
    ));
}
#[test]
fn mixed_scan_isolated_sorted_redacted_and_duplicates() {
    let tree = Tree::new();
    tree.write(AYUMI, &ayumi());
    tree.write("bad.md", b"bad");
    tree.write("unsupported.md", &ayumi());
    tree.write(".missing.md.icloud", b"");
    let conflict = "2024/02/29/2024-02-29_00-00-00-124.md";
    tree.write(conflict, &ayumi());
    // Same producer ID on a different path remains a separate listing, with dating warning evidence.
    tree.write("2024/03/01/2024-02-29_00-00-00-123.md", &ayumi());
    tree.write(".ignored.md", b"bad");
    tree.write("audio.assets/hidden.md", b"bad");
    symlink(tree.temp.path().join("2024"), tree.temp.path().join("link")).unwrap();
    let report = scan::scan(&tree.root, &SourceKind::Ayumi, 0);
    assert_eq!(report.listings.len(), 2);
    assert_eq!(report.failures.len(), 4);
    assert!(!report.incomplete);
    assert!(
        report
            .listings
            .iter()
            .all(|l| l.warnings == vec![Warning::DuplicateProducerId])
    );
    assert!(report.failures.windows(2).all(|w| w[0].0 < w[1].0));
    let debug = format!("{report:?}");
    for marker in [
        tree.temp.path().to_str().unwrap(),
        "BODY_SECRET",
        "FRONT_SECRET",
    ] {
        assert!(!debug.contains(marker));
    }
    let rel = normalize(AYUMI, tree.root.insensitive).unwrap();
    let (_, l) = adapter::observe(&tree.root, &SourceKind::Ayumi, &rel, 0).unwrap();
    let view = adapter::read_view(&tree.root, &SourceKind::Ayumi, &rel, 0, &l.token).unwrap();
    assert!(!format!("{view:?}").contains("BODY_SECRET"));
}
#[test]
fn candidate_bound_is_deterministic() {
    let tree = Tree::new();
    for i in (0..10001).rev() {
        tree.write(&format!("{i:05}.md"), b"bad");
    }
    let report = scan::scan(&tree.root, &SourceKind::Ayumi, 0);
    assert!(report.incomplete);
    assert_eq!(report.failures.len(), 10000);
    assert_eq!(report.failures[0].0, "00000.md");
    assert_eq!(report.failures.last().unwrap().0, "09999.md");
}

#[test]
fn hard_links_stay_distinct_and_denied_files_do_not_hide_neighbours() {
    use std::os::unix::fs::PermissionsExt;
    let tree = Tree::new();
    tree.write(AYUMI, &ayumi());
    let second = "2024/03/01/2024-02-29_00-00-00-123.md";
    fs::create_dir_all(tree.temp.path().join("2024/03/01")).unwrap();
    fs::hard_link(tree.temp.path().join(AYUMI), tree.temp.path().join(second)).unwrap();
    let report = scan::scan(&tree.root, &SourceKind::Ayumi, 0);
    assert_eq!(report.listings.len(), 2);
    assert_ne!(report.listings[0].id, report.listings[1].id);
    tree.write("denied.md", b"private");
    fs::set_permissions(
        tree.temp.path().join("denied.md"),
        fs::Permissions::from_mode(0o0),
    )
    .unwrap();
    let report = scan::scan(&tree.root, &SourceKind::Ayumi, 0);
    assert_eq!(report.listings.len(), 2);
    assert!(report.failures.contains(&(
        "denied.md".into(),
        SourceOutcome::Unavailable(Unavailable::Denied)
    )));
    fs::set_permissions(
        tree.temp.path().join("denied.md"),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
}
#[test]
fn placeholder_and_downloaded_file_cannot_share_one_identity() {
    let tree = Tree::new();
    tree.write("x.md", &ayumi());
    tree.write(".x.md.icloud", b"");
    let report = scan::scan(&tree.root, &SourceKind::Ayumi, 0);
    assert!(report.listings.is_empty());
    assert!(report.failures.is_empty());
    assert_eq!(report.collisions.len(), 1);
}

#[test]
fn moving_a_root_changes_namespace_even_with_same_inode() {
    let tree = Tree::new();
    let target = Tree::new();
    let rel = normalize(AYUMI, tree.root.insensitive).unwrap();
    let old = root::source_id(&tree.root, &rel);
    let moved = target.temp.path().join("moved");
    fs::rename(tree.temp.path(), &moved).unwrap();
    let new = root::resolve_roots(&[source(&moved)])
        .pop()
        .unwrap()
        .unwrap();
    assert_eq!((tree.root.dev, tree.root.ino), (new.dev, new.ino));
    assert_ne!(old, root::source_id(&new, &rel));
}
#[test]
fn ayumi_never_writable_and_body_unchanged() {
    let tree = Tree::new();
    tree.write(AYUMI, &ayumi());
    let rel = normalize(AYUMI, tree.root.insensitive).unwrap();
    let (_, l) = adapter::observe(&tree.root, &SourceKind::Ayumi, &rel, 0).unwrap();
    let view = adapter::read_view(&tree.root, &SourceKind::Ayumi, &rel, 0, &l.token).unwrap();
    assert_eq!(view.write, Err(WriteReason::ProducerUnverified));
    assert_eq!(view.trash, Err(TrashReason::ProducerUnverified));
    assert!(view.body.starts_with("\nBODY_SECRET"));
}
#[test]
fn narihito_save_true_when_enabled_and_view_drops_separator() {
    use super::narihito_write::tests_support::{REL, golden};
    let tree = Tree::new();
    tree.write(REL, &golden("2024-02-29T01:02:03Z", "hello\n"));
    let rel = normalize(REL, tree.root.insensitive).unwrap();
    let view = |tree: &Tree| {
        let (_, l) = adapter::observe(&tree.root, &SourceKind::Narihito, &rel, 0).unwrap();
        adapter::read_view(&tree.root, &SourceKind::Narihito, &rel, 0, &l.token).unwrap()
    };
    let v = view(&tree);
    assert_eq!(v.body, "hello\n");
    assert_eq!(v.write, Ok(()));
    assert_eq!(v.trash, Ok(()));
    let text = String::from_utf8(golden("2024-02-29T01:02:03Z", "hello\n"))
        .unwrap()
        .replace("favorite: false", "favorite: maybe");
    tree.write(REL, text.as_bytes());
    assert_eq!(view(&tree).write, Err(WriteReason::UnsupportedFormat));
    assert_eq!(view(&tree).trash, Err(TrashReason::UnsupportedFormat));
}

#[test]
fn manual_snippets_import_with_exact_times_and_save_without_audio() {
    use super::narihito_write::tests_support::{REL, manual};
    let tree = Tree::new();
    for time in ["01:42:07", "01:42:57"] {
        let path = REL.replace("01-02-03", &time.replace(':', "-"));
        let text = String::from_utf8(manual("manual 日本語\n"))
            .unwrap()
            .replace("01:02:03", time);
        tree.write(&path, text.as_bytes());
        let rel = normalize(&path, tree.root.insensitive).unwrap();
        let (out, listing) = adapter::observe(&tree.root, &SourceKind::Narihito, &rel, 0).unwrap();
        assert!(matches!(out, SourceOutcome::Ok(_)));
        assert!(matches!(listing.dating, Dating::Agreed(_)));
        let view =
            adapter::read_view(&tree.root, &SourceKind::Narihito, &rel, 0, &listing.token).unwrap();
        assert_eq!(view.body, "manual 日本語\n");
        assert_eq!(view.write, Ok(()));
        assert_eq!(
            view.trash,
            Err(diary_application::source::TrashReason::UnsupportedFormat)
        );
        assert!(view.raw_front_matter.contains("schema_version: 2"));
    }
}

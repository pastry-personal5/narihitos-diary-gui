use super::*;
use crate::source::{
    narihito_write::tests_support::{REL, golden},
    tests::Tree,
};
use sha2::{Digest, Sha256};
use std::{fs, os::unix::fs::symlink};

fn sources(tree: &Tree) -> Vec<Source> {
    vec![Source {
        alias: "test".into(),
        kind: SourceKind::Narihito,
        root: tree.temp.path().into(),
    }]
}
fn fixture() -> (Tree, String) {
    let tree = Tree::new();
    tree.write(
        REL,
        &golden("2024-02-29T01:02:03Z", "![ignored](../../private.m4a)\n"),
    );
    let audio = format!(
        "{}.assets/0b1d2c3e-4f50-4a6b-8c7d-9e0f1a2b3c4d.m4a",
        REL.strip_suffix(".md").unwrap()
    );
    tree.write(
        &audio,
        b"synthetic audio for digest checks, not producer decoding evidence",
    );
    (tree, audio)
}

#[test]
fn derives_exact_pair_and_ignores_body_links_and_siblings() {
    let (tree, audio) = fixture();
    tree.write(
        &format!("{}.assets/unrelated.m4a", REL.strip_suffix(".md").unwrap()),
        b"neighbor",
    );
    let before = fs::read(tree.temp.path().join(REL)).unwrap();
    let plan = prepare_pair(&sources(&tree), 0, REL, &|| false).unwrap();
    assert_eq!(plan.audio_rel, audio);
    assert_eq!(plan.markdown_rel, REL);
    assert_eq!(
        plan.audio.as_ref().unwrap().sha256,
        <[u8; 32]>::from(Sha256::digest(
            fs::read(tree.temp.path().join(audio)).unwrap()
        ))
    );
    assert_eq!(fs::read(tree.temp.path().join(REL)).unwrap(), before);
    assert!(!format!("{plan:?}").contains(REL));
}

#[test]
fn absent_audio_is_explicit_and_a_new_file_changes_the_plan() {
    let (tree, audio) = fixture();
    fs::remove_file(tree.temp.path().join(&audio)).unwrap();
    let plan = prepare_pair(&sources(&tree), 0, REL, &|| false).unwrap();
    assert!(plan.audio.is_none());
    tree.write(&audio, b"appeared");
    assert!(
        prepare_pair(&sources(&tree), 0, REL, &|| false)
            .unwrap()
            .audio
            .is_some()
    );
    fs::remove_file(tree.temp.path().join(&audio)).unwrap();
    fs::remove_dir(tree.temp.path().join(audio).parent().unwrap()).unwrap();
    assert!(
        prepare_pair(&sources(&tree), 0, REL, &|| false)
            .unwrap()
            .audio
            .is_none()
    );
}

#[test]
fn refuses_links_and_unsafe_ancestors() {
    let (tree, audio) = fixture();
    let other = Tree::new();
    other.write("other", b"outside");
    fs::remove_file(tree.temp.path().join(&audio)).unwrap();
    symlink(
        other.temp.path().join("other"),
        tree.temp.path().join(&audio),
    )
    .unwrap();
    assert_eq!(
        prepare_pair(&sources(&tree), 0, REL, &|| false).unwrap_err(),
        PreparationError::UnsafePath
    );
    fs::remove_file(tree.temp.path().join(&audio)).unwrap();
    fs::hard_link(
        other.temp.path().join("other"),
        tree.temp.path().join(&audio),
    )
    .unwrap();
    assert_eq!(
        prepare_pair(&sources(&tree), 0, REL, &|| false).unwrap_err(),
        PreparationError::UnsafePath
    );
    fs::remove_file(tree.temp.path().join(&audio)).unwrap();
    fs::remove_dir(tree.temp.path().join(&audio).parent().unwrap()).unwrap();
    symlink(
        other.temp.path(),
        tree.temp.path().join(&audio).parent().unwrap(),
    )
    .unwrap();
    assert_eq!(
        prepare_pair(&sources(&tree), 0, REL, &|| false).unwrap_err(),
        PreparationError::UnsafePath
    );
}

#[test]
fn declared_hash_must_match_full_audio_and_oversize_is_refused() {
    let (tree, audio) = fixture();
    let bytes = String::from_utf8(fs::read(tree.temp.path().join(REL)).unwrap()).unwrap();
    tree.write(
        REL,
        bytes
            .replacen(
                "favorite: false",
                &format!("favorite: false\naudio_sha256: \"{}\"", "0".repeat(64)),
                1,
            )
            .as_bytes(),
    );
    assert_eq!(
        prepare_pair(&sources(&tree), 0, REL, &|| false).unwrap_err(),
        PreparationError::Changed
    );
    let hash = Sha256::digest(fs::read(tree.temp.path().join(&audio)).unwrap());
    let hash: String = hash.iter().map(|b| format!("{b:02x}")).collect();
    tree.write(
        REL,
        bytes
            .replacen(
                "favorite: false",
                &format!("favorite: false\naudio_sha256: \"{hash}\""),
                1,
            )
            .as_bytes(),
    );
    assert!(prepare_pair(&sources(&tree), 0, REL, &|| false).is_ok());
    fs::OpenOptions::new()
        .write(true)
        .open(tree.temp.path().join(audio))
        .unwrap()
        .set_len(MAX_AUDIO_BYTES + 1)
        .unwrap();
    assert_eq!(
        prepare_pair(&sources(&tree), 0, REL, &|| false).unwrap_err(),
        PreparationError::Limit
    );
}

#[test]
fn claims_across_roots_and_dates_and_incomplete_observations_block() {
    let (tree, _) = fixture();
    let other = Tree::new();
    // A second valid producer document with the same ID, on a different date/root.
    let bytes = String::from_utf8(golden("2024-02-29T01:02:03Z", ""))
        .unwrap()
        .replace("2024-02-29", "2024-03-01");
    let rel = REL
        .replace("2024/02/29", "2024/03/01")
        .replace("2024-02-29", "2024-03-01");
    other.write(&rel, bytes.as_bytes());
    let mut all = sources(&tree);
    all.extend(sources(&other));
    assert_eq!(
        prepare_pair(&all, 0, REL, &|| false).unwrap_err(),
        PreparationError::OwnershipAmbiguous
    );
    fs::remove_file(other.temp.path().join(rel)).unwrap();
    other.write("unknown.md", b"cannot assess ownership");
    assert_eq!(
        prepare_pair(&all, 0, REL, &|| false).unwrap_err(),
        PreparationError::OwnershipAmbiguous
    );
    fs::remove_file(other.temp.path().join("unknown.md")).unwrap();
    symlink(tree.temp.path(), other.temp.path().join("linked-directory")).unwrap();
    assert_eq!(
        prepare_pair(&all, 0, REL, &|| false).unwrap_err(),
        PreparationError::OwnershipAmbiguous
    );
}

#[test]
fn unsupported_producer_metadata_cancellation_and_repointed_roots_block() {
    let (tree, _) = fixture();
    assert_eq!(
        prepare_pair(&sources(&tree), 0, REL, &|| true).unwrap_err(),
        PreparationError::Changed
    );
    let mut configured = sources(&tree);
    configured[0].kind = SourceKind::Ayumi;
    assert_eq!(
        prepare_pair(&configured, 0, REL, &|| false).unwrap_err(),
        PreparationError::Unsupported
    );
    let bytes = String::from_utf8(golden("2024-02-29T01:02:03Z", ""))
        .unwrap()
        .replace(".m4a\"", "../../other.m4a\"");
    tree.write(REL, bytes.as_bytes());
    assert_eq!(
        prepare_pair(&sources(&tree), 0, REL, &|| false).unwrap_err(),
        PreparationError::Unsupported
    );
    let outside = Tree::new();
    let roots = resolve_roots(&sources(&tree));
    let root = roots.into_iter().next().unwrap().unwrap();
    let moved = outside.temp.path().join("moved");
    fs::rename(tree.temp.path(), &moved).unwrap();
    fs::create_dir(tree.temp.path()).unwrap();
    let path = normalize(REL, root.insensitive).unwrap();
    assert_eq!(
        optional_audio(&root, &path, &|| false),
        Err(PreparationError::RootChanged)
    );
}

#[test]
fn cloud_placeholder_cannot_be_reported_as_missing_audio() {
    let (tree, audio) = fixture();
    fs::remove_file(tree.temp.path().join(&audio)).unwrap();
    let (parent, name) = audio.rsplit_once('/').unwrap();
    tree.write(&format!("{parent}/.{name}.icloud"), b"placeholder");
    assert_eq!(
        prepare_pair(&sources(&tree), 0, REL, &|| false).unwrap_err(),
        PreparationError::Unavailable
    );
}

#[test]
fn manual_snippet_never_prepares_a_guessed_audio_pair() {
    use crate::source::narihito_write::tests_support::manual;
    let (tree, audio) = fixture();
    tree.write(REL, &manual("manual text\n"));
    let before = fs::read(tree.temp.path().join(&audio)).unwrap();
    assert!(matches!(
        prepare_pair(&sources(&tree), 0, REL, &|| false),
        Err(PreparationError::Unsupported)
    ));
    assert_eq!(fs::read(tree.temp.path().join(audio)).unwrap(), before);
}

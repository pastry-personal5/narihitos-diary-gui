use super::*;
use std::{fs, os::unix::fs::PermissionsExt, path::Path};

struct Fx {
    _temp: tempfile::TempDir,
    root: PathBuf,
    dir: PathBuf,
    store: Store,
}
fn fx() -> Fx {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let dir = root.join("app");
    Fx {
        store: Store::new(dir.clone()),
        _temp: temp,
        root,
        dir,
    }
}
fn put(f: &Fx, text: &str) {
    fs::create_dir_all(&f.dir).unwrap();
    fs::set_permissions(&f.dir, fs::Permissions::from_mode(0o700)).unwrap();
    let file = f.dir.join(FILE);
    let _ = fs::remove_file(&file);
    fs::write(&file, text).unwrap();
    fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
}
fn codes(s: &Snapshot) -> Vec<(&'static str, &'static str)> {
    s.warnings.iter().map(|w| (w.section, w.code)).collect()
}
fn on_disk(f: &Fx) -> Value {
    serde_json::from_slice(&fs::read(f.dir.join(FILE)).unwrap()).unwrap()
}

#[test]
fn missing_file_gives_defaults_without_creating_anything() {
    let f = fx();
    let s = f.store.snapshot();
    assert_eq!(s.revision, "missing");
    assert!(s.warnings.is_empty() && !s.read_only && s.writing_locale.is_none());
    assert!(!f.dir.exists());
}

#[test]
fn bad_files_fall_back_with_a_message_and_stay_untouched() {
    let cases = [
        "{not json",
        r#"{"schema_version":1,"writing":{"locale":"en-US"},"writing":{"locale":"ja-JP"}}"#,
        r#"{"schema_version":1,"surprise":{}}"#,
        r#"{"writing":{"locale":"en-US"}}"#,
        r#"{"schema_version":"1"}"#,
        r#"[]"#,
    ];
    for text in cases {
        let f = fx();
        put(&f, text);
        let s = f.store.snapshot();
        assert_eq!(codes(&s), [("file", "file_invalid")], "{text}");
        assert!(s.recoverable && s.writing_locale.is_none());
        assert_eq!(
            f.store
                .update(&s.revision, Change::Writing("en-US".into()))
                .err(),
            Some(Error::FileInvalid)
        );
        assert_eq!(fs::read_to_string(f.dir.join(FILE)).unwrap(), text);
    }
    let f = fx();
    put(
        &f,
        &format!(
            r#"{{"schema_version":1,"keybindings":{{"a":["{}"]}}}}"#,
            "x".repeat(300_000)
        ),
    );
    assert_eq!(codes(&f.store.snapshot()), [("file", "file_invalid")]);
}

#[test]
fn newer_schema_is_read_only_and_rejects_updates_untouched() {
    let f = fx();
    let text = r#"{"schema_version":2,"future":true}"#;
    put(&f, text);
    let s = f.store.snapshot();
    assert!(s.read_only && s.recoverable);
    assert_eq!(codes(&s), [("file", "newer_schema")]);
    for change in [
        Change::Writing("en-US".into()),
        Change::Keybindings(json!({})),
    ] {
        assert_eq!(
            f.store.update(&s.revision, change).err(),
            Some(Error::ReadOnly)
        );
    }
    assert_eq!(fs::read_to_string(f.dir.join(FILE)).unwrap(), text);
}

#[test]
fn a_bad_section_degrades_alone_and_survives_other_updates() {
    let f = fx();
    put(
        &f,
        r#"{"schema_version":1,"keybindings":{"palette.open":"nope"},"writing":{"locale":"ko-KR"}}"#,
    );
    let s = f.store.snapshot();
    assert_eq!(codes(&s), [("keybindings", "section_invalid")]);
    assert_eq!(s.writing_locale.as_deref(), Some("ko-KR"));
    assert_eq!(s.keybindings, json!({}));
    f.store
        .update(&s.revision, Change::Writing("en-US".into()))
        .unwrap();
    // The invalid keybindings payload is preserved byte for byte in meaning.
    assert_eq!(on_disk(&f)["keybindings"], json!({"palette.open":"nope"}));
    assert_eq!(on_disk(&f)["writing"], json!({"locale":"en-US"}));
}

#[test]
fn writes_create_a_private_directory_and_file_and_merge_sections() {
    let f = fx();
    let s = f.store.snapshot();
    let s = f
        .store
        .update(&s.revision, Change::Writing("ja_jp".into()))
        .unwrap();
    assert_eq!(s.writing_locale.as_deref(), Some("ja-JP"));
    let mode = |p: &Path| fs::metadata(p).unwrap().permissions().mode() & 0o7777;
    assert_eq!(mode(&f.dir), 0o700);
    assert_eq!(mode(&f.dir.join(FILE)), 0o600);
    let s = f
        .store
        .update(
            &s.revision,
            Change::Keybindings(json!({"palette.open":["Shift+Command+P"]})),
        )
        .unwrap();
    assert_eq!(s.writing_locale.as_deref(), Some("ja-JP"));
    assert_eq!(s.keybindings, json!({"palette.open":["Shift+Command+P"]}));
    assert_eq!(on_disk(&f)["schema_version"], 1);
    assert!(!f.dir.join(TEMP).exists());
}

#[test]
fn a_stale_revision_conflicts_without_overwriting() {
    let f = fx();
    let first = f.store.snapshot();
    let after = f
        .store
        .update(&first.revision, Change::Writing("en-US".into()))
        .unwrap();
    let other_window = f
        .store
        .update(&first.revision, Change::Writing("ko-KR".into()));
    assert_eq!(other_window.err(), Some(Error::Conflict));
    assert_eq!(f.store.snapshot().writing_locale.as_deref(), Some("en-US"));
    // A change made behind the store's back (another process) conflicts too.
    put(&f, r#"{"schema_version":1,"writing":{"locale":"fr-FR"}}"#);
    assert_eq!(
        f.store
            .update(&after.revision, Change::Writing("ko-KR".into()))
            .err(),
        Some(Error::Conflict)
    );
    assert_eq!(on_disk(&f)["writing"], json!({"locale":"fr-FR"}));
}

#[test]
fn a_failure_before_rename_keeps_the_old_file_and_leaves_no_temp() {
    let f = fx();
    let s = f.store.snapshot();
    let s = f
        .store
        .update(&s.revision, Change::Writing("en-US".into()))
        .unwrap();
    f.store.set_fault(Fault::BeforeRename);
    assert_eq!(
        f.store
            .update(&s.revision, Change::Writing("ko-KR".into()))
            .err(),
        Some(Error::Unavailable)
    );
    f.store.set_fault(Fault::None);
    assert_eq!(on_disk(&f)["writing"], json!({"locale":"en-US"}));
    assert!(!f.dir.join(TEMP).exists());
    // Not uncertain: the next attempt with the same revision goes through.
    f.store
        .update(&s.revision, Change::Writing("ko-KR".into()))
        .unwrap();
}

#[test]
fn a_failure_after_rename_is_uncertain_until_the_settings_are_reloaded() {
    let f = fx();
    let s = f.store.snapshot();
    f.store.set_fault(Fault::AfterRename);
    assert_eq!(
        f.store
            .update(&s.revision, Change::Writing("en-US".into()))
            .err(),
        Some(Error::Uncertain)
    );
    f.store.set_fault(Fault::None);
    assert_eq!(
        f.store
            .update("missing", Change::Writing("ko-KR".into()))
            .err(),
        Some(Error::Uncertain)
    );
    let reloaded = f.store.snapshot();
    assert_eq!(reloaded.writing_locale.as_deref(), Some("en-US"));
    f.store
        .update(&reloaded.revision, Change::Writing("ko-KR".into()))
        .unwrap();
}

#[test]
fn recovery_backs_up_the_original_first_and_ordinary_updates_never_do() {
    for text in ["{broken", r#"{"schema_version":3}"#] {
        let f = fx();
        put(&f, text);
        let s = f.store.snapshot();
        let fresh = f.store.update(&s.revision, Change::Recover).unwrap();
        assert!(fresh.warnings.is_empty() && !fresh.read_only);
        let backups: Vec<_> = fs::read_dir(&f.dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .filter(|n| n.contains(".bak-"))
            .collect();
        assert_eq!(backups.len(), 1);
        assert_eq!(fs::read_to_string(f.dir.join(&backups[0])).unwrap(), text);
        assert_eq!(
            fs::metadata(f.dir.join(&backups[0]))
                .unwrap()
                .permissions()
                .mode()
                & 0o7777,
            0o600
        );
        assert_eq!(on_disk(&f), json!({"schema_version":1}));
    }
    let f = fx();
    let s = f.store.snapshot();
    assert_eq!(
        f.store.update(&s.revision, Change::Recover).err(),
        Some(Error::Invalid)
    );
}

#[test]
fn unsafe_locations_are_reported_and_never_followed() {
    let f = fx();
    put(&f, r#"{"schema_version":1}"#);
    fs::set_permissions(f.dir.join(FILE), fs::Permissions::from_mode(0o644)).unwrap();
    let s = f.store.snapshot();
    assert_eq!(codes(&s), [("file", "location_unsafe")]);
    assert_eq!(
        f.store
            .update(&s.revision, Change::Writing("en-US".into()))
            .err(),
        Some(Error::Unavailable)
    );
    let g = fx();
    let target = g.root.join("elsewhere");
    fs::create_dir(&target).unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o700)).unwrap();
    std::os::unix::fs::symlink(&target, &g.dir).unwrap();
    let s = g.store.snapshot();
    assert_eq!(codes(&s), [("file", "location_unsafe")]);
    assert_eq!(
        g.store
            .update(&s.revision, Change::Writing("en-US".into()))
            .err(),
        Some(Error::Unavailable)
    );
    assert!(fs::read_dir(&target).unwrap().next().is_none());
}

#[test]
fn first_run_stores_the_first_valid_system_language_once() {
    let f = fx();
    let s = f.store.snapshot();
    let s = f
        .store
        .update(
            &s.revision,
            Change::InitWriting(vec!["not a tag".into(), "ko_KR".into(), "en-US".into()]),
        )
        .unwrap();
    assert_eq!(s.writing_locale.as_deref(), Some("ko-KR"));
    // A saved choice is never inferred again, and nothing is rewritten.
    let again = f
        .store
        .update(&s.revision, Change::InitWriting(vec!["en-US".into()]))
        .unwrap();
    assert_eq!(again.revision, s.revision);
    assert_eq!(again.writing_locale.as_deref(), Some("ko-KR"));
}

#[test]
fn an_unresolved_language_is_an_error_and_writes_nothing() {
    let f = fx();
    let s = f.store.snapshot();
    for list in [vec![], vec!["???".to_string(), "x".to_string()]] {
        assert_eq!(
            f.store.update(&s.revision, Change::InitWriting(list)).err(),
            Some(Error::Unresolved)
        );
    }
    assert!(!f.dir.exists());
    // An invalid saved section is user data too: init leaves it alone.
    put(&f, r#"{"schema_version":1,"writing":{"locale":"???"}}"#);
    let s = f.store.snapshot();
    let after = f
        .store
        .update(&s.revision, Change::InitWriting(vec!["en-US".into()]))
        .unwrap();
    assert_eq!(codes(&after), [("writing", "section_invalid")]);
    assert_eq!(on_disk(&f)["writing"], json!({"locale":"???"}));
}

#[test]
fn invalid_input_is_rejected_before_any_write() {
    let f = fx();
    let s = f.store.snapshot();
    for change in [
        Change::Writing("english".into()),
        Change::Writing("en-US-x-private".into()),
        Change::Keybindings(json!([])),
        Change::Keybindings(json!({"a":"b"})),
        Change::Keybindings(json!({"a":[1]})),
    ] {
        assert_eq!(
            f.store.update(&s.revision, change).err(),
            Some(Error::Invalid)
        );
    }
    assert!(!f.dir.exists());
}

#[test]
fn language_tags_canonicalise_within_a_bounded_subset() {
    for (input, want) in [
        ("ja-JP", Some("ja-JP")),
        ("JA_jp", Some("ja-JP")),
        ("en", Some("en")),
        ("zh-hans-cn", Some("zh-Hans-CN")),
        ("es-419", Some("es-419")),
        ("de-DE-1996", Some("de-DE-1996")),
        ("", None),
        ("e", None),
        ("english", None),
        ("en-US-u-ca-gregory", None),
        ("en-x-private", None),
        ("en--US", None),
        ("日本語", None),
    ] {
        assert_eq!(canonical_tag(input).as_deref(), want, "{input}");
    }
    assert!(canonical_tag(&"a".repeat(40)).is_none());
}

#[test]
fn snapshots_hold_no_path_and_the_store_lives_outside_workspace_roots() {
    let f = fx();
    let s = f.store.snapshot();
    let text = serde_json::to_string(&s).unwrap();
    assert!(!text.contains(f.root.to_str().unwrap()));
    // The store only ever uses the one directory it was given (the Tauri app config dir).
    assert_eq!(f.store.dir, f.dir);
}

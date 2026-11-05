use super::*;
use std::os::unix::fs::{PermissionsExt, symlink};

const MINIMAL: &str =
    "schema_version: 1\npaths:\n  entries_root: /workspace\n  runtime_root: /runtime\n";
fn error(text: &str) -> ConfigError {
    parse(text.as_bytes()).err().expect("must reject fixture")
}

#[test]
fn minimal_and_tagged_profiles_accept_only_their_fields() {
    let c = parse(MINIMAL.as_bytes()).unwrap();
    assert_eq!(c.server.port, 8781);
    assert_eq!(c.server.port_fallback_count, 8);
    assert!(c.sources.is_empty() && c.agents.profiles.is_empty());
    for kind in ["codex", "claude", "qwen"] {
        let valid = format!(
            "{MINIMAL}agents:\n  profiles:\n    - alias: personal\n      type: {kind}\n      profile_home: profiles/personal\n"
        );
        assert!(parse(valid.as_bytes()).is_ok());
        assert!(parse(valid.replace("profile_home:", "home:").as_bytes()).is_err());
        assert!(
            parse(
                valid
                    .replace("      profile_home: profiles/personal\n", "")
                    .as_bytes()
            )
            .is_err()
        );
    }
    let apple = format!(
        "{MINIMAL}agents:\n  profiles:\n    - alias: local\n      type: apple-foundation-models\n"
    );
    assert!(parse(apple.as_bytes()).is_ok());
    for extra in [
        "profile_home: /private/home",
        "executable: /bin/false",
        "model: other",
        "token: invented",
    ] {
        assert!(parse(format!("{apple}      {extra}\n").as_bytes()).is_err());
    }
}

#[test]
fn yaml_language_and_type_confusion_fail_without_echoing_values() {
    for extra in [
        "schema_version: 1\n",
        "unknown: secret\n",
        "notion: {root_page_id: true, token: secret}\n",
        "notion: {root_page_id: page, token: 123}\n",
        "sources: &x []\n",
        "sources: !!seq []\n",
        "sources: !custom []\n",
        "sources: *missing\n",
    ] {
        assert!(
            parse(format!("{MINIMAL}{extra}").as_bytes()).is_err(),
            "rejected fixture"
        );
    }
    assert_eq!(
        error(&MINIMAL.replace("version: 1", "version: 2")),
        ConfigError::Version
    );
    assert_eq!(
        error(&MINIMAL.replace("/workspace", "relative")),
        ConfigError::Path
    );
    // Quoted punctuation is ordinary string data, not a YAML tag or alias.
    assert!(
        parse(
            format!("{MINIMAL}notion: {{root_page_id: page, token: '!&*invented'}}\n").as_bytes()
        )
        .is_ok()
    );
    assert_eq!(error(&" ".repeat(MAX_CONFIG_BYTES + 1)), ConfigError::Limit);
    assert_eq!(
        error(&format!("x: {}0{}", "[".repeat(17), "]".repeat(17))),
        ConfigError::Limit
    );
}

#[test]
fn port_range_and_alias_bounds_are_enforced() {
    for (port, count, valid) in [
        (8781, 0, true),
        (8781, 8, true),
        (65535, 0, true),
        (65535, 1, false),
        (1023, 0, false),
        (8781, 9, false),
    ] {
        assert_eq!(
            parse(
                format!("{MINIMAL}server: {{port: {port}, port_fallback_count: {count}}}\n")
                    .as_bytes()
            )
            .is_ok(),
            valid
        );
    }
    let source = "sources:\n  - {alias: mobile, type: narihito, root: /source}\n  - {alias: mobile, type: ayumi, root: /other}\n";
    assert_eq!(error(&format!("{MINIMAL}{source}")), ConfigError::Alias);
    assert_eq!(
        error(&format!(
            "{MINIMAL}sources:\n{}",
            "  - {alias: x, type: ayumi, root: /source}\n".repeat(65)
        )),
        ConfigError::Limit
    );
}

#[test]
fn root_aliases_nesting_and_private_path_overlap_are_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    fs::create_dir(root.join("workspace")).unwrap();
    symlink(root.join("workspace"), root.join("alias")).unwrap();
    let config_file = root.join("config.yaml");
    for runtime in ["workspace", "workspace/nested", "alias"] {
        let text = format!(
            "schema_version: 1\npaths: {{entries_root: '{}', runtime_root: '{}'}}\n",
            root.join("workspace").display(),
            root.join(runtime).display()
        );
        let mut c = parse(text.as_bytes()).unwrap();
        assert_eq!(c.resolve_paths(&config_file), Err(ConfigError::Overlap));
    }
    let text = format!(
        "schema_version: 1\npaths: {{entries_root: '{}', runtime_root: '{}'}}\nagents:\n  profiles:\n    - {{alias: p, type: codex, profile_home: workspace/profile}}\n",
        root.join("workspace").display(),
        root.join("runtime").display()
    );
    let mut c = parse(text.as_bytes()).unwrap();
    assert_eq!(c.resolve_paths(&config_file), Err(ConfigError::Overlap));
}

#[test]
fn private_open_rejects_unsafe_modes_and_symlinks_including_ancestors() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let config = root.join("config.yaml");
    fs::write(&config, MINIMAL).unwrap();
    fs::set_permissions(&config, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(open_private(&config).is_ok());
    fs::set_permissions(&config, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(open_private(&config).is_err());
    fs::set_permissions(&config, fs::Permissions::from_mode(0o600)).unwrap();
    symlink(&config, root.join("link.yaml")).unwrap();
    assert!(open_private(&root.join("link.yaml")).is_err());
    fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(open_private(&config).is_err());
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let parent = tempfile::tempdir().unwrap();
    let alias = parent.path().canonicalize().unwrap().join("alias");
    symlink(&root, &alias).unwrap();
    assert!(open_private(&alias.join("config.yaml")).is_err());
}

#[test]
fn opened_file_is_not_replaced_by_a_later_path_swap() {
    use std::io::Read;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let path = root.join("config.yaml");
    fs::write(&path, "original").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    let (mut file, _) = open_private(&path).unwrap();
    fs::rename(&path, root.join("old.yaml")).unwrap();
    fs::write(&path, "replacement").unwrap();
    let mut text = String::new();
    file.read_to_string(&mut text).unwrap();
    assert_eq!(text, "original");
}

#[test]
fn private_load_resolves_missing_optional_paths_without_reading_credentials() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let config = root.join("config.yaml");
    let text = format!(
        "schema_version: 1\npaths: {{entries_root: '{}', runtime_root: '{}'}}\nsources:\n  - {{alias: missing, type: narihito, root: source}}\nagents:\n  profiles:\n    - {{alias: missing, type: codex, profile_home: profiles/missing, executable: bin/missing}}\n",
        root.join("workspace").display(),
        root.join("runtime").display()
    );
    fs::write(&config, text).unwrap();
    fs::set_permissions(&config, fs::Permissions::from_mode(0o600)).unwrap();
    let c = load(&config).unwrap();
    assert_eq!(c.sources[0].root, root.join("source"));
    assert!(!root.join("workspace").exists());
    assert!(!root.join("runtime").exists());
    match &c.agents.profiles[0] {
        Profile::Codex {
            profile_home,
            executable,
            ..
        } => {
            assert_eq!(profile_home, &root.join("profiles/missing"));
            assert_eq!(executable.as_ref().unwrap(), &root.join("bin/missing"));
        }
        _ => panic!("expected CLI profile"),
    }
}

#[test]
fn executable_selection_uses_launch_path_and_override_without_execution() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    fs::create_dir(root.join("bin")).unwrap();
    for name in ["codex", "claude", "qwen", "override"] {
        let path = root.join("bin").join(name);
        fs::write(&path, "#!/bin/sh\nexit 71\n").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    for name in ["codex", "claude", "qwen"] {
        assert_eq!(
            select_executable(None, name, std::ffi::OsStr::new("bin"), &root),
            Some(root.join("bin").join(name))
        );
    }
    assert_eq!(
        select_executable(
            Some(&root.join("bin/override")),
            "codex",
            std::ffi::OsStr::new("bin"),
            &root
        ),
        Some(root.join("bin/override"))
    );
    assert!(
        select_executable(
            Some(&root.join("missing")),
            "codex",
            std::ffi::OsStr::new("bin"),
            &root
        )
        .is_none()
    );
    fs::set_permissions(root.join("bin/codex"), fs::Permissions::from_mode(0o600)).unwrap();
    assert!(select_executable(None, "codex", std::ffi::OsStr::new("bin"), &root).is_none());
    assert!(select_executable(None, "unknown", std::ffi::OsStr::new("bin"), &root).is_none());
}

#[test]
fn optional_availability_stays_unverified_and_never_reads_profile_files() {
    use diary_application::UnavailableReason as Reason;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    fs::create_dir(root.join("profile")).unwrap();
    let yaml = format!(
        "schema_version: 1\npaths: {{entries_root: '{}', runtime_root: '{}'}}\nsources:\n  - {{alias: missing, type: narihito, root: missing}}\nagents:\n  profiles:\n    - {{alias: cli, type: codex, profile_home: profile}}\n    - {{alias: missing, type: claude, profile_home: absent}}\n    - {{alias: native, type: apple-foundation-models}}\n",
        root.join("workspace").display(),
        root.join("runtime").display()
    );
    let mut c = parse(yaml.as_bytes()).unwrap();
    c.resolve_paths(&root.join("config.yaml")).unwrap();
    let summary = c.observe_startup(std::ffi::OsStr::new("missing-bin"), &root);
    assert!(matches!(
        summary.sources[0].reason,
        Some(Reason::SourceRootMissing)
    ));
    assert!(matches!(
        summary.profiles[0].reason,
        Some(Reason::ExecutableMissing)
    ));
    assert!(matches!(
        summary.profiles[1].reason,
        Some(Reason::ProfileHomeMissing)
    ));
    assert!(matches!(
        summary.profiles[2].reason,
        Some(Reason::NativeAdapterUnverified)
    ));
}

#[test]
fn readable_source_roots_are_available_for_both_adapters_without_opt_in() {
    use diary_application::UnavailableReason as Reason;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    fs::create_dir(root.join("ayumi")).unwrap();
    fs::create_dir(root.join("mobile")).unwrap();
    let yaml = format!(
        "schema_version: 1\npaths: {{entries_root: '{}', runtime_root: '{}'}}\nsources:\n  - {{alias: ayumi, type: ayumi, root: ayumi}}\n  - {{alias: mobile, type: narihito, root: mobile}}\n  - {{alias: gone, type: ayumi, root: gone}}\n",
        root.join("workspace").display(),
        root.join("runtime").display()
    );
    let mut c = parse(yaml.as_bytes()).unwrap();
    c.resolve_paths(&root.join("config.yaml")).unwrap();
    let summary = c.observe_startup(std::ffi::OsStr::new("missing-bin"), &root);
    assert!(summary.sources[0].reason.is_none(), "readable ayumi root");
    assert!(
        summary.sources[1].reason.is_none(),
        "readable narihito root"
    );
    assert!(matches!(
        summary.sources[2].reason,
        Some(Reason::SourceRootMissing)
    ));
}

#[test]
fn specified_null_and_empty_paths_are_rejected_and_missing_parent_dotdot_normalizes() {
    for extra in [
        "notion: null\n",
        "sources: null\n",
        "agents: {profiles: null}\n",
        "paths: {entries_root: /workspace, runtime_root: /runtime, icloud_entries_root: null}\n",
    ] {
        assert!(parse(format!("{MINIMAL}{extra}").as_bytes()).is_err());
    }
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let mut c =
        parse(format!("{MINIMAL}sources: [{{alias: x, type: ayumi, root: ''}}]\n").as_bytes())
            .unwrap();
    assert_eq!(
        c.resolve_paths(&root.join("config.yaml")),
        Err(ConfigError::Path)
    );
    assert_eq!(
        canonical_proposed(&root.join("missing/../new")).unwrap(),
        root.join("new")
    );
}

#[test]
fn profile_alias_namespace_and_entire_optional_schema_are_validated() {
    let apple = "{alias: local, type: apple-foundation-models}";
    assert_eq!(
        error(&format!(
            "{MINIMAL}agents:\n  profiles: [{apple}, {apple}]\n"
        )),
        ConfigError::Alias
    );
    assert_eq!(
        error(&format!(
            "{MINIMAL}agents:\n  profiles:\n{}",
            "    - {alias: local, type: apple-foundation-models}\n".repeat(65)
        )),
        ConfigError::Limit
    );
    for alias in ["", "contains space", "é", &"x".repeat(65)] {
        assert_eq!(
            error(&format!(
                "{MINIMAL}agents:\n  profiles: [{{alias: '{alias}', type: apple-foundation-models}}]\n"
            )),
            ConfigError::Alias
        );
    }
    assert!(parse(format!("{MINIMAL}sources: [{{alias: local, type: ayumi, root: source}}]\nagents:\n  profiles: [{apple}, {{alias: Local, type: apple-foundation-models}}]\n").as_bytes()).is_ok());
    for extra in [
        "notion: {root_page_id: page, token: ''}\n",
        "notion: {root_page_id: page, token: invented, extra: true}\n",
        "server: {port: 8781, extra: true}\n",
        "agents: {profiles: [], extra: true}\n",
        "sources: [{alias: local, type: unknown, root: source}]\n",
    ] {
        assert!(parse(format!("{MINIMAL}{extra}").as_bytes()).is_err());
    }
}

#[test]
fn denied_optional_folders_do_not_abort_schema_or_observation() {
    use diary_application::UnavailableReason as Reason;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let denied = root.join("denied");
    let denied_profile = root.join("denied-profile");
    fs::create_dir(&denied_profile).unwrap();
    fs::set_permissions(&denied_profile, fs::Permissions::from_mode(0o0)).unwrap();
    fs::create_dir(&denied).unwrap();
    fs::set_permissions(&denied, fs::Permissions::from_mode(0o0)).unwrap();
    let yaml = format!(
        "{MINIMAL}sources: [{{alias: denied, type: ayumi, root: '{}'}}]\nagents:\n  profiles: [{{alias: denied, type: codex, profile_home: '{}'}}]\n",
        denied.display(),
        denied_profile.display()
    );
    let mut c = parse(yaml.as_bytes()).unwrap();
    c.resolve_paths(&root.join("config.yaml")).unwrap();
    let summary = c.observe_startup(std::ffi::OsStr::new(""), &root);
    fs::set_permissions(&denied, fs::Permissions::from_mode(0o700)).unwrap();
    fs::set_permissions(&denied_profile, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(matches!(
        summary.sources[0].reason,
        Some(Reason::SourceRootDenied)
    ));
    assert!(matches!(
        summary.profiles[0].reason,
        Some(Reason::ProfileHomeDenied)
    ));
}

#[test]
fn every_root_pair_rejects_equality_and_nesting_in_either_direction() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let config_file = root.join("config.yaml");
    let p = |name: &str| root.join(name).display().to_string();
    // (entries, runtime, icloud, sources, expect overlap)
    type Case<'a> = (&'a str, &'a str, Option<&'a str>, Vec<&'a str>, bool);
    let cases: [Case; 7] = [
        ("e", "r", None, vec!["s"], false),
        ("e", "r", None, vec!["e"], true),
        ("e", "r", None, vec!["e/nested"], true),
        ("a/b", "a", None, vec![], true),
        ("e", "r", Some("r"), vec![], true),
        ("e", "r", None, vec!["s", "s/inner"], true),
        (".", "r", None, vec![], true), // the config file lives inside the entries root
    ];
    for (entries, runtime, icloud, sources, overlap) in cases {
        let mut yaml = format!(
            "schema_version: 1\npaths: {{entries_root: '{}', runtime_root: '{}'",
            p(entries),
            p(runtime)
        );
        if let Some(i) = icloud {
            yaml.push_str(&format!(", icloud_entries_root: '{}'", p(i)));
        }
        yaml.push_str(if sources.is_empty() {
            "}\nsources: []\n"
        } else {
            "}\nsources:\n"
        });
        for (n, s) in sources.iter().enumerate() {
            yaml.push_str(&format!(
                "  - {{alias: s{n}, type: ayumi, root: '{}'}}\n",
                p(s)
            ));
        }
        let mut c = parse(yaml.as_bytes()).unwrap();
        let result = c.resolve_paths(&config_file);
        if overlap {
            assert_eq!(
                result,
                Err(ConfigError::Overlap),
                "{entries} {runtime} {icloud:?} {sources:?}"
            );
        } else {
            assert_eq!(result, Ok(()));
        }
    }
}

#[test]
fn startup_selection_and_observation_never_execute_cli_files() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let marker = root.join("executed");
    fs::create_dir_all(root.join("bin")).unwrap();
    fs::create_dir(root.join("profile")).unwrap();
    for name in ["codex", "override"] {
        let path = root.join("bin").join(name);
        fs::write(&path, format!("#!/bin/sh\ntouch '{}'\n", marker.display())).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    let yaml = format!(
        "schema_version: 1\npaths: {{entries_root: '{}', runtime_root: '{}'}}\nagents:\n  profiles:\n    - {{alias: a, type: codex, profile_home: profile}}\n    - {{alias: b, type: codex, profile_home: profile, executable: bin/override}}\n",
        root.join("workspace").display(),
        root.join("runtime").display()
    );
    let mut c = parse(yaml.as_bytes()).unwrap();
    c.resolve_paths(&root.join("config.yaml")).unwrap();
    let _ = c.observe_startup(std::ffi::OsStr::new("bin"), &root);
    let _ = select_executable(None, "codex", std::ffi::OsStr::new("bin"), &root);
    assert!(!marker.exists(), "startup must not run CLI executables");
}

mod reload_tests {
    use super::*;
    use diary_application::{ConfigReloader, ReloadOutcome};

    struct Fx {
        _temp: tempfile::TempDir,
        file: PathBuf,
        root: PathBuf,
        handle: ConfigHandle,
    }
    fn write(file: &Path, text: &str) {
        let _ = fs::remove_file(file);
        fs::write(file, text).unwrap();
        fs::set_permissions(file, fs::Permissions::from_mode(0o600)).unwrap();
    }
    fn text(root: &Path, port: u16, extra: &str) -> String {
        format!(
            "schema_version: 1\npaths: {{entries_root: '{r}/workspace', runtime_root: '{r}/runtime'}}\nserver: {{port: {port}}}\n{extra}",
            r = root.display()
        )
    }
    fn fx() -> Fx {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let file = root.join("config.yaml");
        write(&file, &text(&root, 8781, ""));
        let config = load(&file).unwrap();
        let handle = ConfigHandle::new(&file, &config, "/usr/bin".into(), root.clone());
        Fx {
            _temp: temp,
            file,
            root,
            handle,
        }
    }

    #[test]
    fn unchanged_and_profile_changes_apply_and_a_busy_profile_change_waits() {
        let f = fx();
        assert!(matches!(f.handle.reload(false), ReloadOutcome::Applied(_)));
        let with_profile = text(
            &f.root,
            8781,
            "agents:\n  profiles:\n    - {alias: local, type: apple-foundation-models}\n",
        );
        write(&f.file, &with_profile);
        assert!(matches!(f.handle.reload(true), ReloadOutcome::Busy));
        // Busy changed nothing: the same file applies once idle, then is the baseline.
        let ReloadOutcome::Applied(summary) = f.handle.reload(false) else {
            panic!("expected applied");
        };
        assert_eq!(summary.profiles.len(), 1);
        assert!(matches!(f.handle.reload(true), ReloadOutcome::Applied(_)));
    }

    #[test]
    fn restart_fields_and_invalid_files_keep_the_last_config_with_redacted_issues() {
        let f = fx();
        write(&f.file, &text(&f.root, 8790, ""));
        let ReloadOutcome::RestartRequired(issues) = f.handle.reload(false) else {
            panic!("expected restart");
        };
        assert_eq!(issues[0].path, "server");
        write(&f.file, "schema_version: 1\nsecret_key: hunter2\n");
        let ReloadOutcome::Rejected(issues) = f.handle.reload(false) else {
            panic!("expected rejection");
        };
        assert_eq!((issues[0].path, issues[0].code), ("config", "schema"));
        assert!(!format!("{issues:?}").contains("hunter2"));
        // The original file still applies: nothing was adopted from the failures.
        write(&f.file, &text(&f.root, 8781, ""));
        assert!(matches!(f.handle.reload(false), ReloadOutcome::Applied(_)));
        fs::set_permissions(&f.file, fs::Permissions::from_mode(0o644)).unwrap();
        let ReloadOutcome::Rejected(issues) = f.handle.reload(false) else {
            panic!("expected rejection");
        };
        assert_eq!(issues[0].code, "permissions");
    }
}

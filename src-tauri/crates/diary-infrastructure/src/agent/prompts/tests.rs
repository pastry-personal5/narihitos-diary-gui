use super::*;
use diary_application::agent::CapabilityReason::*;

const GOOD: &str =
    "---\nprovider: codex\nmodel: m\nreasoning_effort: e\n---\n\nHi {{locale}} {{data}}\n";
fn load(kind: AgentKind, text: &str) -> Result<ParsedPromptAsset, &'static str> {
    parse(kind, CHAT, text)
}
fn en() -> Locale {
    Locale::parse("en").unwrap()
}

#[test]
fn front_matter_is_parsed() {
    let p = load(AgentKind::Codex, GOOD).unwrap();
    assert_eq!(
        (
            p.settings.model.as_str(),
            p.settings.reasoning_effort.as_str()
        ),
        ("m", "e")
    );
    assert_eq!(p.template.render(&en(), Some("D")), "Hi en D"); // trailing newline dropped
    for bad in [
        "no front matter {{locale}}{{data}}",
        "---\nprovider: codex\nmodel: m\n---\n{{locale}}{{data}}",
        "---\nprovider: codex\nprovider: codex\nmodel: m\nreasoning_effort: e\n---\n{{locale}}{{data}}",
        "---\nprovider: codex\nmodel: m\nreasoning_effort: e\nextra: 1\n---\n{{locale}}{{data}}",
        "---\nprovider: codex\nmodel: \nreasoning_effort: e\n---\n{{locale}}{{data}}",
    ] {
        assert!(load(AgentKind::Codex, bad).is_err(), "{bad}");
    }
}

#[test]
fn provider_must_match_directory() {
    assert!(load(AgentKind::Claude, GOOD).is_err());
}

#[test]
fn apple_requires_default_and_not_applicable() {
    let apple = |m: &str, e: &str| {
        format!(
            "---\nprovider: apple-foundation-models\nmodel: {m}\nreasoning_effort: {e}\n---\n{{{{locale}}}}{{{{data}}}}"
        )
    };
    let k = AgentKind::AppleFoundationModels;
    assert!(load(k, &apple("default", "not-applicable")).is_ok());
    assert!(load(k, &apple("gpt", "not-applicable")).is_err());
    assert!(load(k, &apple("default", "medium")).is_err());
}

#[test]
fn cli_model_and_effort_pass_through_unchanged() {
    let t = GOOD
        .replace("model: m", "model: any-name 1")
        .replace("effort: e", "effort: whatever");
    let p = load(AgentKind::Codex, &t).unwrap();
    assert_eq!(
        (
            p.settings.model.as_str(),
            p.settings.reasoning_effort.as_str()
        ),
        ("any-name 1", "whatever")
    );
}

#[test]
fn unknown_placeholder_is_malformed() {
    for body in [
        "{{style}} {{locale}} {{data}}",
        "{{ {{locale}} {{data}}",
        "{{locale}} {{data}} {{",
    ] {
        let t = format!("---\nprovider: codex\nmodel: m\nreasoning_effort: e\n---\n{body}");
        assert!(load(AgentKind::Codex, &t).is_err(), "{body}");
    }
}

#[test]
fn required_placeholders_must_appear_once() {
    let t = |b: &str| format!("---\nprovider: codex\nmodel: m\nreasoning_effort: e\n---\n{b}");
    assert!(load(AgentKind::Codex, &t("{{locale}}")).is_err()); // no data
    assert!(load(AgentKind::Codex, &t("{{data}}")).is_err()); // no locale
    assert!(load(AgentKind::Codex, &t("{{locale}}{{data}}{{data}}")).is_err());
    assert!(load(AgentKind::Codex, &t("{{locale}}{{locale}}{{data}}")).is_ok());
    assert!(parse(AgentKind::Codex, SESSION, &t("{{locale}} x")).is_ok());
    assert!(parse(AgentKind::Codex, SESSION, &t("{{locale}}{{data}}")).is_err());
}

#[test]
fn diary_text_with_placeholders_is_not_expanded() {
    let p = load(AgentKind::Codex, GOOD).unwrap();
    assert_eq!(
        p.template.render(&en(), Some("{{data}} {{locale}}")),
        "Hi en {{data}} {{locale}}"
    );
}

#[test]
fn missing_asset_names_the_action() {
    let a = EmbeddedPromptCatalog::from_table(&[]);
    let e = a.get(AgentKind::Codex, CHAT).err().unwrap();
    assert_eq!(
        (e.kind, e.name, e.reason),
        (AgentKind::Codex, CHAT, AssetReason::Missing)
    );
}

#[test]
fn malformed_asset_disables_only_its_action() {
    let a = EmbeddedPromptCatalog::from_table(&[
        (AgentKind::Codex, CHAT, "garbage"),
        (AgentKind::Codex, GENERATE, GOOD),
    ]);
    assert_eq!(
        a.capability(AgentKind::Codex, PromptAction::Chat, true),
        Capability::Unavailable(AssetsInvalid)
    );
    assert_eq!(
        a.capability(AgentKind::Codex, PromptAction::GenerateCandidate, true),
        Capability::Ready
    );
}

#[test]
fn no_fallback_to_another_agent_type() {
    let a = EmbeddedPromptCatalog::from_table(&[(AgentKind::Codex, CHAT, GOOD)]);
    assert_eq!(
        a.get(AgentKind::Claude, CHAT).err().unwrap().reason,
        AssetReason::Missing
    );
}

#[test]
fn capabilities_follow_assets_and_adapter() {
    let a = EmbeddedPromptCatalog::embedded();
    for action in [
        PromptAction::Chat,
        PromptAction::GenerateCandidate,
        PromptAction::Illustrated,
    ] {
        for k in [
            AgentKind::Codex,
            AgentKind::Claude,
            AgentKind::Qwen,
            AgentKind::AppleFoundationModels,
        ] {
            assert_eq!(
                a.capability(k, action, false),
                Capability::Unavailable(AdapterUnimplemented)
            );
            // Ready is reachable only with a fake registration.
            assert_eq!(a.capability(k, action, true), Capability::Ready);
        }
    }
}

#[test]
fn embedded_table_equals_disk_and_registers_every_file() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../assets/prompts");
    let mut disk = Vec::new();
    for d in std::fs::read_dir(root)
        .unwrap()
        .flatten()
        .filter(|d| d.path().is_dir())
    {
        for f in std::fs::read_dir(d.path()).unwrap().flatten() {
            if f.file_name().to_string_lossy().starts_with('.') {
                continue; // Finder's .DS_Store is not an asset
            }
            let name = f.path().file_stem().unwrap().to_str().unwrap().to_owned();
            disk.push((
                d.file_name().into_string().unwrap(),
                name,
                std::fs::read_to_string(f.path()).unwrap(),
            ));
        }
    }
    let mut table: Vec<_> = EMBEDDED
        .iter()
        .chain(CLIENT_TASKS.iter())
        .chain(ILLUSTRATED_ASSETS.iter())
        .map(|(k, n, t)| (k.asset_dir().to_owned(), n.to_string(), t.to_string()))
        .collect();
    disk.sort();
    table.sort();
    assert_eq!(table, disk);
}

#[test]
fn paragraph_task_assets_are_registered_and_valid_for_every_provider() {
    let catalog = EmbeddedPromptCatalog::from_table(CLIENT_TASKS);
    for kind in [
        AgentKind::Codex,
        AgentKind::Claude,
        AgentKind::Qwen,
        AgentKind::AppleFoundationModels,
    ] {
        let asset = catalog.get(kind, PARAGRAPH_REVIEW).unwrap();
        assert!(
            asset
                .template
                .render(&en(), Some("quoted paragraph"))
                .contains("quoted paragraph")
        );
    }
}

fn real(kind: AgentKind, name: &'static str) -> ParsedPromptAsset {
    EmbeddedPromptCatalog::embedded()
        .get(kind, name)
        .unwrap()
        .clone()
}

#[test]
fn generate_asset_matches_schema_and_packet() {
    for k in [
        AgentKind::Codex,
        AgentKind::Claude,
        AgentKind::Qwen,
        AgentKind::AppleFoundationModels,
    ] {
        let b = real(k, GENERATE).template.body;
        for want in [
            "draft_markdown",
            "claim_support",
            "snippet_ids",
            "claim",
            "warnings",
            "snippets",
            "target_date",
            "civil_date",
            "instant_utc",
            "snippet-1",
        ] {
            assert!(b.contains(want), "{k:?} {want}");
        }
        for gone in [
            "reviewed_memory",
            "reflection_ranges",
            "prior_draft",
            "language",
        ] {
            assert!(!b.contains(gone), "{k:?} {gone}");
        }
    }
    let c = real(AgentKind::Codex, GENERATE);
    assert_eq!(
        (
            c.settings.model.as_str(),
            c.settings.reasoning_effort.as_str()
        ),
        ("gpt-5.6-luna", "medium")
    );
    let c = real(AgentKind::Claude, GENERATE);
    assert_eq!(
        (
            c.settings.model.as_str(),
            c.settings.reasoning_effort.as_str()
        ),
        ("claude-sonnet-4-6", "low")
    );
    let c = real(AgentKind::Qwen, GENERATE);
    // Qwen's --json-schema errors on plain-text answers; the asset must name its tool.
    assert!(c.template.body.contains("`structured_output`"));
    assert_eq!(
        (
            c.settings.model.as_str(),
            c.settings.reasoning_effort.as_str()
        ),
        ("qwen3.6:35b-a3b", "not-applicable")
    );
}

#[test]
fn generate_asset_names_all_five_styles() {
    for k in [
        AgentKind::Codex,
        AgentKind::Claude,
        AgentKind::Qwen,
        AgentKind::AppleFoundationModels,
    ] {
        let b = real(k, GENERATE).template.body;
        for s in diary_application::agent::Style::ALL {
            assert!(b.contains(&format!("`{}`", s.id())), "{k:?} {}", s.id());
        }
    }
}

#[test]
fn chat_asset_matches_packet() {
    for k in [
        AgentKind::Codex,
        AgentKind::Claude,
        AgentKind::Qwen,
        AgentKind::AppleFoundationModels,
    ] {
        let b = real(k, CHAT).template.body;
        for want in ["document", "transcript", "user_message", "unsaved", "kind"] {
            assert!(b.contains(want), "{k:?} {want}");
        }
        for gone in ["reviewed_memory", "reflection_ranges", "prior_draft"] {
            assert!(!b.contains(gone));
        }
    }
}

#[test]
fn illustrated_assets_render_captured_context_and_locale_for_all_providers() {
    let c = EmbeddedPromptCatalog::embedded();
    for kind in [
        AgentKind::Codex,
        AgentKind::Claude,
        AgentKind::Qwen,
        AgentKind::AppleFoundationModels,
    ] {
        let request = AgentRequest::new(
            ProfileHandle {
                alias: "test".into(),
                kind,
            },
            Locale::parse("ko-KR").unwrap(),
            Mode::Illustrated(diary_application::illustrated::Packet {
                body: "PRIVATE {{data}}".into(),
                date: diary_domain::authentic::DiaryDate::parse("2026-11-13").unwrap(),
                settings: diary_application::illustrated::PromptSettings {
                    trigger: "tag".into(),
                    directives: vec!["photo".into()],
                },
                max_photos: 4,
            }),
        );
        let p = c
            .resolve(PromptSelection::for_request(&request))
            .unwrap()
            .prepare(request)
            .unwrap();
        assert!(p.task().contains("ko-KR"));
        assert!(p.task().contains("PRIVATE {{data}}"));
        assert!(p.task().contains("prompt_directives"));
        assert!(p.task().contains("tag"));
    }
}

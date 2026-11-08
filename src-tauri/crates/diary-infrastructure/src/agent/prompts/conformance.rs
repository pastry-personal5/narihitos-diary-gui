use super::*;
use diary_domain::authentic::DiaryDate;
use sha2::{Digest, Sha256};

fn request(provider: AgentKind, action: PromptAction) -> AgentRequest {
    let mode = match action {
        PromptAction::Chat => {
            Mode::Chat(ChatPacket::new(None, vec![], "{{locale}} PRIVATE".into()).unwrap())
        }
        PromptAction::GenerateCandidate => Mode::Candidate {
            index: 1,
            count: 1,
            packet: GenerationPacket::new(
                Style::Standard,
                DiaryDate::parse("2026-11-06").unwrap(),
                vec![SnippetItem {
                    id: "snippet-1".into(),
                    instant_utc: "2026-11-06T00:00:00Z".into(),
                    civil_date: "2026-11-06".into(),
                    offset: "+00:00".into(),
                    body: "{{data}} PRIVATE".into(),
                }],
            )
            .unwrap(),
        },
    };
    AgentRequest::new(
        ProfileHandle {
            alias: "fixture".into(),
            kind: provider,
        },
        Locale::parse("ko-KR").unwrap(),
        mode,
    )
}
fn source(kind: AgentKind, body: &str) -> String {
    format!(
        "---\nprovider: {}\nmodel: {}\nreasoning_effort: {}\n---\n{body}\n",
        kind.asset_dir(),
        if kind.spec().fixed_pins {
            "default"
        } else {
            "fixture"
        },
        if kind.spec().fixed_pins {
            "not-applicable"
        } else {
            "low"
        }
    )
}

#[test]
fn every_embedded_part_renders_identically_to_pre_m30_bytes() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/prompt-render-baseline.json"
    ))
    .unwrap();
    let entries = fixture["entries"].as_array().unwrap();
    let catalog = EmbeddedPromptCatalog::embedded();
    assert_eq!(entries.len(), EMBEDDED.len() * 2);
    for &(kind, name, _) in EMBEDDED {
        for locale in ["en-US", "ko-KR"] {
            let asset = format!("assets/prompts/{}/{name}.md", kind.asset_dir());
            let expected = entries
                .iter()
                .find(|entry| entry["asset"] == asset && entry["locale"] == locale)
                .unwrap();
            let rendered = catalog
                .get(kind, name)
                .unwrap()
                .template
                .render(&Locale::parse(locale).unwrap(), Some("fixture-data"));
            let hash = Sha256::digest(rendered.as_bytes())
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>();
            assert_eq!(
                hash,
                expected["sha256"].as_str().unwrap(),
                "{asset} {locale}"
            );
            assert_eq!(rendered.len() as u64, expected["bytes"].as_u64().unwrap());
        }
    }
}

#[test]
fn captured_definitions_remain_independent_across_revisions_and_metadata_changes() {
    let k = AgentKind::Codex;
    let first = source(k, "revision-one {{locale}} {{data}}");
    let second = source(k, "revision-two {{locale}} {{data}}");
    let old = EmbeddedPromptCatalog::from_table(&[(k, CHAT, &first)]);
    let new = EmbeddedPromptCatalog::from_table(&[(k, CHAT, &second)]);
    let selection = PromptSelection {
        provider: k,
        action: PromptAction::Chat,
    };
    let captured = old.resolve(selection).unwrap();
    let replaced = new.resolve(selection).unwrap();
    assert_eq!(captured.provenance().id, replaced.provenance().id);
    assert_ne!(
        captured.provenance().revision,
        replaced.provenance().revision
    );
    // The original catalog can disappear. Neither preparation nor execution needs it.
    drop(old);
    let running = captured.prepare(request(k, selection.action)).unwrap();
    let later = replaced.prepare(request(k, selection.action)).unwrap();
    assert!(running.task().starts_with("revision-one ko-KR"));
    assert!(later.task().starts_with("revision-two ko-KR"));
    assert!(running.task().contains("{{locale}} PRIVATE"));
    assert_eq!(running.provenance(), captured.provenance());
    let metadata = first.replace("reasoning_effort: low", "reasoning_effort: medium");
    let revision = EmbeddedPromptCatalog::from_table(&[(k, CHAT, &metadata)])
        .resolve(selection)
        .unwrap();
    assert_ne!(
        captured.provenance().revision,
        revision.provenance().revision
    );
    assert_eq!(revision.settings().reasoning_effort, "medium");
}

#[test]
fn apple_task_and_locale_session_are_one_captured_definition() {
    let k = AgentKind::AppleFoundationModels;
    let task = source(k, "task-one {{locale}} {{data}}");
    let session_one = source(k, "session-one {{locale}}");
    let session_two = source(k, "session-two {{locale}}");
    let selection = PromptSelection {
        provider: k,
        action: PromptAction::Chat,
    };
    let captured =
        EmbeddedPromptCatalog::from_table(&[(k, CHAT, &task), (k, SESSION, &session_one)])
            .resolve(selection)
            .unwrap();
    let later = EmbeddedPromptCatalog::from_table(&[(k, CHAT, &task), (k, SESSION, &session_two)])
        .resolve(selection)
        .unwrap();
    assert_ne!(captured.provenance().revision, later.provenance().revision);
    let prepared = captured.prepare(request(k, selection.action)).unwrap();
    assert_eq!(prepared.session(), Some("session-one ko-KR"));
    assert_eq!(prepared.settings().model, "default");
    assert_eq!(prepared.settings().reasoning_effort, "not-applicable");
    assert!(
        EmbeddedPromptCatalog::from_table(&[(k, CHAT, &task)])
            .resolve(selection)
            .is_err()
    );
    assert!(
        EmbeddedPromptCatalog::from_table(&[(k, CHAT, &task), (k, SESSION, "malformed")])
            .resolve(selection)
            .is_err()
    );
    assert!(
        captured
            .prepare(request(AgentKind::Codex, selection.action))
            .is_err()
    );
    assert!(
        captured
            .prepare(request(k, PromptAction::GenerateCandidate))
            .is_err()
    );
}

#[test]
fn oversized_data_and_templates_fail_without_truncation_or_fallback() {
    let catalog = EmbeddedPromptCatalog::embedded();
    let k = AgentKind::Codex;
    let mut oversized = request(k, PromptAction::Chat);
    oversized = AgentRequest::new(
        oversized.profile().clone(),
        oversized.locale().clone(),
        Mode::Chat(
            ChatPacket::new(
                Some(ChatDocument {
                    kind: DocumentKind::Authentic,
                    date: DiaryDate::parse("2026-11-06").unwrap(),
                    unsaved: true,
                    body: "\"".repeat(200 * 1024),
                }),
                vec![],
                "hello".into(),
            )
            .unwrap(),
        ),
    );
    let captured = catalog
        .resolve(PromptSelection::for_request(&oversized))
        .unwrap();
    assert_eq!(
        captured.prepare(oversized).err().unwrap().0,
        AgentErrorCode::Context
    );
    let big = source(k, &("x".repeat(64 * 1024) + " {{locale}} {{data}}"));
    assert!(
        EmbeddedPromptCatalog::from_table(&[(k, CHAT, &big)])
            .resolve(PromptSelection {
                provider: k,
                action: PromptAction::Chat
            })
            .is_err()
    );
    for id in ["", "../private", "USER DATA", &"x".repeat(97)] {
        assert!(PromptId::parse(id).is_err());
    }
    for revision in ["path.md", &"g".repeat(64), &"A".repeat(64)] {
        assert!(PromptRevision::parse(revision).is_err());
    }
}

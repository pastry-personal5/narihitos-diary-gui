use super::*;
use diary_domain::authentic::DiaryDate;

fn date() -> DiaryDate {
    DiaryDate::from_ymd(2026, 11, 6).unwrap()
}
fn snip(n: usize) -> SnippetItem {
    SnippetItem {
        id: format!("snippet-{n}"),
        instant_utc: "2026-11-06T01:00:00Z".into(),
        civil_date: "2026-11-06".into(),
        offset: "+09:00".into(),
        body: "x".into(),
    }
}

#[test]
fn profile_status_has_the_eight_states() {
    use ProfileStatus::*;
    let all = [
        Missing,
        UnsupportedVersion,
        Unavailable,
        AvailableAuthUnknown,
        Unauthenticated,
        Ready,
        Busy,
        Failed,
    ];
    assert_eq!(all.len(), 8);
}

#[test]
fn request_rejects_empty_selection() {
    assert!(GenerationPacket::new(Style::Standard, date(), vec![]).is_err());
    assert!(GenerationPacket::new(Style::Standard, date(), vec![snip(2)]).is_err());
    assert!(GenerationPacket::new(Style::Standard, date(), vec![snip(1), snip(1)]).is_err());
    assert!(GenerationPacket::new(Style::Standard, date(), vec![snip(1), snip(2)]).is_ok());
}

#[test]
fn request_enforces_message_and_transcript_bounds() {
    let ctx = |r: Result<ChatPacket, AgentError>| r.unwrap_err().0 == AgentErrorCode::Context;
    let t = |n: usize, len: usize| {
        (0..n)
            .map(|_| Turn {
                role: Role::User,
                text: "a".repeat(len),
            })
            .collect::<Vec<_>>()
    };
    assert!(ChatPacket::new(None, vec![], "a".repeat(MAX_USER_MESSAGE_BYTES)).is_ok());
    assert!(ctx(ChatPacket::new(
        None,
        vec![],
        "a".repeat(MAX_USER_MESSAGE_BYTES + 1)
    )));
    assert!(ChatPacket::new(None, t(100, 1), "m".into()).is_ok());
    assert!(ctx(ChatPacket::new(None, t(101, 1), "m".into())));
    assert!(ChatPacket::new(None, t(1, MAX_TRANSCRIPT_BYTES), "m".into()).is_ok());
    assert!(ctx(ChatPacket::new(
        None,
        t(1, MAX_TRANSCRIPT_BYTES + 1),
        "m".into()
    )));
}

#[test]
fn locale_parse_accepts_bcp47_shapes_and_rejects_braces_spaces() {
    for ok in ["en", "ja-JP", "zh-Hant-TW", "en-US-x-private"] {
        assert!(Locale::parse(ok).is_ok(), "{ok}");
    }
    let long = "a".repeat(36);
    for bad in [
        "",
        "e",
        "{{data}}",
        "en US",
        "en--US",
        "-en",
        "1a",
        "en-",
        "en-abcdefghi",
        &long,
    ] {
        assert!(Locale::parse(bad).is_err(), "{bad}");
    }
}

#[test]
fn error_codes_are_the_closed_allowlist() {
    let names: Vec<_> = AgentErrorCode::ALL.iter().map(|c| c.as_str()).collect();
    assert_eq!(
        names,
        [
            "busy",
            "cancelled",
            "timeout",
            "context",
            "invalid_output",
            "unavailable",
            "assets_unavailable",
            "unsupported_language",
            "refusal",
            "rate_limited",
            "failed"
        ]
    );
}

#[test]
fn error_display_carries_no_content() {
    assert_eq!(
        AgentError(AgentErrorCode::InvalidOutput).to_string(),
        "agent error: invalid_output"
    );
}

fn ok() -> Result<AgentOutput, AgentError> {
    Ok(AgentOutput::Reply("hi".into()))
}

#[test]
fn late_result_after_committed_cancel_is_rejected() {
    let run = Run::new();
    assert!(run.request_cancel());
    assert!(!run.request_cancel());
    assert_eq!(run.finish(ok()), Err(AgentErrorCode::Cancelled.into()));
}

#[test]
fn run_from_token_shares_the_cancel_commit() {
    let owner = Run::new();
    let adapter = Run::from(owner.token());
    assert!(adapter.finish(ok()).is_ok());
    owner.request_cancel();
    assert_eq!(adapter.finish(ok()), Err(AgentErrorCode::Cancelled.into()));
}

#[test]
fn late_delta_is_ignored() {
    let run = Run::new();
    assert!(run.accept(AgentEvent::Delta("a".into())).is_some());
    run.request_cancel();
    assert!(run.accept(AgentEvent::Delta("b".into())).is_none());
    assert!(run.accept(AgentEvent::Phase(Phase::Stopping)).is_none());
}

#[test]
fn success_cannot_race_cancel() {
    let run = Run::new();
    assert!(run.finish(ok()).is_ok());
    run.request_cancel();
    assert_eq!(run.finish(ok()), Err(AgentErrorCode::Cancelled.into()));
    assert_eq!(
        run.finish(Err(AgentErrorCode::Timeout.into())),
        Err(AgentErrorCode::Timeout.into())
    );
}

/// The API-visible reasons are one list shared with the TypeScript client.
#[test]
fn api_visible_reasons_match_the_shared_list() {
    let listed: Vec<String> = include_str!("../../../../../src/application/profile-reasons.json")
        .split('"')
        .skip(1)
        .step_by(2)
        .map(str::to_owned)
        .collect();
    let visible: Vec<&str> = Reason::ALL
        .iter()
        .filter(|r| r.is_api_visible())
        .map(|r| r.as_str())
        .collect();
    assert_eq!(listed, visible);
}

#[test]
fn kind_spec_covers_every_kind() {
    for k in AgentKind::ALL {
        let sp = k.spec();
        assert_eq!(sp.default_binary.is_some(), sp.needs_home, "{k:?}");
        assert_eq!(sp.session_asset, sp.fixed_pins, "{k:?}");
    }
    let dirs: std::collections::HashSet<_> = AgentKind::ALL.iter().map(|k| k.asset_dir()).collect();
    assert_eq!(dirs.len(), AgentKind::ALL.len());
}

#[test]
fn reason_ids_are_unique_snake_case() {
    let ids: std::collections::HashSet<_> = Reason::ALL.iter().map(|r| r.as_str()).collect();
    assert_eq!(ids.len(), Reason::ALL.len());
    for id in ids {
        assert!(
            id.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'),
            "{id}"
        );
    }
}

#[test]
fn fake_runner_honors_the_port() {
    struct Fake;
    impl AgentRunner for Fake {
        fn check(&self, _: &ProfileHandle) -> ProfileCheck {
            ProfileStatus::Ready.into()
        }
        fn run(
            &self,
            _: AgentRequest,
            _: &mut dyn EventSink,
            c: &CancelToken,
        ) -> Result<AgentOutput, AgentError> {
            if c.is_cancelled() {
                Err(AgentErrorCode::Cancelled.into())
            } else {
                ok()
            }
        }
    }
    struct Sink;
    impl EventSink for Sink {
        fn event(&mut self, _: AgentEvent) {}
    }
    let req = AgentRequest::new(
        ProfileHandle {
            alias: "a".into(),
            kind: AgentKind::Codex,
        },
        Locale::parse("en").unwrap(),
        Mode::Chat(ChatPacket::new(None, vec![], "hi".into()).unwrap()),
    );
    let run = Run::new();
    assert!(Fake.run(req, &mut Sink, &run.token()).is_ok());
}

#[test]
fn capability_precedence() {
    use AssetState::*;
    use CapabilityReason::*;
    assert_eq!(
        capability(&[Missing, Invalid], true),
        Capability::Unavailable(AssetsInvalid)
    );
    assert_eq!(
        capability(&[Valid, Missing], true),
        Capability::Unavailable(AssetsMissing)
    );
    assert_eq!(
        capability(&[Valid], false),
        Capability::Unavailable(AdapterUnimplemented)
    );
    assert_eq!(capability(&[Valid], true), Capability::Ready);
}

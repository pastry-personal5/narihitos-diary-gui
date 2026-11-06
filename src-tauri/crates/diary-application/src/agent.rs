//! Agent port: typed requests, closed events and errors, bounds, the cancel-epoch run guard and
//! the per-action capability model. Pure values only: no I/O, JSON, prompts or process control.
//! JSON encoding, output validation and prompt assets live in `diary-infrastructure::agent`.
use diary_domain::authentic::DiaryDate;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub const SCHEMA_VERSION: u32 = 1;
pub const MAX_PACKET_BYTES: usize = 256 * 1024;
pub const MAX_USER_MESSAGE_BYTES: usize = 64 * 1024;
pub const MAX_TURNS: usize = 100;
pub const MAX_TRANSCRIPT_BYTES: usize = 128 * 1024;
pub const MAX_TEXT_BYTES: usize = 1024 * 1024; // draft and chat reply
pub const MAX_RESULT_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_PROCESS_OUTPUT_BYTES: usize = 10 * 1024 * 1024; // consumed by P1-M17
pub const MAX_PROTOCOL_RECORD_BYTES: usize = 1024 * 1024; // consumed by P1-M17
pub const INACTIVITY_SECS: u64 = 120; // consumed by P1-M17
pub const WALL_SECS: u64 = 600; // consumed by P1-M17

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgentKind {
    Codex,
    Claude,
    Qwen,
    AppleFoundationModels,
}
impl AgentKind {
    pub const ALL: [Self; 4] = [
        Self::Codex,
        Self::Claude,
        Self::Qwen,
        Self::AppleFoundationModels,
    ];
    /// Per-kind facts, kept in one place.
    pub const fn spec(self) -> &'static KindSpec {
        match self {
            Self::Codex => &KindSpec {
                asset_dir: "codex",
                default_binary: Some("codex"),
                needs_home: true,
                session_asset: false,
                fixed_pins: false,
            },
            Self::Claude => &KindSpec {
                asset_dir: "claude",
                default_binary: Some("claude"),
                needs_home: true,
                session_asset: false,
                fixed_pins: false,
            },
            Self::Qwen => &KindSpec {
                asset_dir: "qwen",
                default_binary: Some("qwen"),
                needs_home: true,
                session_asset: false,
                fixed_pins: false,
            },
            Self::AppleFoundationModels => &KindSpec {
                asset_dir: "apple-foundation-models",
                default_binary: None,
                needs_home: false,
                session_asset: true,
                fixed_pins: true,
            },
        }
    }
    pub fn parse(v: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.asset_dir() == v)
    }
    /// Directory under `assets/prompts` and the config `type` name.
    pub fn asset_dir(self) -> &'static str {
        self.spec().asset_dir
    }
}

/// What differs between agent kinds without any behavior: names, requirements, pin policy.
#[derive(Debug, PartialEq, Eq)]
pub struct KindSpec {
    pub asset_dir: &'static str,
    /// Binary looked up on the launch `PATH` when no `executable` override is set.
    pub default_binary: Option<&'static str>,
    pub needs_home: bool,
    /// Whether a `session-instructions` asset is required next to the task prompts.
    pub session_asset: bool,
    /// Apple-style pins: model `default`, effort `not-applicable`.
    pub fixed_pins: bool,
}

/// The only profile reference a request carries: no command, path or executable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileHandle {
    pub alias: String,
    pub kind: AgentKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileStatus {
    Missing,
    UnsupportedVersion,
    Unavailable,
    AvailableAuthUnknown,
    Unauthenticated,
    Ready,
    Busy,
    Failed,
}

/// Why a profile or run is not usable. Closed, content-free; ids are stable snake_case.
/// Only the profile, version, gate, asset and Apple groups reach the API; run reasons are log-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    ExecutableMissing,
    ProfileHomeMissing,
    ProfileHomeNotDirectory,
    ProfileUnknown,
    KindMismatch,
    VersionUnsupported,
    VersionUnreadable,
    ProbeTimedOut,
    ProbeFailed,
    SpawnFailed,
    HostBusy,
    HostShuttingDown,
    LeaseEnded,
    CancelledBeforeStart,
    RunDirUnavailable,
    AssetsMissing,
    AssetsInvalid,
    PromptPinRejected,
    PacketTooLarge,
    OsUnsupported,
    DeviceIneligible,
    AppleIntelligenceDisabled,
    ModelNotReady,
    ModelUnavailableUnknown,
    LocaleUnsupported,
    NativeSessionBusy,
    NativeErrorUnmapped,
    HelperMissing,
    HelperUntrusted,
    HelperFailed,
    RecordTooLarge,
    OutputTooLarge,
    EscapedDescendants,
    ExitNonzero,
    ResultMissing,
    InitMismatch,
}
impl Reason {
    pub const ALL: [Self; 36] = [
        Self::ExecutableMissing,
        Self::ProfileHomeMissing,
        Self::ProfileHomeNotDirectory,
        Self::ProfileUnknown,
        Self::KindMismatch,
        Self::VersionUnsupported,
        Self::VersionUnreadable,
        Self::ProbeTimedOut,
        Self::ProbeFailed,
        Self::SpawnFailed,
        Self::HostBusy,
        Self::HostShuttingDown,
        Self::LeaseEnded,
        Self::CancelledBeforeStart,
        Self::RunDirUnavailable,
        Self::AssetsMissing,
        Self::AssetsInvalid,
        Self::PromptPinRejected,
        Self::PacketTooLarge,
        Self::OsUnsupported,
        Self::DeviceIneligible,
        Self::AppleIntelligenceDisabled,
        Self::ModelNotReady,
        Self::ModelUnavailableUnknown,
        Self::LocaleUnsupported,
        Self::NativeSessionBusy,
        Self::NativeErrorUnmapped,
        Self::HelperMissing,
        Self::HelperUntrusted,
        Self::HelperFailed,
        Self::RecordTooLarge,
        Self::OutputTooLarge,
        Self::EscapedDescendants,
        Self::ExitNonzero,
        Self::ResultMissing,
        Self::InitMismatch,
    ];
    pub fn parse(v: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|r| r.as_str() == v)
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ExecutableMissing => "executable_missing",
            Self::ProfileHomeMissing => "profile_home_missing",
            Self::ProfileHomeNotDirectory => "profile_home_not_directory",
            Self::ProfileUnknown => "profile_unknown",
            Self::KindMismatch => "kind_mismatch",
            Self::VersionUnsupported => "version_unsupported",
            Self::VersionUnreadable => "version_unreadable",
            Self::ProbeTimedOut => "probe_timed_out",
            Self::ProbeFailed => "probe_failed",
            Self::SpawnFailed => "spawn_failed",
            Self::HostBusy => "host_busy",
            Self::HostShuttingDown => "host_shutting_down",
            Self::LeaseEnded => "lease_ended",
            Self::CancelledBeforeStart => "cancelled_before_start",
            Self::RunDirUnavailable => "run_dir_unavailable",
            Self::AssetsMissing => "assets_missing",
            Self::AssetsInvalid => "assets_invalid",
            Self::PromptPinRejected => "prompt_pin_rejected",
            Self::PacketTooLarge => "packet_too_large",
            Self::OsUnsupported => "os_unsupported",
            Self::DeviceIneligible => "device_ineligible",
            Self::AppleIntelligenceDisabled => "apple_intelligence_disabled",
            Self::ModelNotReady => "model_not_ready",
            Self::ModelUnavailableUnknown => "model_unavailable_unknown",
            Self::LocaleUnsupported => "locale_unsupported",
            Self::NativeSessionBusy => "native_session_busy",
            Self::NativeErrorUnmapped => "native_error_unmapped",
            Self::HelperMissing => "helper_missing",
            Self::HelperUntrusted => "helper_untrusted",
            Self::HelperFailed => "helper_failed",
            Self::RecordTooLarge => "record_too_large",
            Self::OutputTooLarge => "output_too_large",
            Self::EscapedDescendants => "escaped_descendants",
            Self::ExitNonzero => "exit_nonzero",
            Self::ResultMissing => "result_missing",
            Self::InitMismatch => "init_mismatch",
        }
    }
    /// Reasons a status check can report to the API (the rest are log-only run reasons).
    pub fn is_api_visible(self) -> bool {
        !matches!(
            self,
            Self::RecordTooLarge
                | Self::OutputTooLarge
                | Self::EscapedDescendants
                | Self::ExitNonzero
                | Self::ResultMissing
                | Self::InitMismatch
                | Self::NativeErrorUnmapped
                | Self::CancelledBeforeStart
                | Self::LeaseEnded
                | Self::HostShuttingDown
                | Self::NativeSessionBusy
        )
    }
}

/// A status plus the closed reason behind it (`None` when ready or not yet attributed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProfileCheck {
    pub status: ProfileStatus,
    pub reason: Option<Reason>,
}
impl From<ProfileStatus> for ProfileCheck {
    fn from(status: ProfileStatus) -> Self {
        Self {
            status,
            reason: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentErrorCode {
    Busy,
    Cancelled,
    Timeout,
    Context,
    InvalidOutput,
    Unavailable,
    AssetsUnavailable,
    UnsupportedLanguage,
    Refusal,
    RateLimited,
    Failed,
}
impl AgentErrorCode {
    pub const ALL: [Self; 11] = [
        Self::Busy,
        Self::Cancelled,
        Self::Timeout,
        Self::Context,
        Self::InvalidOutput,
        Self::Unavailable,
        Self::AssetsUnavailable,
        Self::UnsupportedLanguage,
        Self::Refusal,
        Self::RateLimited,
        Self::Failed,
    ];
    pub fn parse(v: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.as_str() == v)
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Busy => "busy",
            Self::Cancelled => "cancelled",
            Self::Timeout => "timeout",
            Self::Context => "context",
            Self::InvalidOutput => "invalid_output",
            Self::Unavailable => "unavailable",
            Self::AssetsUnavailable => "assets_unavailable",
            Self::UnsupportedLanguage => "unsupported_language",
            Self::Refusal => "refusal",
            Self::RateLimited => "rate_limited",
            Self::Failed => "failed",
        }
    }
}

/// A code only: no field can hold a prompt, response, path or stderr.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AgentError(pub AgentErrorCode);
impl std::fmt::Display for AgentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "agent error: {}", self.0.as_str())
    }
}
impl std::error::Error for AgentError {}
impl From<AgentErrorCode> for AgentError {
    fn from(c: AgentErrorCode) -> Self {
        Self(c)
    }
}

/// BCP-47 shape only; never a registry lookup (shortcut: upgrade if an adapter rejects a valid tag).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Locale(String);
impl Locale {
    pub fn parse(s: &str) -> Result<Self, AgentError> {
        let ok = (2..=35).contains(&s.len())
            && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
            && s.split('-').enumerate().all(|(i, t)| {
                (1..=8).contains(&t.len()) && (i > 0 || t.bytes().all(|b| b.is_ascii_alphabetic()))
            });
        if ok {
            Ok(Self(s.to_owned()))
        } else {
            Err(AgentErrorCode::UnsupportedLanguage.into())
        }
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Standard,
    Concise,
    Reflective,
    Chronological,
    Thematic,
}
impl Style {
    /// Deterministic candidate order.
    pub const ALL: [Self; 5] = [
        Self::Standard,
        Self::Concise,
        Self::Reflective,
        Self::Chronological,
        Self::Thematic,
    ];
    pub fn id(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Concise => "concise",
            Self::Reflective => "reflective",
            Self::Chronological => "chronological",
            Self::Thematic => "thematic",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnippetItem {
    pub id: String,
    pub instant_utc: String,
    pub civil_date: String,
    pub offset: String,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerationPacket {
    style: Style,
    target_date: DiaryDate,
    snippets: Vec<SnippetItem>,
}
impl GenerationPacket {
    /// Rejects an empty selection and any ID that is not the sequential `snippet-N` label.
    pub fn new(
        style: Style,
        target_date: DiaryDate,
        snippets: Vec<SnippetItem>,
    ) -> Result<Self, AgentError> {
        let sequential = snippets
            .iter()
            .enumerate()
            .all(|(i, s)| s.id == format!("snippet-{}", i + 1));
        if snippets.is_empty() || !sequential {
            return Err(AgentErrorCode::Context.into());
        }
        Ok(Self {
            style,
            target_date,
            snippets,
        })
    }
    pub fn style(&self) -> Style {
        self.style
    }
    pub fn target_date(&self) -> DiaryDate {
        self.target_date
    }
    pub fn snippets(&self) -> &[SnippetItem] {
        &self.snippets
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentKind {
    Authentic,
    Source,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatDocument {
    pub kind: DocumentKind,
    pub date: DiaryDate,
    pub unsaved: bool,
    pub body: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Turn {
    pub role: Role,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatPacket {
    document: Option<ChatDocument>,
    transcript: Vec<Turn>,
    user_message: String,
}
impl ChatPacket {
    /// Over-bound input is `context`; nothing is ever truncated.
    pub fn new(
        document: Option<ChatDocument>,
        transcript: Vec<Turn>,
        user_message: String,
    ) -> Result<Self, AgentError> {
        let bytes: usize = transcript.iter().map(|t| t.text.len()).sum();
        if user_message.len() > MAX_USER_MESSAGE_BYTES
            || transcript.len() > MAX_TURNS
            || bytes > MAX_TRANSCRIPT_BYTES
        {
            return Err(AgentErrorCode::Context.into());
        }
        Ok(Self {
            document,
            transcript,
            user_message,
        })
    }
    pub fn document(&self) -> Option<&ChatDocument> {
        self.document.as_ref()
    }
    pub fn transcript(&self) -> &[Turn] {
        &self.transcript
    }
    pub fn user_message(&self) -> &str {
        &self.user_message
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    Chat(ChatPacket),
    Candidate {
        index: u8,
        count: u8,
        packet: GenerationPacket,
    },
}

/// Immutable after construction (private fields).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRequest {
    profile: ProfileHandle,
    locale: Locale,
    mode: Mode,
}
impl AgentRequest {
    pub fn new(profile: ProfileHandle, locale: Locale, mode: Mode) -> Self {
        Self {
            profile,
            locale,
            mode,
        }
    }
    pub fn schema_version(&self) -> u32 {
        SCHEMA_VERSION
    }
    pub fn profile(&self) -> &ProfileHandle {
        &self.profile
    }
    pub fn locale(&self) -> &Locale {
        &self.locale
    }
    pub fn mode(&self) -> &Mode {
        &self.mode
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    PreparingContext,
    StartingAgent,
    WaitingForOutput,
    ReceivingOutput,
    ValidatingOutput,
    BetweenCandidates,
    Stopping,
    Finished,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentEvent {
    Phase(Phase),
    /// Provisional text; never an `AgentOutput`.
    Delta(String),
    /// The provisional text so far, replacing everything streamed before: the source revised text
    /// it had already emitted, so appending the next delta would show something that was retracted.
    Replace(String),
}
pub trait EventSink {
    fn event(&mut self, event: AgentEvent);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claim {
    pub claim: String,
    pub snippet_ids: Vec<String>,
}
/// Constructed only by the infrastructure validator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub draft_markdown: String,
    pub claim_support: Vec<Claim>,
    pub warnings: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentOutput {
    Candidate(Candidate),
    Reply(String),
}

#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);
impl CancelToken {
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

pub trait AgentRunner {
    /// Non-inference check; adapters bound it to 5 seconds.
    fn check(&self, profile: &ProfileHandle) -> ProfileCheck;
    fn run(
        &self,
        request: AgentRequest,
        events: &mut dyn EventSink,
        cancel: &CancelToken,
    ) -> Result<AgentOutput, AgentError>;
}

/// One admitted run. Cancellation commits once; afterwards events drop and success cannot win.
#[derive(Debug, Default)]
pub struct Run {
    cancel: CancelToken,
}
/// Adapters receive a `CancelToken` through the port; this lets them route events and the result
/// through the same commit-once guard the job owner uses.
impl From<CancelToken> for Run {
    fn from(cancel: CancelToken) -> Self {
        Self { cancel }
    }
}
impl Run {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn token(&self) -> CancelToken {
        self.cancel.clone()
    }
    /// True only for the first call.
    pub fn request_cancel(&self) -> bool {
        !self.cancel.0.swap(true, Ordering::SeqCst)
    }
    pub fn accept(&self, event: AgentEvent) -> Option<AgentEvent> {
        (!self.cancel.is_cancelled()).then_some(event)
    }
    pub fn finish(
        &self,
        result: Result<AgentOutput, AgentError>,
    ) -> Result<AgentOutput, AgentError> {
        match result {
            Ok(_) if self.cancel.is_cancelled() => Err(AgentErrorCode::Cancelled.into()),
            r => r,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Chat,
    Generate,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetState {
    Valid,
    Missing,
    Invalid,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityReason {
    AssetsMissing,
    AssetsInvalid,
    AdapterUnimplemented,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    Ready,
    Unavailable(CapabilityReason),
}
/// Precedence: invalid assets, missing assets, no adapter, ready.
pub fn capability(assets: &[AssetState], adapter_registered: bool) -> Capability {
    use CapabilityReason::*;
    if assets.contains(&AssetState::Invalid) {
        Capability::Unavailable(AssetsInvalid)
    } else if assets.contains(&AssetState::Missing) {
        Capability::Unavailable(AssetsMissing)
    } else if !adapter_registered {
        Capability::Unavailable(AdapterUnimplemented)
    } else {
        Capability::Ready
    }
}

/// One profile as the status API reports it. `status` is `None` until a check has run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileView {
    pub alias: String,
    pub kind: AgentKind,
    pub status: Option<ProfileStatus>,
    /// Why the status is what it is; `None` when ready, unchecked or not attributed.
    pub reason: Option<Reason>,
    pub chat: Capability,
    pub generate: Capability,
}
/// Read-only profile status; implementations never start inference.
pub trait ProfileDirectory: Send + Sync {
    fn list(&self) -> Vec<ProfileView>;
    /// Runs the non-inference check; `None` for an unknown alias.
    fn check(&self, alias: &str) -> Option<ProfileView>;
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let listed: Vec<String> = include_str!("../../../../src/application/profile-reasons.json")
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
        let dirs: std::collections::HashSet<_> =
            AgentKind::ALL.iter().map(|k| k.asset_dir()).collect();
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
}

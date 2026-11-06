//! Typed diagnostic model. No free-form text can be attached to a `Record`:
//! every field is an enum, a number or a validated opaque ID.

use crate::agent::{AgentErrorCode, AgentKind, Phase, ProfileStatus, Reason, Style};

macro_rules! tag_enum {
    ($(#[$m:meta])* $name:ident { $($var:ident => $s:literal),+ $(,)? }) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum $name { $($var),+ }
        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$var),+];
            pub const fn as_str(self) -> &'static str {
                match self { $($name::$var => $s),+ }
            }
            pub fn parse(value: &str) -> Option<Self> {
                match value { $($s => Some($name::$var),)+ _ => None }
            }
        }
    };
}

tag_enum!(Level { Error => "error", Warn => "warn", Info => "info" });
tag_enum!(Role { Server => "server", Client => "client" });
tag_enum!(EventCode {
    ServerStarted => "server.started",
    ServerStopping => "server.stopping",
    ListenerBound => "server.listener_bound",
    DiscoveryPublished => "server.discovery_published",
    WorkspaceOpened => "workspace.opened",
    WorkspaceInspection => "workspace.inspection_required",
    WorkspaceRecovery => "workspace.recovery",
    OperationBlocked => "operation.blocked",
    AuthenticSaved => "authentic.saved",
    AuthenticCreated => "authentic.created",
    AuthenticConflict => "authentic.conflict",
    AuthenticRecovered => "authentic.recovered",
    AuthenticRebuilt => "authentic.rebuilt",
    AuthenticBlocked => "authentic.blocked",
    SettingsLoaded => "settings.loaded",
    SettingsSaved => "settings.saved",
    SettingsFailed => "settings.failed",
    SessionOpened => "session.opened",
    SessionClosed => "session.closed",
    ClientBusy => "session.client_busy",
    AuthRejected => "http.auth_rejected",
    JobFinished => "job.finished",
    AgentRun => "agent.run",
    AgentCheck => "agent.check",
    AgentBlocked => "agent.blocked",
    AgentHelper => "agent.helper",
    LogDropped => "log.dropped",
    UiBridgeError => "ui.bridge_error",
    UiRenderError => "ui.render_error",
    ClientEventRejected => "client.event_rejected",
});
tag_enum!(ErrorTag {
    StaleRevision => "stale_revision",
    ExternalChange => "external_change",
    Invalid => "invalid",
    Denied => "denied",
    Busy => "busy",
    Unavailable => "unavailable",
    Limit => "limit",
    Internal => "internal",
});
tag_enum!(StateTag {
    Saved => "saved",
    Stale => "stale",
    Conflict => "conflict",
    Missing => "missing",
    Unreadable => "unreadable",
    Pending => "pending",
});
tag_enum!(PathClass {
    EntriesRoot => "entries_root",
    RuntimeRoot => "runtime_root",
    SourceRoot => "source_root",
    OutsideRoot => "outside_root",
    Symlink => "symlink",
    Unknown => "unknown",
});
tag_enum!(ResourceKind {
    AuthenticEntry => "authentic_entry",
    SourceSnippet => "source_snippet",
    Audio => "audio",
    ExportTarget => "export_target",
    Config => "config",
    Job => "job",
    Operation => "operation",
});
tag_enum!(Component {
    Editor => "editor",
    Bridge => "bridge",
    Layout => "layout",
    Settings => "settings",
});

tag_enum!(ActionTag { Check => "check", Chat => "chat", Generate => "generate" });
tag_enum!(PhaseTag {
    PreparingContext => "preparing_context",
    StartingAgent => "starting_agent",
    WaitingForOutput => "waiting_for_output",
    ReceivingOutput => "receiving_output",
    ValidatingOutput => "validating_output",
    BetweenCandidates => "between_candidates",
    Stopping => "stopping",
    Finished => "finished",
});
tag_enum!(StopTag {
    Cancelled => "cancelled",
    Inactivity => "inactivity",
    Wall => "wall",
    RecordTooLarge => "record_too_large",
    OutputTooLarge => "output_too_large",
});
tag_enum!(ExitTag { Success => "success", Code => "code", Signal => "signal" });
tag_enum!(StatusTag {
    Missing => "missing",
    UnsupportedVersion => "unsupported_version",
    Unavailable => "unavailable",
    AvailableAuthUnknown => "available_auth_unknown",
    Unauthenticated => "unauthenticated",
    Ready => "ready",
    Busy => "busy",
    Failed => "failed",
});
impl From<Phase> for PhaseTag {
    fn from(p: Phase) -> Self {
        match p {
            Phase::PreparingContext => Self::PreparingContext,
            Phase::StartingAgent => Self::StartingAgent,
            Phase::WaitingForOutput => Self::WaitingForOutput,
            Phase::ReceivingOutput => Self::ReceivingOutput,
            Phase::ValidatingOutput => Self::ValidatingOutput,
            Phase::BetweenCandidates => Self::BetweenCandidates,
            Phase::Stopping => Self::Stopping,
            Phase::Finished => Self::Finished,
        }
    }
}
impl From<ProfileStatus> for StatusTag {
    fn from(s: ProfileStatus) -> Self {
        match s {
            ProfileStatus::Missing => Self::Missing,
            ProfileStatus::UnsupportedVersion => Self::UnsupportedVersion,
            ProfileStatus::Unavailable => Self::Unavailable,
            ProfileStatus::AvailableAuthUnknown => Self::AvailableAuthUnknown,
            ProfileStatus::Unauthenticated => Self::Unauthenticated,
            ProfileStatus::Ready => Self::Ready,
            ProfileStatus::Busy => Self::Busy,
            ProfileStatus::Failed => Self::Failed,
        }
    }
}

/// Agent diagnostics: every field is a closed tag or a number, never text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentDetail {
    pub kind: AgentKind,
    pub action: Option<ActionTag>,
    /// Last phase reached.
    pub phase: Option<PhaseTag>,
    pub code: Option<AgentErrorCode>,
    pub stop: Option<StopTag>,
    pub exit: Option<ExitTag>,
    pub reason: Option<Reason>,
    pub status: Option<StatusTag>,
    /// For checks: whether the reason differs from the previous check of that profile.
    pub changed: Option<bool>,
    pub escaped: Option<u32>,
    pub events: Option<u32>,
    pub input_bytes: Option<u32>,
    pub output_bytes: Option<u32>,
    pub exit_number: Option<i32>,
    pub candidate_index: Option<u32>,
    pub style: Option<Style>,
    pub requested: Option<u32>,
    pub ready: Option<u32>,
    pub failed: Option<u32>,
    pub cancelled: Option<u32>,
    pub not_started: Option<u32>,
}
impl AgentDetail {
    pub fn new(kind: AgentKind) -> Self {
        Self {
            kind,
            action: None,
            phase: None,
            code: None,
            stop: None,
            exit: None,
            reason: None,
            status: None,
            changed: None,
            escaped: None,
            events: None,
            input_bytes: None,
            output_bytes: None,
            exit_number: None,
            candidate_index: None,
            style: None,
            requested: None,
            ready: None,
            failed: None,
            cancelled: None,
            not_started: None,
        }
    }
}

impl EventCode {
    /// Only UI codes may be reported by the WebView.
    pub const fn webview_allowed(self) -> bool {
        matches!(self, Self::UiBridgeError | Self::UiRenderError)
    }
}

fn lower_hex(s: &str) -> bool {
    s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn valid_date(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return false;
    }
    let num = |r: std::ops::Range<usize>| -> Option<u32> {
        let t = &s[r];
        t.bytes()
            .all(|c| c.is_ascii_digit())
            .then(|| t.parse().ok())?
    };
    let (Some(y), Some(m), Some(d)) = (num(0..4), num(5..7), num(8..10)) else {
        return false;
    };
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let days = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    (1..=days).contains(&d)
}

pub fn valid_uuid(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 36
        && [8, 13, 18, 23].iter().all(|&i| b[i] == b'-')
        && s.bytes()
            .enumerate()
            .all(|(i, c)| [8, 13, 18, 23].contains(&i) || matches!(c, b'0'..=b'9' | b'a'..=b'f'))
}

/// A UUID, a `YYYY-MM-DD` date, or 16–64 lowercase hex characters; nothing else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpaqueId(String);
impl OpaqueId {
    pub fn parse(value: &str) -> Option<Self> {
        let ok = valid_uuid(value)
            || valid_date(value)
            || ((16..=64).contains(&value.len()) && lower_hex(value));
        ok.then(|| Self(value.to_owned()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// 32 lowercase hex characters (a UUID without hyphens).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceId(String);
impl TraceId {
    pub fn parse(value: &str) -> Option<Self> {
        (value.len() == 32 && lower_hex(value)).then(|| Self(value.to_owned()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Redacted description of a resource: identity, size and state, never content or path text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceDetail {
    pub kind: ResourceKind,
    pub id: OpaqueId,
    pub revision: Option<u64>,
    pub bytes: Option<u32>,
    pub lines: Option<u32>,
    pub fingerprint: Option<[u8; 8]>,
    pub state: Option<StateTag>,
    pub path: Option<PathClass>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub code: EventCode,
    pub level: Level,
    pub trace: Option<TraceId>,
    pub session: Option<OpaqueId>,
    pub component: Option<Component>,
    pub operation: Option<OpaqueId>,
    pub job: Option<OpaqueId>,
    pub resource: Option<ResourceDetail>,
    pub error: Option<ErrorTag>,
    pub count: Option<u32>,
    pub agent: Option<AgentDetail>,
}

impl Record {
    pub fn new(code: EventCode, level: Level) -> Self {
        Self {
            code,
            level,
            trace: None,
            session: None,
            component: None,
            operation: None,
            job: None,
            resource: None,
            error: None,
            count: None,
            agent: None,
        }
    }
    pub fn trace(mut self, v: TraceId) -> Self {
        self.trace = Some(v);
        self
    }
    pub fn session(mut self, v: OpaqueId) -> Self {
        self.session = Some(v);
        self
    }
    pub fn component(mut self, v: Component) -> Self {
        self.component = Some(v);
        self
    }
    pub fn operation(mut self, v: OpaqueId) -> Self {
        self.operation = Some(v);
        self
    }
    pub fn job(mut self, v: OpaqueId) -> Self {
        self.job = Some(v);
        self
    }
    pub fn resource(mut self, v: ResourceDetail) -> Self {
        self.resource = Some(v);
        self
    }
    pub fn error(mut self, v: ErrorTag) -> Self {
        self.error = Some(v);
        self
    }
    pub fn count(mut self, v: u32) -> Self {
        self.count = Some(v);
        self
    }
    pub fn agent(mut self, v: AgentDetail) -> Self {
        self.agent = Some(v);
        self
    }
}

/// Sink for diagnostics. Implementations must never block the caller.
pub trait Diagnostics: Send + Sync {
    fn emit(&self, record: Record);
}

pub struct NullDiagnostics;
impl Diagnostics for NullDiagnostics {
    fn emit(&self, _record: Record) {}
}

#[cfg(test)]
mod tests;

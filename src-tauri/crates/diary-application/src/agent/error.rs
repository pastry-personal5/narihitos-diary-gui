//! Closed agent error codes; a code only, never content.

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

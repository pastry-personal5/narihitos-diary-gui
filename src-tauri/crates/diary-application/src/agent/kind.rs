//! Agent kinds, their per-kind facts and the profile handle a request carries.

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

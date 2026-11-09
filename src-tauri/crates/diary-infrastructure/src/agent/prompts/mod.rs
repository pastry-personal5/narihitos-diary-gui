//! Embedded definitions, strict parsing and single-pass rendering, independent of provider execution.
mod catalog;
mod parser;
mod renderer;
pub use catalog::EmbeddedPromptCatalog;
use diary_application::agent::*;
use parser::parse;

pub type Table<'a> = [(AgentKind, &'a str, &'a str)];

macro_rules! asset {
    ($kind:expr, $dir:literal, $name:literal) => {
        (
            $kind,
            $name,
            include_str!(concat!(
                "../../../../../../assets/prompts/",
                $dir,
                "/",
                $name,
                ".md"
            )),
        )
    };
}

pub const EMBEDDED: &Table<'static> = &[
    asset!(AgentKind::Codex, "codex", "generate-candidate"),
    asset!(AgentKind::Codex, "codex", "chat"),
    asset!(AgentKind::Claude, "claude", "generate-candidate"),
    asset!(AgentKind::Claude, "claude", "chat"),
    asset!(AgentKind::Qwen, "qwen", "generate-candidate"),
    asset!(AgentKind::Qwen, "qwen", "chat"),
    asset!(
        AgentKind::AppleFoundationModels,
        "apple-foundation-models",
        "generate-candidate"
    ),
    asset!(
        AgentKind::AppleFoundationModels,
        "apple-foundation-models",
        "chat"
    ),
    asset!(
        AgentKind::AppleFoundationModels,
        "apple-foundation-models",
        "session-instructions"
    ),
];

/// Client task-message assets are inventoried and parsed here in tests, not resolved as server agent actions.
#[cfg(test)]
const CLIENT_TASKS: &Table<'static> = &[
    asset!(AgentKind::Codex, "codex", "review-paragraph"),
    asset!(AgentKind::Claude, "claude", "review-paragraph"),
    asset!(AgentKind::Qwen, "qwen", "review-paragraph"),
    asset!(
        AgentKind::AppleFoundationModels,
        "apple-foundation-models",
        "review-paragraph"
    ),
];

pub const GENERATE: &str = "generate-candidate";
pub const PARAGRAPH_REVIEW: &str = "review-paragraph";
pub const CHAT: &str = "chat";
pub const SESSION: &str = "session-instructions";

#[derive(Clone)]
pub(super) struct PromptTemplate {
    body: String,
}

#[derive(Clone)]
pub(super) struct ParsedPromptAsset {
    template: PromptTemplate,
    /// Passed through unchanged; parsing does not claim the pin is supported.
    settings: AgentExecutionSettings,
    source: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetReason {
    Missing,
    Malformed(&'static str),
}
/// Names the asset only; carries no asset text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssetError {
    pub kind: AgentKind,
    pub name: &'static str,
    pub reason: AssetReason,
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod conformance;

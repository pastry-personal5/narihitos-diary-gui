//! Immutable prompt selection and preparation ports. No catalog I/O, asset parsing or JSON.
use super::{AgentError, AgentErrorCode, AgentKind, AgentRequest, Mode};
use std::sync::Arc;

pub const MAX_PREPARED_PROMPT_BYTES: usize = 512 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptId(String);
impl PromptId {
    /// App-authored identifiers only; never accept paths, arbitrary labels or diary content.
    pub fn parse(value: &str) -> Result<Self, AgentError> {
        if value.is_empty()
            || value.len() > 96
            || !value.bytes().all(|b| {
                b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'-' | b'_')
            })
        {
            return Err(AgentErrorCode::AssetsUnavailable.into());
        }
        Ok(Self(value.into()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptRevision(String);
impl PromptRevision {
    pub fn parse(value: &str) -> Result<Self, AgentError> {
        if value.len() != 64
            || !value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(AgentErrorCode::AssetsUnavailable.into());
        }
        Ok(Self(value.into()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptProvenance {
    pub id: PromptId,
    pub revision: PromptRevision,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptAction {
    Illustrated,
    Chat,
    GenerateCandidate,
}
impl PromptAction {
    pub fn of(mode: &Mode) -> Self {
        match mode {
            Mode::Illustrated(_) => Self::Illustrated,
            Mode::Chat(_) => Self::Chat,
            Mode::Candidate { .. } => Self::GenerateCandidate,
        }
    }
}

/// V1 selects only the bundled default. Future catalogs can define explicit selections separately.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PromptSelection {
    pub provider: AgentKind,
    pub action: PromptAction,
}
impl PromptSelection {
    pub fn for_request(request: &AgentRequest) -> Self {
        Self {
            provider: request.profile().kind,
            action: PromptAction::of(request.mode()),
        }
    }
}

/// Effective defaults are separate from template text. V1 has no override precedence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentExecutionSettings {
    pub model: String,
    pub reasoning_effort: String,
}

/// Content-bearing types intentionally provide no Debug or serialization implementation.
pub struct RenderedPrompt {
    pub task: String,
    pub session: Option<String>,
}
pub trait PromptRenderer: Send + Sync {
    fn retained_bytes(&self) -> usize;
    fn render(&self, request: &AgentRequest) -> Result<RenderedPrompt, AgentError>;
}

#[derive(Clone)]
pub struct ResolvedPrompt {
    provenance: PromptProvenance,
    selection: PromptSelection,
    settings: AgentExecutionSettings,
    renderer: Arc<dyn PromptRenderer>,
}
impl ResolvedPrompt {
    pub fn new(
        provenance: PromptProvenance,
        selection: PromptSelection,
        settings: AgentExecutionSettings,
        renderer: Arc<dyn PromptRenderer>,
    ) -> Self {
        Self {
            provenance,
            selection,
            settings,
            renderer,
        }
    }
    pub fn retained_bytes(&self) -> usize {
        self.renderer.retained_bytes()
            + self.settings.model.len()
            + self.settings.reasoning_effort.len()
            + self.provenance.id.as_str().len()
            + self.provenance.revision.as_str().len()
    }
    pub fn provenance(&self) -> &PromptProvenance {
        &self.provenance
    }
    pub fn selection(&self) -> PromptSelection {
        self.selection
    }
    pub fn settings(&self) -> &AgentExecutionSettings {
        &self.settings
    }
    /// Uses only this captured definition; it cannot observe a catalog replacement.
    pub fn prepare(&self, request: AgentRequest) -> Result<PreparedAgentRun, AgentError> {
        if PromptSelection::for_request(&request) != self.selection {
            return Err(AgentErrorCode::AssetsUnavailable.into());
        }
        let parts = self.renderer.render(&request)?;
        if parts.task.is_empty()
            || parts
                .task
                .len()
                .saturating_add(parts.session.as_ref().map_or(0, String::len))
                > MAX_PREPARED_PROMPT_BYTES
        {
            return Err(AgentErrorCode::Context.into());
        }
        if self.selection.provider.spec().session_asset
            && parts.session.as_ref().is_none_or(String::is_empty)
        {
            return Err(AgentErrorCode::AssetsUnavailable.into());
        }
        Ok(PreparedAgentRun {
            request,
            provenance: self.provenance.clone(),
            settings: self.settings.clone(),
            parts,
        })
    }
}

pub trait PromptResolver {
    fn resolve(&self, selection: PromptSelection) -> Result<ResolvedPrompt, AgentError>;
}

/// One fully prepared invocation; task/session parts and execution pins travel together.
pub struct PreparedAgentRun {
    request: AgentRequest,
    provenance: PromptProvenance,
    settings: AgentExecutionSettings,
    parts: RenderedPrompt,
}
impl PreparedAgentRun {
    pub fn request(&self) -> &AgentRequest {
        &self.request
    }
    pub fn provenance(&self) -> &PromptProvenance {
        &self.provenance
    }
    pub fn settings(&self) -> &AgentExecutionSettings {
        &self.settings
    }
    pub fn task(&self) -> &str {
        &self.parts.task
    }
    pub fn session(&self) -> Option<&str> {
        self.parts.session.as_deref()
    }
    pub fn input_contract_version(&self) -> u32 {
        self.request.schema_version()
    }
    pub fn output_contract_version(&self) -> u32 {
        super::SCHEMA_VERSION
    }
    pub fn into_request(self) -> AgentRequest {
        self.request
    }
}

//! Synthetic prompt definitions for application/transport tests; no production fallback.
use diary_application::agent::*;
use std::sync::Arc;
struct Renderer(char);
impl PromptRenderer for Renderer {
    fn retained_bytes(&self) -> usize {
        0
    }
    fn render(&self, _: &AgentRequest) -> Result<RenderedPrompt, AgentError> {
        Ok(RenderedPrompt {
            task: format!("fixture:{}", self.0),
            session: Some("fixture-session".into()),
        })
    }
}
pub(crate) fn definition(selection: PromptSelection) -> ResolvedPrompt {
    definition_at(selection, 'a')
}
pub(crate) fn definition_at(selection: PromptSelection, revision: char) -> ResolvedPrompt {
    ResolvedPrompt::new(
        PromptProvenance {
            id: PromptId::parse("synthetic.fixture").unwrap(),
            revision: PromptRevision::parse(&revision.to_string().repeat(64)).unwrap(),
        },
        selection,
        AgentExecutionSettings {
            model: "fixture".into(),
            reasoning_effort: "fixture".into(),
        },
        Arc::new(Renderer(revision)),
    )
}

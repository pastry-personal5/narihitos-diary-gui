//! The one `AgentRunner` the rest of the code holds: dispatches on `profile.kind` to the registered
//! adapters, shares one `Host` (cancel/shutdown/busy) and one set of prompt assets.
use super::{
    assets::PromptAssets, claude::ClaudeAdapter, codex::CodexAdapter, host::Host,
    profiles::ProfileTable, qwen::QwenAdapter,
};
use diary_application::agent::*;
use std::sync::Arc;

type Runner = Arc<dyn AgentRunner + Send + Sync>;

pub struct Agents {
    host: Arc<Host>,
    assets: PromptAssets,
    runners: Vec<(AgentKind, Runner)>,
}

impl Agents {
    /// Every adapter this build ships, in one place.
    pub fn standard(table: Arc<ProfileTable>, host: Arc<Host>) -> Self {
        let runners: Vec<(AgentKind, Runner)> = vec![
            (
                AgentKind::Codex,
                Arc::new(CodexAdapter::new(table.clone(), host.clone())),
            ),
            (
                AgentKind::Claude,
                Arc::new(ClaudeAdapter::new(table.clone(), host.clone())),
            ),
            (
                AgentKind::Qwen,
                Arc::new(QwenAdapter::new(table, host.clone())),
            ),
        ];
        Self {
            host,
            assets: PromptAssets::embedded(),
            runners,
        }
    }
    pub fn registered(&self, kind: AgentKind) -> bool {
        self.runners.iter().any(|(k, _)| *k == kind)
    }
    pub fn assets(&self) -> &PromptAssets {
        &self.assets
    }
    pub fn host(&self) -> &Arc<Host> {
        &self.host
    }
    pub fn shutdown(&self) {
        self.host.shutdown();
    }
    pub fn cancel_all(&self) {
        self.host.cancel_all();
    }
    fn runner(&self, kind: AgentKind) -> Option<&Runner> {
        self.runners
            .iter()
            .find(|(k, _)| *k == kind)
            .map(|(_, r)| r)
    }
}

impl AgentRunner for Agents {
    fn check(&self, profile: &ProfileHandle) -> ProfileCheck {
        match self.runner(profile.kind) {
            Some(r) => r.check(profile),
            None => ProfileStatus::Unavailable.into(),
        }
    }
    fn run(
        &self,
        request: AgentRequest,
        events: &mut dyn EventSink,
        cancel: &CancelToken,
    ) -> Result<AgentOutput, AgentError> {
        match self.runner(request.profile().kind) {
            Some(r) => r.run(request, events, cancel),
            None => Err(AgentErrorCode::Unavailable.into()),
        }
    }
}

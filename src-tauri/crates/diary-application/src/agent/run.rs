//! Events, outputs, the runner port and the cancel-epoch run guard.
use super::{
    AgentError, AgentErrorCode, AgentRequest, PreparedAgentRun, ProfileCheck, ProfileHandle,
    PromptResolver, PromptSelection,
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

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
    Illustrated(crate::illustrated::Plan),
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

pub trait AgentRunner: PromptResolver {
    /// Non-inference check; adapters bound it to 5 seconds.
    fn check(&self, profile: &ProfileHandle) -> ProfileCheck;
    fn run_prepared(
        &self,
        prepared: PreparedAgentRun,
        events: &mut dyn EventSink,
        cancel: &CancelToken,
    ) -> Result<AgentOutput, AgentError>;
    /// Standalone invocation without a GUI/session. Admitted jobs use captured definitions instead.
    fn run(
        &self,
        request: AgentRequest,
        events: &mut dyn EventSink,
        cancel: &CancelToken,
    ) -> Result<AgentOutput, AgentError> {
        if cancel.is_cancelled() {
            return Err(AgentErrorCode::Cancelled.into());
        }
        let prompt = self.resolve(PromptSelection::for_request(&request))?;
        self.run_prepared(prompt.prepare(request)?, events, cancel)
    }
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

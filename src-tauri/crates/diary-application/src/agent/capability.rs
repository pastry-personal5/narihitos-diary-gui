//! Per-action capability model and the read-only profile directory port.
use super::{AgentKind, ProfileStatus, Reason};

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

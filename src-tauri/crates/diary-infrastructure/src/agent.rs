//! Agent adapters' infrastructure: config-to-port mapping, embedded prompt assets, packet encoding,
//! strict generation-output validation, the generic CLI supervisor, the Codex adapter and read-only
//! profile status. No native (Apple) bridge yet.
use crate::config::Profile;
use diary_application::agent::{AgentKind, ProfileHandle};

pub mod apple;
pub mod assets;
pub mod claude;
pub mod codex;
pub mod engine;
pub mod host;
pub mod output;
pub mod packet;
pub mod process;
pub mod profiles;
pub mod qwen;
pub mod registry;

pub fn handle(profile: &Profile) -> ProfileHandle {
    let kind = match profile {
        Profile::Codex { .. } => AgentKind::Codex,
        Profile::Claude { .. } => AgentKind::Claude,
        Profile::Qwen { .. } => AgentKind::Qwen,
        Profile::AppleFoundationModels { .. } => AgentKind::AppleFoundationModels,
    };
    ProfileHandle {
        alias: profile.alias().to_owned(),
        kind,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn agent_kind_maps_every_config_profile() {
        let cli = |a: &str| (a.to_owned(), PathBuf::from("/h"), None);
        let (a, h, e) = cli("a");
        let profiles = [
            Profile::Codex {
                alias: a.clone(),
                profile_home: h.clone(),
                executable: e.clone(),
            },
            Profile::Claude {
                alias: a.clone(),
                profile_home: h.clone(),
                executable: e.clone(),
            },
            Profile::Qwen {
                alias: a.clone(),
                profile_home: h,
                executable: e,
            },
            Profile::AppleFoundationModels { alias: a },
        ];
        let kinds: Vec<_> = profiles.iter().map(|p| handle(p).kind).collect();
        assert_eq!(kinds, AgentKind::ALL);
        let dirs: Vec<_> = kinds.iter().map(|k| k.asset_dir()).collect();
        assert_eq!(dirs, ["codex", "claude", "qwen", "apple-foundation-models"]);
        assert_eq!(handle(&profiles[3]).alias, "a");
    }
}

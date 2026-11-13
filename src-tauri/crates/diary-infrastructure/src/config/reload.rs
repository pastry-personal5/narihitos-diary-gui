//! Explicit, atomic config reload: validate a complete new snapshot, then adopt it.
use super::{Config, ConfigError, Profile, SourceKind, load};
use diary_application::{ConfigReloader, ReloadIssue, ReloadOutcome};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    sync::Mutex,
};

/// Fields that cannot change beneath a buffer or job: a change needs a restart.
#[derive(PartialEq)]
struct Fixed {
    illustrated: super::IllustratedConfig,
    entries_root: PathBuf,
    runtime_root: PathBuf,
    port: u16,
    fallback: u8,
    sources: Vec<(u8, PathBuf)>,
}
/// Fields that may change only while no operation can be using them. Never Debug or Serialize.
#[derive(PartialEq)]
struct Guarded {
    images: std::collections::BTreeMap<String, super::ImageEngineConfig>,
    icloud: Option<PathBuf>,
    notion: Option<(String, String)>,
    profiles: Vec<(String, u8, Option<PathBuf>, Option<PathBuf>)>,
}

fn fingerprints(c: &Config) -> (Fixed, Guarded) {
    let fixed = Fixed {
        illustrated: c.illustrated.clone(),
        entries_root: c.paths.entries_root.clone(),
        runtime_root: c.paths.runtime_root.clone(),
        port: c.server.port,
        fallback: c.server.port_fallback_count,
        sources: c
            .sources
            .iter()
            .map(|s| {
                let kind = match s.kind {
                    SourceKind::Narihito => 0,
                    SourceKind::Ayumi => 1,
                };
                (kind, s.root.clone())
            })
            .collect(),
    };
    let guarded = Guarded {
        images: c.image_engines.clone(),
        icloud: c.paths.icloud_entries_root.clone(),
        notion: c
            .notion
            .as_ref()
            .map(|n| (n.root_page_id.clone(), n.token.clone())),
        profiles: c
            .agents
            .profiles
            .iter()
            .map(|p| {
                let (kind, home, exe) = match p {
                    Profile::Codex {
                        profile_home,
                        executable,
                        ..
                    } => (0, Some(profile_home), executable),
                    Profile::Claude {
                        profile_home,
                        executable,
                        ..
                    } => (1, Some(profile_home), executable),
                    Profile::Qwen {
                        profile_home,
                        executable,
                        ..
                    } => (2, Some(profile_home), executable),
                    Profile::AppleFoundationModels { .. } => (3, None, &None),
                };
                (p.alias().into(), kind, home.cloned(), exe.clone())
            })
            .collect(),
    };
    (fixed, guarded)
}

fn issue(e: ConfigError) -> ReloadIssue {
    let (path, code) = match e {
        ConfigError::Syntax => ("config", "syntax"),
        ConfigError::Schema => ("config", "schema"),
        ConfigError::Limit => ("config", "limit"),
        ConfigError::Version => ("schema_version", "unsupported"),
        ConfigError::Port => ("server.port", "invalid"),
        ConfigError::Alias => ("sources|agents.profiles", "alias"),
        ConfigError::Path => ("paths", "invalid"),
        ConfigError::Overlap => ("paths", "overlap"),
        ConfigError::Permissions => ("config", "permissions"),
        ConfigError::Io => ("config", "unreadable"),
    };
    ReloadIssue { path, code }
}

pub struct ConfigHandle {
    images: Option<std::sync::Arc<diary_application::image::Registry>>,
    path: PathBuf,
    launch_path: OsString,
    launch_cwd: PathBuf,
    current: Mutex<(Fixed, Guarded)>,
    profiles: Option<std::sync::Arc<crate::agent::profiles::ProfileTable>>,
}
impl ConfigHandle {
    pub fn new(path: &Path, config: &Config, launch_path: OsString, launch_cwd: PathBuf) -> Self {
        Self {
            path: launch_cwd.join(path),
            launch_path,
            launch_cwd,
            current: Mutex::new(fingerprints(config)),
            profiles: None,
            images: None,
        }
    }
    pub fn with_images(
        mut self,
        images: std::sync::Arc<diary_application::image::Registry>,
    ) -> Self {
        self.images = Some(images);
        self
    }
    /// Keep the agent profile table in step with adopted reloads.
    pub fn with_profiles(
        mut self,
        table: std::sync::Arc<crate::agent::profiles::ProfileTable>,
    ) -> Self {
        self.profiles = Some(table);
        self
    }
}
impl ConfigReloader for ConfigHandle {
    fn reload(&self, busy: bool) -> ReloadOutcome {
        let config = match load(&self.path) {
            Ok(config) => config,
            Err(e) => return ReloadOutcome::Rejected(vec![issue(e)]),
        };
        let (fixed, guarded) = fingerprints(&config);
        {
            let mut current = self.current.lock().expect("reload state poisoned");
            if fixed != current.0 {
                let mut issues = Vec::new();
                let mut note = |changed: bool, path: &'static str| {
                    if changed {
                        issues.push(ReloadIssue {
                            path,
                            code: "restart_required",
                        });
                    }
                };
                note(fixed.illustrated != current.0.illustrated, "illustrated");
                note(
                    fixed.entries_root != current.0.entries_root,
                    "paths.entries_root",
                );
                note(
                    fixed.runtime_root != current.0.runtime_root,
                    "paths.runtime_root",
                );
                note(
                    fixed.port != current.0.port || fixed.fallback != current.0.fallback,
                    "server",
                );
                note(fixed.sources != current.0.sources, "sources");
                return ReloadOutcome::RestartRequired(issues);
            }
            if guarded != current.1 {
                if busy {
                    return ReloadOutcome::Busy;
                }
                if let Some(images) = &self.images
                    && images
                        .replace(crate::image::configured(&config, images.shutdown_signal()))
                        .is_err()
                {
                    return ReloadOutcome::Busy;
                }
                current.1 = guarded;
                if let Some(t) = &self.profiles {
                    t.replace(&config);
                }
            }
        }
        ReloadOutcome::Applied(config.observe_startup(&self.launch_path, &self.launch_cwd))
    }
}

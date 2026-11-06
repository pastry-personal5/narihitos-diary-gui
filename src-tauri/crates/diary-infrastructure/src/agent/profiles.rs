//! Profile table (config → port mapping, updated on reload) and the read-only status service behind
//! `GET /profiles`. Listing never spawns anything; only an explicit check does, and never inference.
use super::{assets::PromptAssets, claude::ClaudeAdapter, codex::CodexAdapter, qwen::QwenAdapter};
use crate::config::{Config, Profile};
use diary_application::agent::{
    Action, AgentKind, AgentRunner, ProfileDirectory, ProfileHandle, ProfileView,
};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub alias: String,
    pub kind: AgentKind,
    pub home: Option<PathBuf>,
    pub executable: Option<PathBuf>,
}
impl Entry {
    fn from_config(p: &Profile) -> Self {
        let (home, executable) = match p {
            Profile::Codex {
                profile_home,
                executable,
                ..
            }
            | Profile::Claude {
                profile_home,
                executable,
                ..
            }
            | Profile::Qwen {
                profile_home,
                executable,
                ..
            } => (Some(profile_home.clone()), executable.clone()),
            Profile::AppleFoundationModels { .. } => (None, None),
        };
        Self {
            alias: p.alias().to_owned(),
            kind: super::handle(p).kind,
            home,
            executable,
        }
    }
}

/// Current profiles; `generation` changes whenever the set does so caches can drop stale answers.
#[derive(Default)]
pub struct ProfileTable {
    entries: Mutex<Vec<Entry>>,
    generation: AtomicU64,
}
impl ProfileTable {
    pub fn from_config(config: &Config) -> Self {
        let t = Self::default();
        t.replace(config);
        t
    }
    pub fn from_entries(entries: Vec<Entry>) -> Self {
        Self {
            entries: Mutex::new(entries),
            generation: AtomicU64::new(0),
        }
    }
    pub fn replace(&self, config: &Config) {
        let next: Vec<_> = config
            .agents
            .profiles
            .iter()
            .map(Entry::from_config)
            .collect();
        let mut cur = self.entries.lock().expect("profile table poisoned");
        if *cur != next {
            *cur = next;
            self.generation.fetch_add(1, Ordering::SeqCst);
        }
    }
    pub fn list(&self) -> Vec<Entry> {
        self.entries.lock().expect("profile table poisoned").clone()
    }
    pub fn get(&self, alias: &str) -> Option<Entry> {
        self.list().into_iter().find(|e| e.alias == alias)
    }
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }
}

pub struct ProfileService {
    table: Arc<ProfileTable>,
    adapter: Arc<CodexAdapter>,
    claude: Arc<ClaudeAdapter>,
    qwen: Arc<QwenAdapter>,
    cache: Mutex<(
        u64,
        HashMap<String, diary_application::agent::ProfileStatus>,
    )>,
}
impl ProfileService {
    pub fn new(
        table: Arc<ProfileTable>,
        adapter: Arc<CodexAdapter>,
        claude: Arc<ClaudeAdapter>,
        qwen: Arc<QwenAdapter>,
    ) -> Self {
        Self {
            cache: Mutex::new((table.generation(), HashMap::new())),
            table,
            adapter,
            claude,
            qwen,
        }
    }
    fn cached(&self, alias: &str) -> Option<diary_application::agent::ProfileStatus> {
        let mut c = self.cache.lock().expect("profile cache poisoned");
        let generation = self.table.generation();
        if c.0 != generation {
            *c = (generation, HashMap::new());
        }
        c.1.get(alias).copied()
    }
    fn view(
        &self,
        e: &Entry,
        status: Option<diary_application::agent::ProfileStatus>,
    ) -> ProfileView {
        let assets: &PromptAssets = self.adapter.assets();
        let registered = matches!(
            e.kind,
            AgentKind::Codex | AgentKind::Claude | AgentKind::Qwen
        );
        ProfileView {
            alias: e.alias.clone(),
            kind: e.kind,
            status: if registered {
                status
            } else {
                Some(diary_application::agent::ProfileStatus::Unavailable)
            },
            chat: assets.capability(e.kind, Action::Chat, registered),
            generate: assets.capability(e.kind, Action::Generate, registered),
        }
    }
}
impl ProfileDirectory for ProfileService {
    fn list(&self) -> Vec<ProfileView> {
        self.table
            .list()
            .iter()
            .map(|e| self.view(e, self.cached(&e.alias)))
            .collect()
    }
    fn check(&self, alias: &str) -> Option<ProfileView> {
        let generation = self.table.generation();
        let e = self.table.get(alias)?;
        let runner: Option<&dyn AgentRunner> = match e.kind {
            AgentKind::Codex => Some(&*self.adapter),
            AgentKind::Claude => Some(&*self.claude),
            AgentKind::Qwen => Some(&*self.qwen),
            _ => None,
        };
        let status = runner.map(|runner| {
            let handle = ProfileHandle {
                alias: e.alias.clone(),
                kind: e.kind,
            };
            let s = runner.check(&handle);
            let mut c = self.cache.lock().expect("profile cache poisoned");
            // Busy is transient (a run was active); caching it would show a stale "busy" later.
            // A reload during the probe makes the answer stale; the cache itself may lag behind.
            if s != diary_application::agent::ProfileStatus::Busy
                && generation == self.table.generation()
            {
                if c.0 != generation {
                    *c = (generation, HashMap::new());
                }
                c.1.insert(e.alias.clone(), s);
            }
            s
        });
        Some(self.view(&e, status))
    }
}

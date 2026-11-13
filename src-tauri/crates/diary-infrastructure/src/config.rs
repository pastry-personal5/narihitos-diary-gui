//! Strict private startup configuration. Errors never contain rejected values.
use diary_logging::private_fs::{FsError, PrivateDir, check_private_metadata};
use serde::Deserialize;
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

impl From<FsError> for ConfigError {
    fn from(e: FsError) -> Self {
        match e {
            FsError::Path => Self::Path,
            FsError::Permissions => Self::Permissions,
            FsError::Io | FsError::Busy => Self::Io,
        }
    }
}

pub const MAX_CONFIG_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigError {
    Syntax,
    Schema,
    /// Legacy Notion configuration must be explicitly rewritten as notion.pages.
    NotionPagesRequired,
    Limit,
    Version,
    Port,
    Alias,
    Path,
    Overlap,
    Permissions,
    Io,
}

// Deliberately no Debug or Serialize implementation: this structure holds secrets.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub illustrated: IllustratedConfig,
    #[serde(default)]
    pub image_engines: std::collections::BTreeMap<String, ImageEngineConfig>,
    pub schema_version: u32,
    pub paths: Paths,
    #[serde(default)]
    pub server: Server,
    #[serde(default)]
    pub sources: Vec<Source>,
    #[serde(default)]
    pub agents: Agents,
    pub notion: Option<Notion>,
}
#[derive(Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageEngineConfig {
    #[serde(default)]
    pub trigger: String,
    #[serde(default)]
    pub prompt_directives: Vec<String>,
    pub label: String,
    pub program: PathBuf,
    #[serde(default = "image_timeout")]
    pub timeout_seconds: u64,
}
fn image_timeout() -> u64 {
    1800
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Paths {
    pub entries_root: PathBuf,
    pub runtime_root: PathBuf,
    pub icloud_entries_root: Option<PathBuf>,
}
#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Server {
    pub port: u16,
    pub port_fallback_count: u8,
}
impl Default for Server {
    fn default() -> Self {
        Self {
            port: 8781,
            port_fallback_count: 8,
        }
    }
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub alias: String,
    #[serde(rename = "type")]
    pub kind: SourceKind,
    pub root: PathBuf,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceKind {
    Narihito,
    Ayumi,
}
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Agents {
    #[serde(default)]
    pub profiles: Vec<Profile>,
}
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Profile {
    Codex {
        alias: String,
        profile_home: PathBuf,
        executable: Option<PathBuf>,
    },
    Claude {
        alias: String,
        profile_home: PathBuf,
        executable: Option<PathBuf>,
    },
    Qwen {
        alias: String,
        profile_home: PathBuf,
        executable: Option<PathBuf>,
    },
    AppleFoundationModels {
        alias: String,
    },
}
impl Profile {
    pub fn alias(&self) -> &str {
        match self {
            Self::Codex { alias, .. }
            | Self::Claude { alias, .. }
            | Self::Qwen { alias, .. }
            | Self::AppleFoundationModels { alias } => alias,
        }
    }
    fn cli_paths(&mut self) -> Option<(&mut PathBuf, &mut Option<PathBuf>)> {
        match self {
            Self::Codex {
                profile_home,
                executable,
                ..
            }
            | Self::Claude {
                profile_home,
                executable,
                ..
            }
            | Self::Qwen {
                profile_home,
                executable,
                ..
            } => Some((profile_home, executable)),
            Self::AppleFoundationModels { .. } => None,
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Notion {
    pub pages: Vec<NotionPage>,
    pub token: String,
}
#[derive(Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotionPage {
    #[serde(rename = "type")]
    pub kind: NotionKind,
    pub root_page_id: String,
}
#[derive(Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NotionKind {
    Normal,
    Illustrated,
}

/// Parsing is independent of filesystem access, for synthetic schema fixtures.
pub fn parse(bytes: &[u8]) -> Result<Config, ConfigError> {
    if bytes.len() > MAX_CONFIG_BYTES {
        return Err(ConfigError::Limit);
    }
    validate_yaml_events(bytes)?;
    // Value decoding rejects duplicates; decoding from Value prevents YAML's
    // string deserializer from coercing booleans/numbers into credential strings.
    let value: serde_yaml_ng::Value =
        serde_yaml_ng::from_slice(bytes).map_err(|_| ConfigError::Syntax)?;
    fn no_null(value: &serde_yaml_ng::Value) -> bool {
        match value {
            serde_yaml_ng::Value::Null => false,
            serde_yaml_ng::Value::Sequence(items) => items.iter().all(no_null),
            serde_yaml_ng::Value::Mapping(items) => items
                .iter()
                .all(|(key, value)| no_null(key) && no_null(value)),
            _ => true,
        }
    }
    if !no_null(&value) {
        return Err(ConfigError::Schema);
    }
    if value
        .get("notion")
        .and_then(|n| n.as_mapping())
        .is_some_and(|n| n.contains_key(serde_yaml_ng::Value::String("root_page_id".into())))
    {
        return Err(ConfigError::NotionPagesRequired);
    }
    let mut config: Config = serde_yaml_ng::from_value(value).map_err(|_| ConfigError::Schema)?;
    if config.schema_version != 1 {
        return Err(ConfigError::Version);
    }
    if config.server.port < 1024
        || config.server.port_fallback_count > 8
        || config
            .server
            .port
            .checked_add(u16::from(config.server.port_fallback_count))
            .is_none()
    {
        return Err(ConfigError::Port);
    }
    if !config.paths.entries_root.is_absolute() || !config.paths.runtime_root.is_absolute() {
        return Err(ConfigError::Path);
    }
    if config.sources.len() > 64 || config.agents.profiles.len() > 64 {
        return Err(ConfigError::Limit);
    }
    aliases(config.sources.iter().map(|s| s.alias.as_str()))?;
    aliases(config.agents.profiles.iter().map(Profile::alias))?;
    if let Some(n) = &mut config.notion {
        if n.token.trim().is_empty()
            || n.token.len() > 4096
            || n.pages.is_empty()
            || n.pages.len() > 2
        {
            return Err(ConfigError::Schema);
        }
        for page in &mut n.pages {
            if page.root_page_id.len() != 32
                && (page.root_page_id.len() != 36
                    || ![8, 13, 18, 23]
                        .into_iter()
                        .all(|i| page.root_page_id.as_bytes()[i] == b'-'))
            {
                return Err(ConfigError::Schema);
            }
            page.root_page_id =
                crate::export::notion::adapter::normalize_page_id(&page.root_page_id)
                    .ok_or(ConfigError::Schema)?;
        }
        if n.pages.len() == 2
            && (n.pages[0].kind == n.pages[1].kind
                || n.pages[0].root_page_id == n.pages[1].root_page_id)
        {
            return Err(ConfigError::Schema);
        }
        n.pages
            .sort_by_key(|p| matches!(p.kind, NotionKind::Illustrated));
    }
    if !(1..=99).contains(&config.illustrated.max_photos)
        || config
            .illustrated
            .engine
            .as_ref()
            .is_some_and(|id| !config.image_engines.contains_key(id))
    {
        return Err(ConfigError::Schema);
    }
    if config.image_engines.len() > 64 {
        return Err(ConfigError::Limit);
    }
    aliases(config.image_engines.keys().map(String::as_str))?;
    for e in config.image_engines.values() {
        if !(diary_application::illustrated::PromptSettings {
            trigger: e.trigger.clone(),
            directives: e.prompt_directives.clone(),
        })
        .validate()
            || e.label.trim().is_empty()
            || e.label.len() > 128
            || !e.program.is_absolute()
            || !(1..=3600).contains(&e.timeout_seconds)
        {
            return Err(ConfigError::Schema);
        }
    }
    Ok(config)
}
fn aliases<'a>(values: impl Iterator<Item = &'a str>) -> Result<(), ConfigError> {
    let mut seen = HashSet::new();
    for alias in values {
        if alias.is_empty()
            || alias.len() > 64
            || !alias
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
            || !seen.insert(alias)
        {
            return Err(ConfigError::Alias);
        }
    }
    Ok(())
}

/// Event pass bounds depth before recursive deserialization and rejects all
/// anchors, aliases and explicit tags, including built-in tags. Quoted literal
/// punctuation remains valid. Each parser/event allocation is released once.
fn validate_yaml_events(bytes: &[u8]) -> Result<(), ConfigError> {
    use unsafe_libyaml::*;
    // SAFETY: parser is initialized before use; the input slice lives throughout
    // parsing; union fields are read only for their active discriminant. Every
    // initialized event and the parser are deleted on all return paths.
    unsafe {
        let mut parser = std::mem::MaybeUninit::<yaml_parser_t>::uninit();
        if yaml_parser_initialize(parser.as_mut_ptr()).fail {
            return Err(ConfigError::Syntax);
        }
        let mut parser = parser.assume_init();
        yaml_parser_set_input_string(&mut parser, bytes.as_ptr(), bytes.len() as u64);
        let result = (|| {
            let mut depth = 0usize;
            loop {
                let mut event = std::mem::MaybeUninit::<yaml_event_t>::uninit();
                if yaml_parser_parse(&mut parser, event.as_mut_ptr()).fail {
                    return Err(ConfigError::Syntax);
                }
                let mut event = event.assume_init();
                let kind = event.type_;
                let forbidden = match kind {
                    YAML_ALIAS_EVENT => true,
                    YAML_SCALAR_EVENT => {
                        !event.data.scalar.tag.is_null() || !event.data.scalar.anchor.is_null()
                    }
                    YAML_SEQUENCE_START_EVENT => {
                        !event.data.sequence_start.tag.is_null()
                            || !event.data.sequence_start.anchor.is_null()
                    }
                    YAML_MAPPING_START_EVENT => {
                        !event.data.mapping_start.tag.is_null()
                            || !event.data.mapping_start.anchor.is_null()
                    }
                    _ => false,
                };
                yaml_event_delete(&mut event);
                if forbidden {
                    return Err(ConfigError::Syntax);
                }
                match kind {
                    YAML_SEQUENCE_START_EVENT | YAML_MAPPING_START_EVENT => {
                        depth += 1;
                        if depth > 16 {
                            return Err(ConfigError::Limit);
                        }
                    }
                    YAML_SEQUENCE_END_EVENT | YAML_MAPPING_END_EVENT => {
                        depth = depth.saturating_sub(1);
                    }
                    YAML_STREAM_END_EVENT => return Ok(()),
                    _ => (),
                }
            }
        })();
        yaml_parser_delete(&mut parser);
        result
    }
}

/// Resolve against canonical existing parents, preserving missing leaves. Never
/// create a workspace here: durable initialization belongs to P1-M5.
fn canonical_proposed(path: &Path) -> Result<PathBuf, ConfigError> {
    use std::os::unix::ffi::OsStrExt;
    use std::path::Component;
    if path.as_os_str().is_empty()
        || path.as_os_str().as_bytes().contains(&0)
        || path.as_os_str().len() > 4096
        || !path.is_absolute()
    {
        return Err(ConfigError::Path);
    }
    let mut result = PathBuf::from("/");
    for part in path.components() {
        match part {
            Component::RootDir | Component::CurDir => (),
            Component::ParentDir => {
                result.pop();
            }
            Component::Normal(name) => {
                result.push(name);
                match fs::canonicalize(&result) {
                    Ok(canonical) => result = canonical,
                    Err(error)
                        if matches!(
                            error.kind(),
                            std::io::ErrorKind::NotFound | std::io::ErrorKind::PermissionDenied
                        ) => {}
                    Err(_) => return Err(ConfigError::Path),
                }
            }
            _ => return Err(ConfigError::Path),
        }
    }
    Ok(result)
}

impl Config {
    pub fn resolve_paths(&mut self, config_file: &Path) -> Result<(), ConfigError> {
        let base = config_file.parent().ok_or(ConfigError::Path)?;
        let resolve = |path: &Path| {
            if path.as_os_str().is_empty() {
                return Err(ConfigError::Path);
            }
            canonical_proposed(&base.join(path))
        };
        self.paths.entries_root = resolve(&self.paths.entries_root)?;
        self.paths.runtime_root = resolve(&self.paths.runtime_root)?;
        if let Some(path) = &mut self.paths.icloud_entries_root {
            *path = resolve(path)?;
        }
        for source in &mut self.sources {
            source.root = resolve(&source.root)?;
        }
        let mut roots = vec![&self.paths.entries_root, &self.paths.runtime_root];
        roots.extend(self.paths.icloud_entries_root.iter());
        roots.extend(self.sources.iter().map(|s| &s.root));
        for (i, root) in roots.iter().enumerate() {
            if roots[..i]
                .iter()
                .any(|other| root.starts_with(other) || other.starts_with(root))
            {
                return Err(ConfigError::Overlap);
            }
        }
        let mut private = vec![config_file.to_path_buf()];
        for profile in &mut self.agents.profiles {
            if let Some((home, executable)) = profile.cli_paths() {
                *home = resolve(home)?;
                private.push(home.clone());
                if let Some(path) = executable {
                    *path = resolve(path)?;
                }
            }
        }
        if private
            .iter()
            .any(|path| roots.iter().any(|root| path.starts_with(root)))
        {
            return Err(ConfigError::Overlap);
        }
        Ok(())
    }
}

mod reload;
pub use reload::ConfigHandle;
#[cfg(test)]
mod tests;

/// Read only a regular current-user 0600 file through a private 0700 parent.
/// Open every ancestor with O_NOFOLLOW, then keep the validated descriptors open
/// through the bounded read; path replacement cannot redirect the opened file.
pub fn load(path: &Path) -> Result<Config, ConfigError> {
    use std::io::Read;
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|_| ConfigError::Io)?
            .join(path)
    };
    let (mut file, canonical) = open_private(&absolute)?;
    let mut bytes = Vec::new();
    file.by_ref()
        .take((MAX_CONFIG_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| ConfigError::Io)?;
    let mut config = parse(&bytes)?;
    config.resolve_paths(&canonical)?;
    for root in [&config.paths.entries_root, &config.paths.runtime_root] {
        if root.try_exists().map_err(|_| ConfigError::Io)? {
            check_private_metadata(
                &fs::symlink_metadata(root).map_err(|_| ConfigError::Io)?,
                true,
            )?;
        }
    }
    Ok(config)
}

pub fn open_private(path: &Path) -> Result<(fs::File, PathBuf), ConfigError> {
    let parent = path.parent().ok_or(ConfigError::Path)?;
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(ConfigError::Path)?;
    let directory = PrivateDir::open(parent)?;
    Ok((directory.read(filename)?, path.to_path_buf()))
}

impl Config {
    /// Resolve launch-PATH defaults without executing any binary or reading profile contents.
    pub fn observe_startup(
        &self,
        launch_path: &std::ffi::OsStr,
        launch_cwd: &Path,
    ) -> diary_application::StartupSummary {
        use diary_application::{Availability, StartupSummary, UnavailableReason as Reason};
        let sources = self
            .sources
            .iter()
            .map(|source| {
                let reason = match fs::read_dir(&source.root) {
                    Ok(_) => None,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                        Some(Reason::SourceRootMissing)
                    }
                    Err(_) => Some(Reason::SourceRootDenied),
                };
                Availability {
                    alias: source.alias.clone(),
                    reason,
                }
            })
            .collect();
        let profiles = self
            .agents
            .profiles
            .iter()
            .map(|profile| {
                let (home, executable, default) = match profile {
                    Profile::Codex {
                        profile_home,
                        executable,
                        ..
                    } => (profile_home, executable, "codex"),
                    Profile::Claude {
                        profile_home,
                        executable,
                        ..
                    } => (profile_home, executable, "claude"),
                    Profile::Qwen {
                        profile_home,
                        executable,
                        ..
                    } => (profile_home, executable, "qwen"),
                    Profile::AppleFoundationModels { alias } => {
                        return Availability {
                            alias: alias.clone(),
                            reason: Some(Reason::NativeAdapterUnverified),
                        };
                    }
                };
                let reason = Some(match fs::read_dir(home) {
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                        Reason::ProfileHomeMissing
                    }
                    Err(_) => Reason::ProfileHomeDenied,
                    Ok(_) => {
                        if select_executable(
                            executable.as_deref(),
                            default,
                            launch_path,
                            launch_cwd,
                        )
                        .is_some()
                        {
                            Reason::AgentAdapterUnverified
                        } else {
                            Reason::ExecutableMissing
                        }
                    }
                });
                Availability {
                    alias: profile.alias().into(),
                    reason,
                }
            })
            .collect();
        StartupSummary { sources, profiles }
    }
}

pub fn select_executable(
    override_path: Option<&Path>,
    default: &str,
    launch_path: &std::ffi::OsStr,
    launch_cwd: &Path,
) -> Option<PathBuf> {
    fn executable(path: &Path) -> Option<PathBuf> {
        use std::os::unix::ffi::OsStrExt;
        let path = fs::canonicalize(path).ok()?;
        if !path.is_absolute() || !fs::metadata(&path).ok()?.is_file() {
            return None;
        }
        let c = std::ffi::CString::new(path.as_os_str().as_bytes()).ok()?;
        // SAFETY: access checks a live, terminated path; it does not execute it.
        if unsafe { libc::access(c.as_ptr(), libc::X_OK) } != 0 {
            return None;
        }
        Some(path)
    }
    if let Some(path) = override_path {
        return executable(path);
    }
    std::env::split_paths(launch_path)
        .find_map(|directory| executable(&launch_cwd.join(directory).join(default)))
}

#[derive(Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct IllustratedConfig {
    pub engine: Option<String>,
    pub max_photos: usize,
}
impl Default for IllustratedConfig {
    fn default() -> Self {
        Self {
            engine: None,
            max_photos: 4,
        }
    }
}

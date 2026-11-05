//! Client-owned user preferences in `user_settings.json` (see docs/architecture/user-settings.md).
//! The server never reads this file. Nothing here logs or returns section contents or paths.
use diary_logging::private_fs::PrivateDir;
use serde::{Deserialize, Serialize, de};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    os::unix::fs::DirBuilderExt,
    path::PathBuf,
    sync::Mutex,
};

mod locale;
pub use locale::canonical_tag;

const FILE: &str = "user_settings.json";
const TEMP: &str = "user_settings.json.tmp";
const MAX_BYTES: usize = 256 * 1024;
const SECTIONS: [&str; 2] = ["keybindings", "writing"];

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct Warning {
    pub section: &'static str,
    pub code: &'static str,
}
#[derive(Serialize, Clone, Debug)]
pub struct Snapshot {
    pub revision: String,
    pub read_only: bool,
    /// True when an explicit `Recover` can replace the file (invalid or newer schema).
    pub recoverable: bool,
    pub warnings: Vec<Warning>,
    pub writing_locale: Option<String>,
    pub keybindings: Value,
}
pub enum Change {
    /// A user-chosen language tag (validated and canonicalised here).
    Writing(String),
    /// First run: store the first valid tag of the system's preferred list, only if none is saved.
    InitWriting(Vec<String>),
    Keybindings(Value),
    /// Back the original up privately, then replace an invalid or newer-schema file with defaults.
    Recover,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Conflict,
    ReadOnly,
    FileInvalid,
    Invalid,
    Unresolved,
    /// The write may or may not have been published: reload before the next update.
    Uncertain,
    Unavailable,
}
impl Error {
    pub fn code(self) -> &'static str {
        match self {
            Self::Conflict => "conflict",
            Self::ReadOnly => "read_only",
            Self::FileInvalid => "file_invalid",
            Self::Invalid => "invalid_input",
            Self::Unresolved => "language_unresolved",
            Self::Uncertain => "uncertain",
            Self::Unavailable => "unavailable",
        }
    }
}

enum Disk {
    Missing,
    Valid(Map<String, Value>),
    Invalid,
    Newer,
    Unsafe,
}
struct Observed {
    revision: String,
    disk: Disk,
}

/// Top-level object that refuses duplicate keys.
struct Top(Vec<(String, Value)>);
impl<'de> Deserialize<'de> for Top {
    fn deserialize<D: de::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> de::Visitor<'de> for V {
            type Value = Top;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("an object")
            }
            fn visit_map<A: de::MapAccess<'de>>(self, mut m: A) -> Result<Top, A::Error> {
                let mut items: Vec<(String, Value)> = Vec::new();
                while let Some((k, v)) = m.next_entry::<String, Value>()? {
                    if items.iter().any(|(seen, _)| *seen == k) {
                        return Err(de::Error::custom("duplicate key"));
                    }
                    items.push((k, v));
                }
                Ok(Top(items))
            }
        }
        d.deserialize_map(V)
    }
}

fn parse(bytes: &[u8]) -> Disk {
    if bytes.len() > MAX_BYTES {
        return Disk::Invalid;
    }
    let Ok(Top(items)) = serde_json::from_slice::<Top>(bytes) else {
        return Disk::Invalid;
    };
    let version = items.iter().find(|(k, _)| k == "schema_version");
    match version.map(|(_, v)| v.as_u64()) {
        Some(Some(1)) => {}
        Some(Some(n)) if n > 1 => return Disk::Newer,
        _ => return Disk::Invalid,
    }
    let mut map = Map::new();
    for (k, v) in items {
        if k == "schema_version" {
            continue;
        }
        if !SECTIONS.contains(&k.as_str()) {
            return Disk::Invalid;
        }
        map.insert(k, v);
    }
    Disk::Valid(map)
}
fn valid_writing(v: &Value) -> Option<String> {
    let object = v.as_object().filter(|o| o.len() == 1)?;
    canonical_tag(object.get("locale")?.as_str()?)
}
fn valid_keybindings(v: &Value) -> bool {
    v.as_object().is_some_and(|o| {
        o.len() <= 512
            && o.values().all(|keys| {
                keys.as_array().is_some_and(|a| {
                    a.len() <= 8 && a.iter().all(|k| k.as_str().is_some_and(|k| k.len() <= 64))
                })
            })
    })
}
fn revision(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn snapshot_of(r: &Observed) -> Snapshot {
    let warn = |section, code| Warning { section, code };
    let mut s = Snapshot {
        revision: r.revision.clone(),
        read_only: false,
        recoverable: false,
        warnings: Vec::new(),
        writing_locale: None,
        keybindings: json!({}),
    };
    match &r.disk {
        Disk::Missing => {}
        Disk::Invalid => {
            s.recoverable = true;
            s.warnings.push(warn("file", "file_invalid"));
        }
        Disk::Newer => {
            s.read_only = true;
            s.recoverable = true;
            s.warnings.push(warn("file", "newer_schema"));
        }
        Disk::Unsafe => {
            s.read_only = true;
            s.warnings.push(warn("file", "location_unsafe"));
        }
        Disk::Valid(map) => {
            if let Some(v) = map.get("writing") {
                match valid_writing(v) {
                    Some(tag) => s.writing_locale = Some(tag),
                    None => s.warnings.push(warn("writing", "section_invalid")),
                }
            }
            if let Some(v) = map.get("keybindings") {
                if valid_keybindings(v) {
                    s.keybindings = v.clone();
                } else {
                    s.warnings.push(warn("keybindings", "section_invalid"));
                }
            }
        }
    }
    s
}

#[cfg(test)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Fault {
    None,
    BeforeRename,
    AfterRename,
}
struct Inner {
    uncertain: bool,
    #[cfg(test)]
    fault: Fault,
}
pub struct Store {
    dir: PathBuf,
    inner: Mutex<Inner>,
}
impl Store {
    /// `dir` is the absolute app config directory; it is created 0700 on the first write.
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            inner: Mutex::new(Inner {
                uncertain: false,
                #[cfg(test)]
                fault: Fault::None,
            }),
        }
    }
    #[cfg(test)]
    pub(crate) fn set_fault(&self, fault: Fault) {
        self.inner.lock().expect("settings state poisoned").fault = fault;
    }

    fn read(&self) -> Observed {
        let missing = || Observed {
            revision: "missing".into(),
            disk: Disk::Missing,
        };
        let unsafe_ = || Observed {
            revision: "unsafe".into(),
            disk: Disk::Unsafe,
        };
        match std::fs::symlink_metadata(&self.dir) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return missing(),
            Err(_) => return unsafe_(),
            Ok(_) => {}
        }
        let Ok(dir) = PrivateDir::open(&self.dir) else {
            return unsafe_();
        };
        match dir.list() {
            Ok(names) if names.iter().any(|n| n == FILE) => {}
            Ok(_) => return missing(),
            Err(_) => return unsafe_(),
        }
        let Ok(file) = dir.read(FILE) else {
            return unsafe_();
        };
        let mut bytes = Vec::new();
        if file
            .take(MAX_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .is_err()
        {
            return unsafe_();
        }
        Observed {
            revision: revision(&bytes),
            disk: parse(&bytes),
        }
    }

    /// The effective settings. Also clears an earlier uncertain write: this is the reload.
    pub fn snapshot(&self) -> Snapshot {
        let mut inner = self.inner.lock().expect("settings state poisoned");
        inner.uncertain = false;
        snapshot_of(&self.read())
    }

    pub fn update(&self, expected: &str, change: Change) -> Result<Snapshot, Error> {
        let mut inner = self.inner.lock().expect("settings state poisoned");
        if inner.uncertain {
            return Err(Error::Uncertain);
        }
        let current = self.read();
        if current.revision != expected {
            return Err(Error::Conflict);
        }
        let mut map = match (&change, &current.disk) {
            (_, Disk::Unsafe) => return Err(Error::Unavailable),
            (Change::Recover, Disk::Invalid | Disk::Newer) => {
                self.backup()?;
                Map::new()
            }
            (Change::Recover, _) => return Err(Error::Invalid),
            (_, Disk::Newer) => return Err(Error::ReadOnly),
            (_, Disk::Invalid) => return Err(Error::FileInvalid),
            (_, Disk::Missing) => Map::new(),
            (_, Disk::Valid(map)) => map.clone(),
        };
        match change {
            Change::Recover => {}
            Change::Writing(raw) => {
                let tag = canonical_tag(&raw).ok_or(Error::Invalid)?;
                map.insert("writing".into(), json!({ "locale": tag }));
            }
            Change::InitWriting(preferred) => {
                // Any saved section, even an invalid one, is the user's data: never replace it.
                if map.contains_key("writing") {
                    return Ok(snapshot_of(&current));
                }
                let tag = preferred
                    .iter()
                    .take(32)
                    .find_map(|p| canonical_tag(p))
                    .ok_or(Error::Unresolved)?;
                map.insert("writing".into(), json!({ "locale": tag }));
            }
            Change::Keybindings(value) => {
                if !valid_keybindings(&value) {
                    return Err(Error::Invalid);
                }
                map.insert("keybindings".into(), value);
            }
        }
        let mut object = Map::new();
        object.insert("schema_version".into(), json!(1));
        object.extend(map);
        let mut bytes =
            serde_json::to_vec_pretty(&Value::Object(object)).map_err(|_| Error::Invalid)?;
        bytes.push(b'\n');
        if bytes.len() > MAX_BYTES {
            return Err(Error::Invalid);
        }
        self.publish(&mut inner, &bytes)?;
        // Read back what is on disk; memory never runs ahead of a confirmed publication.
        Ok(snapshot_of(&self.read()))
    }

    fn open_for_write(&self) -> Result<PrivateDir, Error> {
        if !self.dir.is_dir() {
            std::fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(&self.dir)
                .map_err(|_| Error::Unavailable)?;
        }
        PrivateDir::open(&self.dir).map_err(|_| Error::Unavailable)
    }
    fn backup(&self) -> Result<(), Error> {
        let dir = self.open_for_write()?;
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let mut source = dir.read(FILE).map_err(|_| Error::Unavailable)?;
        for n in 0..100 {
            let name = format!("{FILE}.bak-{secs}-{n}");
            if let Ok(mut target) = dir.create(&name) {
                std::io::copy(&mut source, &mut target).map_err(|_| Error::Unavailable)?;
                target.sync_all().map_err(|_| Error::Unavailable)?;
                return dir.sync().map_err(|_| Error::Unavailable);
            }
        }
        Err(Error::Unavailable)
    }
    fn publish(&self, inner: &mut Inner, bytes: &[u8]) -> Result<(), Error> {
        let dir = self.open_for_write()?;
        let _ = dir.remove(TEMP);
        let abandon = |dir: &PrivateDir| {
            let _ = dir.remove(TEMP);
            Error::Unavailable
        };
        let mut file = dir.create(TEMP).map_err(|_| Error::Unavailable)?;
        if file.write_all(bytes).is_err() || file.sync_all().is_err() {
            return Err(abandon(&dir));
        }
        #[cfg(test)]
        if inner.fault == Fault::BeforeRename {
            return Err(abandon(&dir));
        }
        if dir.rename(TEMP, FILE, false).is_err() {
            return Err(abandon(&dir));
        }
        #[cfg(test)]
        let forced = inner.fault == Fault::AfterRename;
        #[cfg(not(test))]
        let forced = false;
        if forced || dir.sync().is_err() {
            inner.uncertain = true;
            return Err(Error::Uncertain);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;

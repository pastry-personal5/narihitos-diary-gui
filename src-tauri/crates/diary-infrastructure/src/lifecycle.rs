//! OS lock, bounded loopback listener and private ephemeral discovery.
use crate::config::{ConfigError, open_private};
use diary_logging::private_fs::{FsError, PrivateDir};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{Read, Write},
    net::{Ipv4Addr, SocketAddrV4, TcpListener},
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleError {
    UnsafePath,
    WorkspaceUnavailable,
    WorkspaceBusy,
    /// No `.diary` and the directory holds other files: inspect before initializing.
    RootNotEmpty,
    Io,
    InvalidPorts,
    PortsExhausted,
    DiscoveryInvalid,
    Entropy,
}

/// Fills `buf` (at most 256 bytes) from the OS entropy source.
pub fn entropy(buf: &mut [u8]) -> Result<(), LifecycleError> {
    // SAFETY: writable buffer of specified length; getentropy limits requests to 256 bytes.
    if buf.len() > 256 || unsafe { libc::getentropy(buf.as_mut_ptr().cast(), buf.len()) } != 0 {
        return Err(LifecycleError::Entropy);
    }
    Ok(())
}

/// Random (version 4) lowercase hyphenated UUID from OS entropy.
pub fn random_uuid() -> Result<String, LifecycleError> {
    let mut b = [0u8; 16];
    entropy(&mut b)?;
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    Ok(format!(
        "{}-{}-{}-{}-{}",
        hex(&b[..4]),
        hex(&b[4..6]),
        hex(&b[6..8]),
        hex(&b[8..10]),
        hex(&b[10..])
    ))
}

/// Stable workspace identifier: SHA-256 of the canonical entries-root path.
pub fn workspace_key(canonical: &Path) -> String {
    use std::os::unix::ffi::OsStrExt;
    hex(&Sha256::digest(canonical.as_os_str().as_bytes()))
}

pub struct WorkspaceLock {
    _file: File,
    pub workspace_key: String,
    pub root: PathBuf,
}
impl WorkspaceLock {
    /// Takes the exclusive workspace lock. A root without `.diary` gets one (and the lock file)
    /// only when it is empty, so a real diary directory is never half-initialized.
    pub fn acquire(workspace: &Path) -> Result<Self, LifecycleError> {
        use std::os::unix::fs::DirBuilderExt;
        let canonical =
            fs::canonicalize(workspace).map_err(|_| LifecycleError::WorkspaceUnavailable)?;
        if canonical != workspace {
            return Err(LifecycleError::UnsafePath);
        }
        let diary = workspace.join(".diary");
        match fs::symlink_metadata(&diary) {
            Ok(_) => (),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let empty = fs::read_dir(workspace)
                    .map_err(|_| LifecycleError::WorkspaceUnavailable)?
                    .all(|e| e.is_ok_and(|e| e.file_name() == ".DS_Store"));
                if !empty {
                    return Err(LifecycleError::RootNotEmpty);
                }
                fs::DirBuilder::new()
                    .mode(0o700)
                    .create(&diary)
                    .map_err(|_| LifecycleError::Io)?;
                File::open(workspace)
                    .and_then(|d| d.sync_all())
                    .map_err(|_| LifecycleError::Io)?;
            }
            Err(_) => return Err(LifecycleError::WorkspaceUnavailable),
        }
        let file = PrivateDir::open(&diary)?
            .lock("workspace.lock")
            .map_err(|e| match e {
                FsError::Busy => LifecycleError::WorkspaceBusy,
                _ => LifecycleError::UnsafePath,
            })?;
        let workspace_key = workspace_key(&canonical);
        Ok(Self {
            _file: file,
            workspace_key,
            root: canonical,
        })
    }
}

pub fn bind_loopback(port: u16, fallback_count: u8) -> Result<TcpListener, LifecycleError> {
    if port < 1024 || fallback_count > 8 || port.checked_add(u16::from(fallback_count)).is_none() {
        return Err(LifecycleError::InvalidPorts);
    }
    for offset in 0..=u16::from(fallback_count) {
        match TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port + offset)) {
            Ok(listener) => return Ok(listener),
            Err(error) if error.kind() == std::io::ErrorKind::AddrInUse => (),
            Err(_) => return Err(LifecycleError::Io),
        }
    }
    Err(LifecycleError::PortsExhausted)
}

// No Debug: this contains the API credential; only redacted typed DTOs cross IPC.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Discovery {
    pub schema_version: u32,
    pub instance_id: String,
    pub pid: u32,
    pub workspace_key: String,
    pub api_version: u32,
    pub origin: String,
    pub token: String,
}
impl Discovery {
    pub fn fresh(workspace_key: &str, port: u16) -> Result<Self, LifecycleError> {
        let mut bytes = [0u8; 48];
        entropy(&mut bytes)?;
        bytes[6] = (bytes[6] & 0x0f) | 0x40;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        let instance_id = format!(
            "{}-{}-{}-{}-{}",
            hex(&bytes[..4]),
            hex(&bytes[4..6]),
            hex(&bytes[6..8]),
            hex(&bytes[8..10]),
            hex(&bytes[10..16])
        );
        let record = Self {
            schema_version: 1,
            instance_id,
            pid: std::process::id(),
            workspace_key: workspace_key.into(),
            api_version: 1,
            origin: format!("http://127.0.0.1:{port}"),
            token: hex(&bytes[16..]),
        };
        record.validate()?;
        Ok(record)
    }
    pub fn validate(&self) -> Result<(), LifecycleError> {
        let valid_hex = |s: &str, n| {
            s.len() == n
                && s.bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        };
        let uuid_parts: Vec<_> = self.instance_id.split('-').collect();
        let uuid_valid = uuid_parts.len() == 5
            && uuid_parts
                .iter()
                .zip([8, 4, 4, 4, 12])
                .all(|(part, len)| valid_hex(part, len));
        let port = self
            .origin
            .strip_prefix("http://127.0.0.1:")
            .and_then(|p| p.parse::<u16>().ok());
        if self.schema_version != 1
            || self.api_version != 1
            || self.pid == 0
            || !uuid_valid
            || !valid_hex(&self.workspace_key, 64)
            || !valid_hex(&self.token, 64)
            || !port.is_some_and(|p| p >= 1024 && self.origin == format!("http://127.0.0.1:{p}"))
        {
            return Err(LifecycleError::DiscoveryInvalid);
        }
        Ok(())
    }
}

pub fn read_discovery(path: &Path) -> Result<Discovery, LifecycleError> {
    let (mut file, _) = open_private(path).map_err(|_| LifecycleError::UnsafePath)?;
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(16 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| LifecycleError::Io)?;
    if bytes.len() > 16 * 1024 {
        return Err(LifecycleError::DiscoveryInvalid);
    }
    let record: Discovery =
        serde_json::from_slice(&bytes).map_err(|_| LifecycleError::DiscoveryInvalid)?;
    record.validate()?;
    Ok(record)
}

pub struct PublishedDiscovery {
    pub path: PathBuf,
    instance_id: String,
    directory: diary_logging::private_fs::PrivateDir,
}
impl PublishedDiscovery {
    /// Call only after listener/controller readiness, holding the workspace lock.
    /// Runtime root must already be private; never silently repair its modes.
    pub fn publish(
        runtime: &Path,
        record: &Discovery,
        lock: &WorkspaceLock,
    ) -> Result<Self, LifecycleError> {
        record.validate()?;
        if record.workspace_key != lock.workspace_key {
            return Err(LifecycleError::DiscoveryInvalid);
        }
        let runtime_dir = diary_logging::private_fs::PrivateDir::open(runtime)?;
        let directory = runtime_dir.child(&record.workspace_key)?;
        let staging = format!(".{}.tmp", record.instance_id);
        let mut file = directory.create(&staging)?;
        let result = (|| {
            let bytes = serde_json::to_vec(record).map_err(|_| LifecycleError::DiscoveryInvalid)?;
            file.write_all(&bytes).map_err(|_| LifecycleError::Io)?;
            file.sync_all().map_err(|_| LifecycleError::Io)?;
            directory
                .rename(&staging, "discovery.json", false)
                .map_err(|_| LifecycleError::Io)?;
            directory.sync().map_err(|_| LifecycleError::Io)?;
            Ok(())
        })();
        if let Err(error) = result {
            let _ = directory.remove(&staging);
            return Err(error);
        }
        Ok(Self {
            path: runtime.join(&record.workspace_key).join("discovery.json"),
            instance_id: record.instance_id.clone(),
            directory,
        })
    }
}
impl Drop for PublishedDiscovery {
    fn drop(&mut self) {
        // Move the exact record into an exclusive private cleanup slot before
        // inspecting it, so a replacement at the public name is never unlinked.
        let cleanup = format!(".{}.cleanup", self.instance_id);
        if self
            .directory
            .rename("discovery.json", &cleanup, true)
            .is_err()
        {
            return;
        }
        let own = (|| {
            let file = self.directory.read(&cleanup).ok()?;
            let mut bytes = Vec::new();
            file.take(16385).read_to_end(&mut bytes).ok()?;
            if bytes.len() > 16384 {
                return None;
            }
            let record: Discovery = serde_json::from_slice(&bytes).ok()?;
            record.validate().ok()?;
            Some(record.instance_id == self.instance_id)
        })()
        .unwrap_or(false);
        if own {
            let _ = self.directory.remove(&cleanup);
        } else {
            // Never clobber a third record installed while the check ran. If
            // restoration loses that race, preserve the moved record privately.
            let _ = self.directory.rename(&cleanup, "discovery.json", true);
        }
        let _ = self.directory.sync();
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

impl From<diary_logging::private_fs::FsError> for LifecycleError {
    fn from(e: diary_logging::private_fs::FsError) -> Self {
        ConfigError::from(e).into()
    }
}
impl From<ConfigError> for LifecycleError {
    fn from(_: ConfigError) -> Self {
        Self::UnsafePath
    }
}

#[cfg(test)]
mod tests;

use serde::Deserialize;
use std::{
    fs::{File, OpenOptions},
    io::Read,
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{
            ffi::OsStrExt,
            fs::{MetadataExt, OpenOptionsExt},
        },
    },
    path::{Component, Path},
};
#[derive(Deserialize)]
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
pub fn read(path: &Path) -> Result<Discovery, &'static str> {
    decode(open(path)?)
}
fn open(path: &Path) -> Result<File, &'static str> {
    let path = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()
            .map_err(|_| "discovery_unavailable")?
            .join(path)
    };
    let parts: Vec<_> = path.components().collect();
    if parts.len() < 3 {
        return Err("invalid_discovery");
    }
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open("/")
        .map_err(|_| "discovery_unavailable")?;
    for (index, part) in parts.iter().enumerate().skip(1) {
        let Component::Normal(name) = part else {
            return Err("invalid_discovery");
        };
        let name = std::ffi::CString::new(name.as_bytes()).map_err(|_| "invalid_discovery")?;
        let last = index + 1 == parts.len();
        // SAFETY: live directory descriptor and terminated component. Successful fd is owned immediately.
        let fd = unsafe {
            libc::openat(
                file.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY
                    | libc::O_NOFOLLOW
                    | libc::O_CLOEXEC
                    | libc::O_NONBLOCK
                    | if last { 0 } else { libc::O_DIRECTORY },
            )
        };
        if fd < 0 {
            return Err("discovery_unavailable");
        }
        file = unsafe { File::from_raw_fd(fd) };
        let metadata = file.metadata().map_err(|_| "discovery_unavailable")?;
        if last || index + 2 == parts.len() {
            // SAFETY: geteuid takes no arguments and cannot invalidate metadata.
            if metadata.uid() != unsafe { libc::geteuid() }
                || metadata.mode() & 0o7777 != if last { 0o600 } else { 0o700 }
                || if last {
                    !metadata.is_file()
                } else {
                    !metadata.is_dir()
                }
            {
                return Err("unsafe_discovery");
            }
        }
    }
    Ok(file)
}
fn decode(file: File) -> Result<Discovery, &'static str> {
    let mut bytes = Vec::new();
    file.take(16385)
        .read_to_end(&mut bytes)
        .map_err(|_| "discovery_unavailable")?;
    if bytes.len() > 16384 {
        return Err("invalid_discovery");
    }
    let record: Discovery = serde_json::from_slice(&bytes).map_err(|_| "invalid_discovery")?;
    let port = record
        .origin
        .strip_prefix("http://127.0.0.1:")
        .and_then(|p| p.parse::<u16>().ok());
    let hex = |s: &str| {
        s.len() == 64
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    };
    if record.schema_version != 1
        || record.api_version != 1
        || record.pid == 0
        || uuid::Uuid::parse_str(&record.instance_id).is_err()
        || !hex(&record.workspace_key)
        || !hex(&record.token)
        || !port.is_some_and(|p| p >= 1024 && record.origin == format!("http://127.0.0.1:{p}"))
    {
        return Err("invalid_discovery");
    }
    Ok(record)
}

#[cfg(test)]
mod tests;

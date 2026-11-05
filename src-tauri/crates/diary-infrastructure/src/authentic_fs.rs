//! Durable, symlink-refusing file primitives for the Authentic tree on macOS. Directory and
//! file permissions are private (0700 / 0600). Syncs use `F_FULLFSYNC`, which asks the drive to
//! flush its cache; plain `fsync` on macOS does not.
use std::{
    ffi::CString,
    fs::{DirBuilder, File, OpenOptions},
    io::{self, Read, Write},
    os::unix::{
        ffi::OsStrExt,
        fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
        io::AsRawFd,
    },
    path::Path,
};

/// What a read saw: bytes plus the identity and size they came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observed {
    pub bytes: Vec<u8>,
    pub dev: u64,
    pub ino: u64,
    pub mtime_ns: i64,
}

fn no_follow() -> i32 {
    libc::O_NOFOLLOW | libc::O_CLOEXEC
}

/// Flushes the file or directory to stable storage.
pub fn full_sync(file: &File) -> io::Result<()> {
    // SAFETY: the descriptor is owned by `file` for the duration of the call.
    if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_FULLFSYNC) } == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

pub fn sync_path(path: &Path) -> io::Result<()> {
    full_sync(
        &OpenOptions::new()
            .read(true)
            .custom_flags(no_follow())
            .open(path)?,
    )
}

/// Creates every missing directory below `root` (0700) and syncs each new parent. A symlink or
/// non-directory in the chain is refused.
pub fn ensure_dirs(root: &Path, relative: &[&str]) -> io::Result<()> {
    let mut current = root.to_path_buf();
    for part in relative {
        let next = current.join(part);
        match std::fs::symlink_metadata(&next) {
            Ok(m) if m.is_dir() => {}
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "not a directory",
                ));
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                DirBuilder::new().mode(0o700).create(&next)?;
                sync_path(&next)?;
                sync_path(&current)?;
            }
            Err(e) => return Err(e),
        }
        current = next;
    }
    Ok(())
}

/// Exclusive 0600 create and write; fails if the name exists. Not yet synced.
pub fn create_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(no_follow())
        .open(path)?;
    file.write_all(bytes)
}

pub fn sync_file(path: &Path) -> io::Result<()> {
    sync_path(path)
}

/// Publishes `from` at `to` only if `to` is absent (`renamex_np` with `RENAME_EXCL`).
/// `AlreadyExists` when it is not.
pub fn rename_new(from: &Path, to: &Path) -> io::Result<()> {
    let from = CString::new(from.as_os_str().as_bytes())?;
    let to = CString::new(to.as_os_str().as_bytes())?;
    // SAFETY: both pointers are valid NUL-terminated strings for the duration of the call.
    if unsafe { libc::renamex_np(from.as_ptr(), to.as_ptr(), libc::RENAME_EXCL) } == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Atomically replaces `to` with `from` on the same volume.
pub fn rename_replace(from: &Path, to: &Path) -> io::Result<()> {
    std::fs::rename(from, to)
}

pub fn remove(path: &Path) -> io::Result<()> {
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// Reads a regular file without following symlinks; `None` when absent. Larger than `max`
/// bytes is `InvalidData`, never a truncated read.
pub fn read_bounded(path: &Path, max: u64) -> io::Result<Option<Observed>> {
    let mut file = match OpenOptions::new()
        .read(true)
        .custom_flags(no_follow())
        .open(path)
    {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let meta = file.metadata()?;
    if !meta.is_file() || meta.len() > max {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "not a bounded regular file",
        ));
    }
    let mut bytes = Vec::with_capacity(meta.len() as usize);
    (&mut file).take(max + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "file grew while reading",
        ));
    }
    Ok(Some(Observed {
        bytes,
        dev: meta.dev(),
        ino: meta.ino(),
        mtime_ns: meta.mtime() * 1_000_000_000 + meta.mtime_nsec(),
    }))
}

#[cfg(test)]
mod tests;

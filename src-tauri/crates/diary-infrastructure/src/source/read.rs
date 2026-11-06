use super::root::ResolvedRoot;
pub use diary_application::source::Unavailable as ReadError;
use diary_domain::source::{
    front_matter::MAX_FILE,
    path::{SourcePath, normalize},
};
use sha2::{Digest, Sha256};
use std::os::macos::fs::MetadataExt as DarwinMetadataExt;
use std::{
    ffi::CString,
    fmt,
    fs::File,
    io::{self, Read},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{ffi::OsStrExt, fs::MetadataExt},
    },
    path::{Component, Path},
};
pub const SF_DATALESS: u32 = 0x4000_0000;
pub struct Observation {
    pub bytes: Vec<u8>,
    pub sha256: [u8; 32],
    pub size: u64,
    pub dev: u64,
    pub ino: u64,
    pub nlink: u64,
    pub mtime_ns: i128,
    pub ctime_ns: i128,
}
impl fmt::Debug for Observation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Observation")
            .field("size", &self.size)
            .field("digest_prefix", &&self.sha256[..4])
            .finish_non_exhaustive()
    }
}
fn error(e: io::Error) -> ReadError {
    match e.raw_os_error() {
        Some(libc::ENOENT) => ReadError::Missing,
        Some(libc::EACCES | libc::EPERM) => ReadError::Denied,
        Some(libc::ELOOP | libc::ENOTDIR) => ReadError::NotRegular,
        _ => ReadError::Io,
    }
}
pub(crate) fn open_at(
    parent: i32,
    name: &std::ffi::OsStr,
    directory: bool,
) -> Result<File, ReadError> {
    open_component(
        parent,
        name,
        libc::O_RDONLY | if directory { libc::O_DIRECTORY } else { 0 },
    )
}
fn open_component(parent: i32, name: &std::ffi::OsStr, access: i32) -> Result<File, ReadError> {
    let name = CString::new(name.as_bytes()).map_err(|_| ReadError::InvalidPath)?;
    loop {
        // SAFETY: live parent descriptor, NUL-terminated component. Returned descriptor is owned once.
        let fd = unsafe {
            libc::openat(
                parent,
                name.as_ptr(),
                access | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
            )
        };
        if fd >= 0 {
            return Ok(unsafe { File::from_raw_fd(fd) });
        }
        let e = io::Error::last_os_error();
        if e.kind() != io::ErrorKind::Interrupted {
            return Err(error(e));
        }
    }
}
/// Lookup of a known private receipt needs directory search permission, not permission
/// to enumerate Trash. O_SEARCH still checks traversal permissions and O_NOFOLLOW.
pub(crate) fn open_root_lookup(path: &Path) -> Result<File, ReadError> {
    if !path.is_absolute() {
        return Err(ReadError::InvalidPath);
    }
    let mut root = File::open("/").map_err(error)?;
    for part in path.components() {
        match part {
            Component::RootDir => {}
            Component::Normal(p) => {
                root = open_component(root.as_raw_fd(), p, libc::O_SEARCH)?;
                if !root.metadata().map_err(error)?.is_dir() {
                    return Err(ReadError::NotRegular);
                }
            }
            _ => return Err(ReadError::InvalidPath),
        }
    }
    Ok(root)
}
pub(crate) fn open_root(path: &Path) -> Result<File, ReadError> {
    let mut root = File::open("/").map_err(error)?;
    for part in path.components() {
        match part {
            Component::RootDir => {}
            Component::Normal(p) => root = open_at(root.as_raw_fd(), p, true)?,
            _ => return Err(ReadError::InvalidPath),
        }
    }
    Ok(root)
}
pub(crate) fn validated_root(root: &ResolvedRoot) -> Result<File, ReadError> {
    let file = open_root(&root.canonical)?;
    let m = file.metadata().map_err(error)?;
    if (m.dev(), m.ino()) != (root.dev, root.ino) {
        return Err(ReadError::Missing);
    }
    Ok(file)
}
fn signature(m: &std::fs::Metadata) -> (u64, u64, u64, i64, i64, i64, i64) {
    (
        m.dev(),
        m.ino(),
        m.size(),
        m.mtime(),
        m.mtime_nsec(),
        m.ctime(),
        m.ctime_nsec(),
    )
}

/// Open one member through no-follow descriptors, including every ancestor.
pub(crate) fn open_member(root: &ResolvedRoot, rel: &SourcePath) -> Result<File, ReadError> {
    if normalize(&rel.spelling, root.insensitive).as_ref() != Ok(rel) {
        return Err(ReadError::InvalidPath);
    }
    let mut parent = open_root_lookup(&root.canonical)?;
    let metadata = parent.metadata().map_err(error)?;
    if (metadata.dev(), metadata.ino()) != (root.dev, root.ino) {
        return Err(ReadError::Missing);
    }
    check_flags(metadata.st_flags())?;
    let parts: Vec<_> = rel.spelling.split('/').collect();
    for p in &parts[..parts.len() - 1] {
        parent = open_component(parent.as_raw_fd(), p.as_ref(), libc::O_SEARCH)?;
        let m = parent.metadata().map_err(error)?;
        if m.dev() != root.dev {
            return Err(ReadError::NotRegular);
        }
        check_flags(m.st_flags())?;
    }
    open_at(parent.as_raw_fd(), parts[parts.len() - 1].as_ref(), false)
}

/// Full identity/digest without retaining audio bytes in memory or diagnostics.
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileDigest {
    pub sha256: [u8; 32],
    pub size: u64,
    pub dev: u64,
    pub ino: u64,
    pub mtime_ns: i128,
    pub ctime_ns: i128,
}
impl fmt::Debug for FileDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileDigest")
            .field("size", &self.size)
            .finish_non_exhaustive()
    }
}

/// Bounded streaming read for a mutation member. The caller supplies its preparation
/// deadline/cancellation predicate; cancellation never returns a partial digest.
pub fn digest_member(
    root: &ResolvedRoot,
    rel: &SourcePath,
    max_bytes: u64,
    cancelled: &dyn Fn() -> bool,
) -> Result<FileDigest, ReadError> {
    for attempt in 0..2 {
        if cancelled() {
            return Err(ReadError::SourceChanging);
        }
        let mut file = open_member(root, rel)?;
        let before = file.metadata().map_err(error)?;
        if !before.is_file() || before.dev() != root.dev || before.nlink() != 1 {
            return Err(ReadError::NotRegular);
        }
        check_flags(before.st_flags())?;
        if before.size() > max_bytes {
            return Err(ReadError::TooLarge);
        }
        let mut hash = Sha256::new();
        let mut total = 0u64;
        let mut chunk = [0u8; 64 * 1024];
        loop {
            if cancelled() {
                return Err(ReadError::SourceChanging);
            }
            let n = match file.read(&mut chunk) {
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                other => other.map_err(error)?,
            };
            if n == 0 {
                break;
            }
            total += n as u64;
            if total > max_bytes {
                return Err(ReadError::TooLarge);
            }
            hash.update(&chunk[..n]);
        }
        let current = open_member(root, rel)?.metadata().map_err(error)?;
        let after = file.metadata().map_err(error)?;
        if signature(&before) != signature(&after)
            || signature(&before) != signature(&current)
            || after.nlink() != 1
            || current.nlink() != 1
            || total != before.size()
        {
            if attempt == 0 {
                continue;
            }
            return Err(ReadError::SourceChanging);
        }
        check_flags(after.st_flags())?;
        check_flags(current.st_flags())?;
        return Ok(FileDigest {
            sha256: hash.finalize().into(),
            size: total,
            dev: before.dev(),
            ino: before.ino(),
            mtime_ns: i128::from(before.mtime()) * 1_000_000_000 + i128::from(before.mtime_nsec()),
            ctime_ns: i128::from(before.ctime()) * 1_000_000_000 + i128::from(before.ctime_nsec()),
        });
    }
    Err(ReadError::SourceChanging)
}
pub fn read_fresh(root: &ResolvedRoot, rel: &SourcePath) -> Result<Observation, ReadError> {
    read_with(root, rel, || {})
}
fn read_with(
    root: &ResolvedRoot,
    rel: &SourcePath,
    mut between: impl FnMut(),
) -> Result<Observation, ReadError> {
    if normalize(&rel.spelling, root.insensitive).as_ref() != Ok(rel) {
        return Err(ReadError::InvalidPath);
    }
    for attempt in 0..2 {
        let mut parent = validated_root(root)?;
        let parts: Vec<_> = rel.spelling.split('/').collect();
        for p in &parts[..parts.len() - 1] {
            parent = open_at(parent.as_raw_fd(), p.as_ref(), true)?;
            if parent.metadata().map_err(error)?.dev() != root.dev {
                return Err(ReadError::NotRegular);
            }
        }
        let name = parts[parts.len() - 1];
        let mut file = open_at(parent.as_raw_fd(), name.as_ref(), false)?;
        let before = file.metadata().map_err(error)?;
        if !before.is_file() || before.dev() != root.dev {
            return Err(ReadError::NotRegular);
        }
        check_flags(before.st_flags())?;
        if before.size() > MAX_FILE as u64 {
            return Err(ReadError::TooLarge);
        }
        let mut bytes = Vec::new();
        (&mut file)
            .take(MAX_FILE as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(error)?;
        between();
        // Reopen through the root as well: this catches renamed/replaced directory ancestors.
        let current = (|| {
            let mut p = validated_root(root)?;
            for component in &parts[..parts.len() - 1] {
                p = open_at(p.as_raw_fd(), component.as_ref(), true)?;
            }
            open_at(p.as_raw_fd(), name.as_ref(), false)?
                .metadata()
                .map_err(error)
        })();
        let after = file.metadata().map_err(error);
        let stable = current
            .as_ref()
            .is_ok_and(|m| signature(m) == signature(&before))
            && after
                .as_ref()
                .is_ok_and(|m| signature(m) == signature(&before))
            && bytes.len() as u64 == before.size()
            && bytes.len() <= MAX_FILE;
        if !stable {
            if attempt == 0 {
                continue;
            }
            return Err(ReadError::SourceChanging);
        }
        return Ok(Observation {
            sha256: Sha256::digest(&bytes).into(),
            bytes,
            size: before.size(),
            dev: before.dev(),
            ino: before.ino(),
            nlink: before.nlink(),
            mtime_ns: i128::from(before.mtime()) * 1_000_000_000 + i128::from(before.mtime_nsec()),
            ctime_ns: i128::from(before.ctime()) * 1_000_000_000 + i128::from(before.ctime_nsec()),
        });
    }
    Err(ReadError::SourceChanging)
}

fn check_flags(flags: u32) -> Result<(), ReadError> {
    if flags & SF_DATALESS != 0 {
        Err(ReadError::NotDownloaded)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::tests::{AYUMI, Tree, ayumi};
    use std::{fs, io::Write, os::unix::fs::symlink};
    #[test]
    fn fresh_read_retries_once_then_detects_continuous_changes() {
        let tree = Tree::new();
        tree.write(AYUMI, &ayumi());
        let path = normalize(AYUMI, tree.root.insensitive).unwrap();
        let mut calls = 0;
        let obs = read_with(&tree.root, &path, || {
            calls += 1;
            if calls == 1 {
                fs::OpenOptions::new()
                    .append(true)
                    .open(tree.temp.path().join(AYUMI))
                    .unwrap()
                    .write_all(b"once")
                    .unwrap();
            }
        })
        .unwrap();
        assert_eq!(calls, 2);
        assert!(obs.bytes.ends_with(b"once"));
        let err = read_with(&tree.root, &path, || {
            fs::OpenOptions::new()
                .append(true)
                .open(tree.temp.path().join(AYUMI))
                .unwrap()
                .write_all(b"always")
                .unwrap();
        })
        .unwrap_err();
        assert_eq!(err, ReadError::SourceChanging);
    }
    #[test]
    fn rename_replace_and_symlink_swap_never_return_old_observation() {
        let tree = Tree::new();
        tree.write(AYUMI, &ayumi());
        let path = normalize(AYUMI, tree.root.insensitive).unwrap();
        let mut once = false;
        let obs = read_with(&tree.root, &path, || {
            if !once {
                once = true;
                tree.write("replacement", b"new bytes");
                fs::rename(
                    tree.temp.path().join("replacement"),
                    tree.temp.path().join(AYUMI),
                )
                .unwrap();
            }
        })
        .unwrap();
        assert_eq!(obs.bytes, b"new bytes");
        let outside = Tree::new();
        outside.write("secret", b"MUST_NOT_READ");
        let result = read_with(&tree.root, &path, || {
            if tree
                .temp
                .path()
                .join(AYUMI)
                .symlink_metadata()
                .unwrap()
                .is_file()
            {
                fs::remove_file(tree.temp.path().join(AYUMI)).unwrap();
                symlink(
                    outside.temp.path().join("secret"),
                    tree.temp.path().join(AYUMI),
                )
                .unwrap();
            }
        });
        assert_eq!(result.unwrap_err(), ReadError::NotRegular);
    }
    #[test]
    fn dataless_decision_and_replaced_root() {
        assert_eq!(check_flags(SF_DATALESS), Err(ReadError::NotDownloaded));
        assert_eq!(check_flags(0), Ok(()));
        let tree = Tree::new();
        tree.write(AYUMI, &ayumi());
        let path = normalize(AYUMI, tree.root.insensitive).unwrap();
        let moved = tree.temp.path().with_extension("moved");
        fs::rename(tree.temp.path(), &moved).unwrap();
        fs::create_dir(tree.temp.path()).unwrap();
        assert_eq!(
            read_fresh(&tree.root, &path).unwrap_err(),
            ReadError::Missing
        );
        fs::remove_dir_all(moved).unwrap();
    }
    #[test]
    fn known_member_lookup_does_not_require_directory_enumeration_and_never_follows_links() {
        use std::os::unix::fs::PermissionsExt;
        let tree = Tree::new();
        tree.write("receipt", b"known receipt");
        let path = normalize("receipt", tree.root.insensitive).unwrap();
        fs::set_permissions(tree.temp.path(), fs::Permissions::from_mode(0o300)).unwrap();
        let result = digest_member(&tree.root, &path, 100, &|| false);
        fs::set_permissions(tree.temp.path(), fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(
            result.unwrap().sha256,
            <[u8; 32]>::from(Sha256::digest(b"known receipt"))
        );
        let other = Tree::new();
        other.write("outside", b"outside");
        symlink(other.temp.path(), tree.temp.path().join("link")).unwrap();
        let path = normalize("link/outside", tree.root.insensitive).unwrap();
        assert_eq!(
            digest_member(&tree.root, &path, 100, &|| false).unwrap_err(),
            ReadError::NotRegular
        );
    }
}

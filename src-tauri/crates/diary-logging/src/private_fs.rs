//! Descriptor-relative private filesystem operations; never follow swapped ancestors.
use std::{
    ffi::CString,
    fs::{File, OpenOptions},
    io,
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{ffi::OsStrExt, fs::OpenOptionsExt},
    },
    path::{Component, Path},
};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsError {
    Path,
    Permissions,
    Io,
    /// Another holder has the advisory lock.
    Busy,
}

pub fn check_private_metadata(
    metadata: &std::fs::Metadata,
    directory: bool,
) -> Result<(), FsError> {
    use std::os::unix::fs::MetadataExt;
    // SAFETY: geteuid has no preconditions and does not mutate process state.
    let uid = unsafe { libc::geteuid() };
    private_attributes(metadata.uid(), uid, metadata.mode(), directory)
}

fn private_attributes(
    owner: u32,
    current_user: u32,
    mode: u32,
    directory: bool,
) -> Result<(), FsError> {
    let expected_kind = if directory {
        libc::S_IFDIR
    } else {
        libc::S_IFREG
    };
    if owner != current_user
        || mode & 0o7777 != if directory { 0o700 } else { 0o600 }
        || mode & u32::from(libc::S_IFMT) != u32::from(expected_kind)
    {
        return Err(FsError::Permissions);
    }
    Ok(())
}

/// Size and modification time of an opened regular file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileMeta {
    pub len: u64,
    pub modified_epoch_secs: i64,
}

pub struct PrivateDir {
    file: File,
}
fn name(value: &str) -> io::Result<CString> {
    if value.is_empty() || value == "." || value == ".." || value.contains('/') {
        return Err(io::Error::from(io::ErrorKind::InvalidInput));
    }
    CString::new(value).map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))
}
fn owned(fd: libc::c_int) -> io::Result<File> {
    if fd < 0 {
        Err(io::Error::last_os_error())
    } else {
        // SAFETY: a successful syscall returns a new descriptor, owned exactly once.
        Ok(unsafe { File::from_raw_fd(fd) })
    }
}
impl PrivateDir {
    pub fn open(path: &Path) -> Result<Self, FsError> {
        if !path.is_absolute() {
            return Err(FsError::Path);
        }
        let mut file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open("/")
            .map_err(|_| FsError::Io)?;
        for part in path.components().skip(1) {
            let Component::Normal(part) = part else {
                return Err(FsError::Path);
            };
            let part = CString::new(part.as_bytes()).map_err(|_| FsError::Path)?;
            // SAFETY: live parent fd and terminated component; owned() captures the result.
            file = owned(unsafe {
                libc::openat(
                    file.as_raw_fd(),
                    part.as_ptr(),
                    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                )
            })
            .map_err(|_| FsError::Permissions)?;
        }
        check_private_metadata(&file.metadata().map_err(|_| FsError::Io)?, true)?;
        Ok(Self { file })
    }
    pub fn child(&self, child: &str) -> Result<Self, FsError> {
        let child = name(child).map_err(|_| FsError::Path)?;
        // SAFETY: live parent and terminated single-component name.
        let result = unsafe { libc::mkdirat(self.file.as_raw_fd(), child.as_ptr(), 0o700) };
        if result != 0 && io::Error::last_os_error().kind() != io::ErrorKind::AlreadyExists {
            return Err(FsError::Io);
        }
        let file = owned(unsafe {
            libc::openat(
                self.file.as_raw_fd(),
                child.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        })
        .map_err(|_| FsError::Permissions)?;
        check_private_metadata(&file.metadata().map_err(|_| FsError::Io)?, true)?;
        Ok(Self { file })
    }
    pub fn read(&self, value: &str) -> Result<File, FsError> {
        self.file(value, false)
    }
    pub fn create(&self, value: &str) -> Result<File, FsError> {
        self.file(value, true)
    }
    fn file(&self, value: &str, create: bool) -> Result<File, FsError> {
        let value = name(value).map_err(|_| FsError::Path)?;
        let flags = libc::O_NOFOLLOW
            | libc::O_CLOEXEC
            | libc::O_NONBLOCK
            | if create {
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL
            } else {
                libc::O_RDONLY
            };
        // SAFETY: valid parent fd, component and creation mode; result owned immediately.
        let file =
            owned(unsafe { libc::openat(self.file.as_raw_fd(), value.as_ptr(), flags, 0o600) })
                .map_err(|_| FsError::Permissions)?;
        check_private_metadata(&file.metadata().map_err(|_| FsError::Io)?, false)?;
        Ok(file)
    }
    pub fn rename(&self, from: &str, to: &str, exclusive: bool) -> io::Result<()> {
        let from = name(from)?;
        let to = name(to)?;
        // SAFETY: both names belong to this live directory; flags are defined by macOS.
        let result = unsafe {
            libc::renameatx_np(
                self.file.as_raw_fd(),
                from.as_ptr(),
                self.file.as_raw_fd(),
                to.as_ptr(),
                if exclusive { libc::RENAME_EXCL } else { 0 },
            )
        };
        if result == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }
    pub fn remove(&self, value: &str) -> io::Result<()> {
        let value = name(value)?;
        // SAFETY: live directory and terminated component, flags forbid directory removal.
        if unsafe { libc::unlinkat(self.file.as_raw_fd(), value.as_ptr(), 0) } == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }
    /// Entry names (no "." or ".."), sorted. Names only: callers must open each entry
    /// through this directory so a swapped or hostile entry is judged on the opened object.
    pub fn list(&self) -> Result<Vec<String>, FsError> {
        // SAFETY: dup of a live descriptor; fdopendir takes ownership of the duplicate.
        let fd = unsafe { libc::dup(self.file.as_raw_fd()) };
        if fd < 0 {
            return Err(FsError::Io);
        }
        // SAFETY: fd is a fresh directory descriptor.
        let dir = unsafe { libc::fdopendir(fd) };
        if dir.is_null() {
            // SAFETY: fdopendir failed, so fd is still ours.
            unsafe { libc::close(fd) };
            return Err(FsError::Io);
        }
        // The duplicate shares its offset with the original; start from the beginning.
        // SAFETY: dir is a live DIR*.
        unsafe { libc::rewinddir(dir) };
        let mut names = Vec::new();
        loop {
            // SAFETY: dir is live; the returned entry is valid until the next readdir.
            let entry = unsafe { libc::readdir(dir) };
            if entry.is_null() {
                break;
            }
            // SAFETY: d_name is NUL-terminated within the entry.
            let raw = unsafe { std::ffi::CStr::from_ptr((*entry).d_name.as_ptr()) };
            if let Ok(n) = raw.to_str()
                && n != "."
                && n != ".."
            {
                names.push(n.to_owned());
            }
        }
        // SAFETY: closes dir and the duplicate descriptor exactly once.
        unsafe { libc::closedir(dir) };
        names.sort();
        Ok(names)
    }

    fn open_regular(&self, value: &str, flags: libc::c_int) -> Result<File, FsError> {
        let value = name(value).map_err(|_| FsError::Path)?;
        // SAFETY: valid parent fd and component; O_NONBLOCK keeps a fifo from blocking open.
        let file = owned(unsafe {
            libc::openat(
                self.file.as_raw_fd(),
                value.as_ptr(),
                flags | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
                0o600,
            )
        })
        .map_err(|_| FsError::Permissions)?;
        if !file.metadata().map_err(|_| FsError::Io)?.is_file() {
            return Err(FsError::Permissions);
        }
        Ok(file)
    }

    /// Size and mtime of a regular file, judged on the opened object (symlinks, fifos and
    /// other kinds are errors).
    pub fn metadata(&self, value: &str) -> Result<FileMeta, FsError> {
        use std::os::unix::fs::MetadataExt;
        let meta = self
            .open_regular(value, libc::O_RDONLY)?
            .metadata()
            .map_err(|_| FsError::Io)?;
        Ok(FileMeta {
            len: meta.len(),
            modified_epoch_secs: meta.mtime(),
        })
    }

    /// Opens or creates a private (0600, owner-only) regular file for appending.
    pub fn append(&self, value: &str) -> Result<File, FsError> {
        let file = self.open_regular(value, libc::O_WRONLY | libc::O_APPEND | libc::O_CREAT)?;
        check_private_metadata(&file.metadata().map_err(|_| FsError::Io)?, false)?;
        Ok(file)
    }

    /// Opens or creates a private file and takes a non-blocking exclusive advisory lock that
    /// lasts as long as the returned handle. `FsError::Busy` means another holder has it.
    pub fn lock(&self, value: &str) -> Result<File, FsError> {
        let file = self.open_regular(value, libc::O_RDWR | libc::O_CREAT)?;
        check_private_metadata(&file.metadata().map_err(|_| FsError::Io)?, false)?;
        // SAFETY: live descriptor.
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err(
                if io::Error::last_os_error().kind() == io::ErrorKind::WouldBlock {
                    FsError::Busy
                } else {
                    FsError::Io
                },
            );
        }
        Ok(file)
    }

    pub fn sync(&self) -> io::Result<()> {
        self.file.sync_all()
    }
}

#[cfg(test)]
mod tests;

//! Descriptor-relative private filesystem operations; never follow swapped ancestors.
use crate::config::{ConfigError, check_private_metadata};
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
pub(crate) struct PrivateDir {
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
    pub fn open(path: &Path) -> Result<Self, ConfigError> {
        if !path.is_absolute() {
            return Err(ConfigError::Path);
        }
        let mut file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open("/")
            .map_err(|_| ConfigError::Io)?;
        for part in path.components().skip(1) {
            let Component::Normal(part) = part else {
                return Err(ConfigError::Path);
            };
            let part = CString::new(part.as_bytes()).map_err(|_| ConfigError::Path)?;
            // SAFETY: live parent fd and terminated component; owned() captures the result.
            file = owned(unsafe {
                libc::openat(
                    file.as_raw_fd(),
                    part.as_ptr(),
                    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                )
            })
            .map_err(|_| ConfigError::Permissions)?;
        }
        check_private_metadata(&file.metadata().map_err(|_| ConfigError::Io)?, true)?;
        Ok(Self { file })
    }
    pub fn child(&self, child: &str) -> Result<Self, ConfigError> {
        let child = name(child).map_err(|_| ConfigError::Path)?;
        // SAFETY: live parent and terminated single-component name.
        let result = unsafe { libc::mkdirat(self.file.as_raw_fd(), child.as_ptr(), 0o700) };
        if result != 0 && io::Error::last_os_error().kind() != io::ErrorKind::AlreadyExists {
            return Err(ConfigError::Io);
        }
        let file = owned(unsafe {
            libc::openat(
                self.file.as_raw_fd(),
                child.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        })
        .map_err(|_| ConfigError::Permissions)?;
        check_private_metadata(&file.metadata().map_err(|_| ConfigError::Io)?, true)?;
        Ok(Self { file })
    }
    pub fn read(&self, value: &str) -> Result<File, ConfigError> {
        self.file(value, false)
    }
    pub fn create(&self, value: &str) -> Result<File, ConfigError> {
        self.file(value, true)
    }
    fn file(&self, value: &str, create: bool) -> Result<File, ConfigError> {
        let value = name(value).map_err(|_| ConfigError::Path)?;
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
                .map_err(|_| ConfigError::Permissions)?;
        check_private_metadata(&file.metadata().map_err(|_| ConfigError::Io)?, false)?;
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
    pub fn sync(&self) -> io::Result<()> {
        self.file.sync_all()
    }
}

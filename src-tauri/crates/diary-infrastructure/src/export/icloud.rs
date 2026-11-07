//! The macOS iCloud Drive destination: `<root>/YYYY/MM/YYYY-MM-DD.md`, byte-identical to the
//! saved Authentic file. Reads and publications run in `NSFileCoordinator` accessors with a
//! deadline; creation is no-clobber (`renamex_np(RENAME_EXCL)` from a staged same-directory
//! file) and replacement re-checks the authorized observation inside the coordinated write.
//! Coordination is cooperative, so a non-coordinating writer is detected, not prevented.
use super::hex_sha;
use crate::coordination;
use diary_application::export::{
    Applied, ExportAdapter, MAX_SNAPSHOT_BYTES, Observation, Target, Unavailable, Write,
};
use diary_domain::authentic::DiaryDate;
use std::{
    ffi::CString,
    io::{Read as _, Write as _},
    os::{
        macos::fs::MetadataExt,
        unix::{ffi::OsStrExt, fs::OpenOptionsExt},
    },
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};

const OBSERVE_TIMEOUT: Duration = Duration::from_secs(10);
const APPLY_TIMEOUT: Duration = Duration::from_secs(30);
/// `SF_DATALESS`: the file's contents are not materialized locally.
const SF_DATALESS: u32 = 0x4000_0000;

/// What a provider says about one file. Anything but `Local` is never treated as absence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    Local,
    NotDownloaded,
}
type Probe = Arc<dyn Fn(&Path) -> Provider + Send + Sync>;

/// Dataless files and legacy `.name.icloud` placeholders both mean "download needed".
fn system_probe(path: &Path) -> Provider {
    if let Ok(meta) = std::fs::symlink_metadata(path)
        && meta.st_flags() & SF_DATALESS != 0
    {
        return Provider::NotDownloaded;
    }
    match (path.parent(), path.file_name()) {
        (Some(dir), Some(name)) => {
            let mut holder = std::ffi::OsString::from(".");
            holder.push(name);
            holder.push(".icloud");
            if dir.join(holder).exists() {
                Provider::NotDownloaded
            } else {
                Provider::Local
            }
        }
        _ => Provider::Local,
    }
}

pub struct Icloud {
    root: PathBuf,
    probe: Probe,
    access: Arc<Mutex<()>>,
    worker_active: Arc<AtomicBool>,
    observe_timeout: Duration,
    apply_timeout: Duration,
}
impl Icloud {
    /// `None` when the root is missing or not a directory; export then stays `disabled`.
    pub fn new(root: &Path) -> Option<Self> {
        let root = root.canonicalize().ok()?;
        root.is_dir().then_some(Self {
            root,
            probe: Arc::new(system_probe),
            access: Arc::new(Mutex::new(())),
            worker_active: Arc::new(AtomicBool::new(false)),
            observe_timeout: OBSERVE_TIMEOUT,
            apply_timeout: APPLY_TIMEOUT,
        })
    }
    #[cfg(test)]
    fn with(mut self, probe: Probe, observe: Duration, apply: Duration) -> Self {
        (self.probe, self.observe_timeout, self.apply_timeout) = (probe, observe, apply);
        self
    }
    fn dir(&self, date: &DiaryDate) -> PathBuf {
        let text = date.to_string();
        self.root.join(&text[..4]).join(&text[5..7])
    }
    fn path(&self, date: &DiaryDate) -> PathBuf {
        self.dir(date).join(format!("{date}.md"))
    }
    fn stage(path: &Path) -> PathBuf {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        path.with_file_name(format!(".{name}.stage"))
    }
    /// The year and month directories must be real directories under the canonical root.
    fn parents_error(&self, date: &DiaryDate) -> Option<Unavailable> {
        let dir = self.dir(date);
        for p in [&self.root, dir.parent().unwrap_or(&self.root), &dir] {
            match std::fs::symlink_metadata(p) {
                Ok(m) if m.is_dir() => {}
                Ok(_) => return Some(Unavailable::Unreadable),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                    return Some(Unavailable::PermissionDenied);
                }
                Err(_) => return Some(Unavailable::Unreadable),
            }
        }
        None
    }

    fn look_file(&self, path: &Path) -> Observation {
        if (self.probe)(path) == Provider::NotDownloaded {
            return Observation::unavailable(Unavailable::DownloadNeeded);
        }
        let read = || -> std::io::Result<Vec<u8>> {
            let file = std::fs::OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
                .open(path)?;
            if !file.metadata()?.is_file() {
                return Err(std::io::Error::other("not a regular file"));
            }
            let mut bytes = Vec::new();
            file.take(MAX_SNAPSHOT_BYTES as u64 + 1)
                .read_to_end(&mut bytes)?;
            if bytes.len() > MAX_SNAPSHOT_BYTES {
                return Err(std::io::Error::other("export file too large"));
            }
            Ok(bytes)
        };
        match read() {
            Ok(bytes) => {
                let digest = hex_sha(&bytes);
                Observation {
                    present: true,
                    available: true,
                    fingerprint: digest.clone(),
                    digest,
                    ids: path
                        .strip_prefix(&self.root)
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    unavailable: None,
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Observation::absent(),
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                Observation::unavailable(Unavailable::PermissionDenied)
            }
            Err(_) => Observation::unavailable(Unavailable::Unreadable),
        }
    }
    fn look(&self, date: &DiaryDate) -> Observation {
        let path = self.path(date);
        if let Some(reason) = self.parents_error(date) {
            return Observation::unavailable(reason);
        }
        // Refuse known placeholders before asking Foundation for coordinated read access:
        // coordination itself may otherwise request materialization. Recheck in look_file.
        if (self.probe)(&path) == Provider::NotDownloaded {
            return Observation::unavailable(Unavailable::DownloadNeeded);
        }
        let inner = self.clone_parts();
        let target = path.clone();
        let date = *date;
        match timed(
            self.observe_timeout,
            self.worker_active.clone(),
            move |abandoned| {
                coordination::read(&target, || {
                    let Ok(_guard) = inner.access.lock() else {
                        return Observation::unavailable(Unavailable::CoordinationFailed);
                    };
                    if abandoned.load(Ordering::SeqCst) {
                        return Observation::unavailable(Unavailable::CoordinationFailed);
                    }
                    if let Some(reason) = inner.parents_error(&date) {
                        return Observation::unavailable(reason);
                    }
                    // Serialize cleanup with publication, including workers that outlive a deadline.
                    let _ = std::fs::remove_file(Self::stage(&target));
                    inner.look_file(&target)
                })
            },
        ) {
            Some(Ok(seen)) => seen,
            _ => Observation::unavailable(Unavailable::CoordinationFailed),
        }
    }
    fn clone_parts(&self) -> Arc<Self> {
        Arc::new(Self {
            root: self.root.clone(),
            probe: self.probe.clone(),
            access: self.access.clone(),
            worker_active: self.worker_active.clone(),
            observe_timeout: self.observe_timeout,
            apply_timeout: self.apply_timeout,
        })
    }

    /// Runs inside the coordinated write: nothing here may run after the deadline.
    fn publish(
        &self,
        path: &Path,
        bytes: &[u8],
        write: Write,
        expected: &Observation,
        abandoned: &AtomicBool,
    ) -> Applied {
        let Ok(_guard) = self.access.lock() else {
            return Applied::Failed;
        };
        if abandoned.load(Ordering::SeqCst)
            || path
                .parent()
                .is_none_or(|p| p.canonicalize().ok().as_deref() != Some(p))
        {
            return Applied::Failed;
        }
        let now = self.look_file(path);
        if !now.available
            || now.present != expected.present
            || now.fingerprint != expected.fingerprint
        {
            return Applied::Failed;
        }
        if matches!(write, Write::Create) == now.present {
            return Applied::Failed;
        }
        let stage = Self::stage(path);
        let Ok(mut file) = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&stage)
        else {
            return Applied::Failed;
        };
        if file.write_all(bytes).and_then(|_| file.sync_all()).is_err() {
            let _ = std::fs::remove_file(&stage);
            return Applied::Failed;
        }
        self.publish_staged(path, &stage, write, &now, abandoned)
    }

    /// The caller holds `access` and owns this stage. Revalidate after potentially slow I/O.
    fn publish_staged(
        &self,
        path: &Path,
        stage: &Path,
        write: Write,
        before: &Observation,
        abandoned: &AtomicBool,
    ) -> Applied {
        // Staging may block past the deadline. Never publish after observing abandonment.
        // Recheck external drift after staging as well as before it.
        if abandoned.load(Ordering::SeqCst) || &self.look_file(path) != before {
            let _ = std::fs::remove_file(stage);
            return Applied::Failed;
        }
        let published = match write {
            Write::Replace => std::fs::rename(stage, path).is_ok(),
            Write::Create => rename_excl(stage, path),
        };
        if !published {
            let _ = std::fs::remove_file(stage);
            return Applied::Failed;
        }
        Applied::Done
    }
}

fn rename_excl(from: &Path, to: &Path) -> bool {
    let (Ok(a), Ok(b)) = (
        CString::new(from.as_os_str().as_bytes()),
        CString::new(to.as_os_str().as_bytes()),
    ) else {
        return false;
    };
    // SAFETY: both pointers are valid NUL-terminated paths for the duration of the call.
    unsafe { libc::renamex_np(a.as_ptr(), b.as_ptr(), libc::RENAME_EXCL) == 0 }
}

/// Runs `f` on a worker and waits for the deadline. The flag tells a late accessor to do
/// nothing; a call that was already writing is reported as unknown by the caller.
// A stalled Foundation call may outlive the deadline. Permit at most one worker per
// adapter, so repeated retries cannot accumulate blocked native calls.
fn timed<T: Send + 'static>(
    limit: Duration,
    active: Arc<AtomicBool>,
    f: impl FnOnce(Arc<AtomicBool>) -> T + Send + 'static,
) -> Option<T> {
    if active
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return None;
    }
    struct Worker(Arc<AtomicBool>);
    impl Drop for Worker {
        fn drop(&mut self) {
            self.0.store(false, Ordering::SeqCst);
        }
    }
    let abandoned = Arc::new(AtomicBool::new(false));
    let (tx, rx) = mpsc::channel();
    let flag = abandoned.clone();
    std::thread::spawn(move || {
        let worker = Worker(active);
        let value = f(flag);
        drop(worker);
        let _ = tx.send(value);
    });
    match rx.recv_timeout(limit) {
        Ok(v) => Some(v),
        Err(_) => {
            abandoned.store(true, Ordering::SeqCst);
            None
        }
    }
}

impl ExportAdapter for Icloud {
    fn target(&self) -> Target {
        Target::Icloud
    }
    fn root(&self) -> String {
        self.root.to_string_lossy().into_owned()
    }
    fn observe(&self, date: &DiaryDate) -> Observation {
        self.look(date)
    }
    fn apply(
        &self,
        date: &DiaryDate,
        bytes: &[u8],
        write: Write,
        expected: &Observation,
    ) -> Applied {
        if self.parents_error(date).is_some() {
            return Applied::Failed;
        }
        let dir = self.dir(date);
        if std::fs::create_dir_all(&dir).is_err() || self.parents_error(date).is_some() {
            return Applied::Failed;
        }
        let path = self.path(date);
        let (inner, bytes, expected) = (self.clone_parts(), bytes.to_vec(), expected.clone());
        let target = path.clone();
        match timed(
            self.apply_timeout,
            self.worker_active.clone(),
            move |abandoned| {
                coordination::write(&target, || {
                    inner.publish(&target, &bytes, write, &expected, &abandoned)
                })
            },
        ) {
            Some(Ok(applied)) => applied,
            // The accessor never ran: nothing was written.
            Some(Err(_)) => Applied::Failed,
            // Deadline: a write may or may not have landed; the service reads back.
            None => Applied::Unknown,
        }
    }
    fn read_back(&self, date: &DiaryDate) -> Observation {
        self.look(date)
    }
}

#[cfg(test)]
mod tests;

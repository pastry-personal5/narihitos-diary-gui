//! A directory-backed fake destination for tests and the fault-injection build. One file per
//! date, so state survives a server restart and a script can change or delete it. The
//! fingerprint is the content digest. Faults come from `DIARY_FAULT` (`export:lose_reply`,
//! `export:fail_apply`, `export:unavailable`) or the setters.
use super::hex_sha;
use diary_application::export::{Applied, ExportAdapter, Observation, Target, Unavailable, Write};
use diary_domain::authentic::DiaryDate;
use std::{
    io::Write as _,
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
};

pub struct Fake {
    dir: PathBuf,
    lose_reply: AtomicBool,
    fail_apply: AtomicBool,
    unavailable: AtomicBool,
}
impl Fake {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            lose_reply: AtomicBool::new(false),
            fail_apply: AtomicBool::new(false),
            unavailable: AtomicBool::new(false),
        }
    }
    pub fn lose_reply(&self, on: bool) {
        self.lose_reply.store(on, Ordering::SeqCst);
    }
    pub fn fail_apply(&self, on: bool) {
        self.fail_apply.store(on, Ordering::SeqCst);
    }
    pub fn unavailable(&self, on: bool) {
        self.unavailable.store(on, Ordering::SeqCst);
    }
    fn path(&self, date: &DiaryDate) -> PathBuf {
        self.dir.join(format!("{date}.md"))
    }
    fn flag(&self, own: &AtomicBool, name: &str) -> bool {
        own.load(Ordering::SeqCst)
            || std::env::var("DIARY_FAULT").is_ok_and(|v| v == format!("export:{name}"))
    }
    fn look(&self, date: &DiaryDate) -> Observation {
        if self.flag(&self.unavailable, "unavailable") {
            return Observation::unavailable(Unavailable::DownloadNeeded);
        }
        match std::fs::read(self.path(date)) {
            Ok(bytes) => {
                let digest = hex_sha(&bytes);
                Observation {
                    present: true,
                    available: true,
                    fingerprint: digest.clone(),
                    digest,
                    ids: format!("{date}.md"),
                    unavailable: None,
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Observation::absent(),
            Err(_) => Observation::unavailable(Unavailable::Unreadable),
        }
    }
}
impl ExportAdapter for Fake {
    fn target(&self) -> Target {
        Target::Icloud
    }
    fn root(&self) -> String {
        self.dir.to_string_lossy().into_owned()
    }
    fn observe(&self, date: &DiaryDate) -> Observation {
        self.look(date)
    }
    fn apply(
        &self,
        date: &DiaryDate,
        bytes: &[u8],
        write: Write,
        _expected: &Observation,
    ) -> Applied {
        if self.flag(&self.fail_apply, "fail_apply") {
            return Applied::Failed;
        }
        let path = self.path(date);
        let done = match write {
            // No-clobber: a racing creator makes this fail instead of overwriting.
            Write::Create => std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .and_then(|mut f| f.write_all(bytes)),
            Write::Replace => {
                let tmp = path.with_extension("tmp");
                std::fs::write(&tmp, bytes).and_then(|_| std::fs::rename(&tmp, &path))
            }
        };
        match done {
            Err(_) => Applied::Failed,
            Ok(()) if self.flag(&self.lose_reply, "lose_reply") => Applied::Unknown,
            Ok(()) => Applied::Done,
        }
    }
    fn read_back(&self, date: &DiaryDate) -> Observation {
        self.look(date)
    }
}

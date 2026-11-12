//! Journaled pair publication under the workspace's existing process lock.
use crate::authentic_fs as fs;
use diary_application::{
    agent::CancelToken,
    illustrated::{Error, Store as Port},
};
use diary_domain::{
    authentic::{DiaryDate, valid_uuid},
    illustrated::Entry,
};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Intent {
    date: String,
    delete: bool,
}
pub struct Store {
    root: PathBuf,
    gate: Mutex<bool>,
}
fn io<T>(r: std::io::Result<T>) -> Result<T, Error> {
    r.map_err(|_| Error::Storage)
}
impl Store {
    /// Caller must retain the workspace OS lock for this store's lifetime.
    pub fn open(root: PathBuf) -> Result<Self, Error> {
        io(fs::ensure_dirs(&root, &["assets", "diary", "illustrated"]))?;
        io(fs::ensure_dirs(&root, &[".diary", "illustrated-pending"]))?;
        let s = Self {
            root,
            gate: Mutex::new(true),
        };
        s.recover()?;
        Ok(s)
    }
    fn pending(&self) -> PathBuf {
        self.root.join(".diary/illustrated-pending")
    }
    fn parent(&self, date: DiaryDate) -> Result<PathBuf, Error> {
        let y = format!("{:04}", date.year());
        let m = format!("{:02}", date.month());
        io(fs::ensure_dirs(
            &self.root,
            &["assets", "diary", "illustrated", &y, &m],
        ))?;
        Ok(self.root.join(format!("assets/diary/illustrated/{y}/{m}")))
    }
    fn load(&self, date: DiaryDate) -> Result<Option<Entry>, Error> {
        let p = self.parent(date)?;
        io(fs::read_bounded(
            &p.join(format!("{date}.md")),
            5 * 1024 * 1024,
        ))?
        .map(|b| Entry::parse(&b.bytes, date).map_err(|_| Error::Invalid))
        .transpose()
    }
    fn recover(&self) -> Result<(), Error> {
        for item in io(std::fs::read_dir(self.pending()))? {
            let item = io(item)?;
            let id = item.file_name().to_string_lossy().into_owned();
            if !valid_uuid(&id) || !io(item.file_type())?.is_dir() {
                return Err(Error::Storage);
            }
            let dir = item.path();
            if let Some(b) = io(fs::read_bounded(&dir.join("intent.json"), 1024))? {
                let intent: Intent =
                    serde_json::from_slice(&b.bytes).map_err(|_| Error::Storage)?;
                self.finish(&dir, &intent)?;
            } else {
                io(std::fs::remove_dir_all(&dir))?;
            }
        }
        io(fs::sync_path(&self.pending()))
    }
    fn finish(&self, dir: &Path, intent: &Intent) -> Result<(), Error> {
        let date = DiaryDate::parse(&intent.date).ok_or(Error::Storage)?;
        let parent = self.parent(date)?;
        let md = parent.join(format!("{date}.md"));
        let photos = parent.join(date.to_string());
        for (target, backup, new) in [
            (&photos, dir.join("old-photos"), dir.join("photos")),
            (&md, dir.join("old.md"), dir.join("entry.md")),
        ] {
            let target_exists =
                io(std::fs::symlink_metadata(target)).map(|m| !m.file_type().is_symlink());
            if matches!(target_exists, Ok(false)) {
                return Err(Error::Storage);
            }
            if (intent.delete || new.exists()) && target.exists() && !backup.exists() {
                io(fs::rename_new(target, &backup))?;
                io(fs::sync_path(&parent))?;
                io(fs::sync_path(dir))?;
            }
            if !intent.delete && new.exists() {
                io(fs::rename_new(&new, target))?;
                io(fs::sync_path(&parent))?;
                io(fs::sync_path(dir))?;
            }
        }
        // Receipt first; recovery can re-enter every preceding rename.
        if !dir.join("done").exists() {
            io(fs::create_new(&dir.join("done"), b"done"))?;
            io(fs::sync_file(&dir.join("done")))?;
            io(fs::sync_path(dir))?;
        }
        io(std::fs::remove_dir_all(dir))?;
        io(fs::sync_path(&self.pending()))
    }
    fn validate_existing(&self, date: DiaryDate) -> Result<(), Error> {
        if let Some(e) = self.load(date)? {
            let p = self.parent(date)?.join(date.to_string());
            if !io(std::fs::symlink_metadata(&p))?.is_dir() {
                return Err(Error::Storage);
            }
            let mut count = 0;
            for item in io(std::fs::read_dir(&p))? {
                let item = io(item)?;
                if !io(item.file_type())?.is_file() {
                    return Err(Error::Storage);
                }
                let name = item.file_name().to_string_lossy().into_owned();
                if !(1..=e.photos.len()).any(|i| name == format!("{i:02}.png")) {
                    return Err(Error::Invalid);
                }
                count += 1;
            }
            if count != e.photos.len() {
                return Err(Error::Invalid);
            }
        } else if self.parent(date)?.join(date.to_string()).exists() {
            return Err(Error::Invalid);
        }
        Ok(())
    }
    fn transaction(
        &self,
        date: DiaryDate,
        entry: Option<(&Entry, &[PathBuf])>,
        cancel: &CancelToken,
    ) -> Result<(), Error> {
        let id = crate::lifecycle::random_uuid().map_err(|_| Error::Storage)?;
        io(fs::ensure_dirs(
            &self.root,
            &[".diary", "illustrated-pending", &id],
        ))?;
        let dir = self.pending().join(id);
        let prepare = (|| {
            if let Some((e, images)) = entry {
                if e.photos.len() != images.len() {
                    return Err(Error::Invalid);
                }
                io(fs::ensure_dirs(&dir, &["photos"]))?;
                for (i, image) in images.iter().enumerate() {
                    let bytes = io(fs::read_bounded(image, 32 * 1024 * 1024))?
                        .ok_or(Error::Storage)?
                        .bytes;
                    let path = dir.join(format!("photos/{:02}.png", i + 1));
                    io(fs::create_new(&path, &bytes))?;
                    io(fs::sync_file(&path))?;
                }
                io(fs::sync_path(&dir.join("photos")))?;
                io(fs::create_new(
                    &dir.join("entry.md"),
                    &e.render().map_err(|_| Error::Invalid)?,
                ))?;
                io(fs::sync_file(&dir.join("entry.md")))?;
            }
            if cancel.is_cancelled() {
                return Err(Error::Cancelled);
            }
            Ok(())
        })();
        if let Err(e) = prepare {
            let _ = std::fs::remove_dir_all(&dir);
            return Err(e);
        }
        let intent = Intent {
            date: date.to_string(),
            delete: entry.is_none(),
        };
        io(fs::create_new(
            &dir.join("intent.tmp"),
            &serde_json::to_vec(&intent).map_err(|_| Error::Storage)?,
        ))?;
        io(fs::sync_file(&dir.join("intent.tmp")))?;
        io(fs::sync_path(&dir))?;
        io(fs::rename_new(
            &dir.join("intent.tmp"),
            &dir.join("intent.json"),
        ))?;
        io(fs::sync_path(&dir))?;
        io(fs::sync_path(&self.pending()))?;
        self.finish(&dir, &intent)
    }
}
impl Port for Store {
    fn list(&self) -> Result<Vec<DiaryDate>, Error> {
        let ready = self.gate.lock().unwrap();
        if !*ready {
            return Err(Error::Unavailable);
        }
        let mut dates = vec![];
        let root = self.root.join("assets/diary/illustrated");
        for y in io(std::fs::read_dir(root))? {
            let y = io(y)?;
            if !io(y.file_type())?.is_dir() {
                return Err(Error::Storage);
            }
            for m in io(std::fs::read_dir(y.path()))? {
                let m = io(m)?;
                if !io(m.file_type())?.is_dir() {
                    return Err(Error::Storage);
                }
                for d in io(std::fs::read_dir(m.path()))? {
                    let d = io(d)?;
                    let n = d.file_name().to_string_lossy().into_owned();
                    if let Some(date) = n.strip_suffix(".md").and_then(DiaryDate::parse) {
                        if d.path().parent() != Some(self.parent(date)?.as_path()) {
                            return Err(Error::Invalid);
                        }
                        self.load(date)?.ok_or(Error::Invalid)?;
                        dates.push(date);
                    }
                }
            }
        }
        dates.sort();
        Ok(dates)
    }
    fn read(&self, date: DiaryDate) -> Result<Option<Entry>, Error> {
        let ready = self.gate.lock().unwrap();
        if !*ready {
            return Err(Error::Unavailable);
        }
        let e = self.load(date)?;
        if let Some(e) = &e {
            let p = self.parent(date)?.join(date.to_string());
            if !io(std::fs::symlink_metadata(&p))?.is_dir() {
                return Err(Error::Storage);
            }
            for i in 1..=e.photos.len() {
                io(fs::read_bounded(
                    &p.join(format!("{i:02}.png")),
                    32 * 1024 * 1024,
                ))?
                .ok_or(Error::Invalid)?;
            }
        }
        Ok(e)
    }
    fn photo(&self, date: DiaryDate, version: &str, n: usize) -> Result<Vec<u8>, Error> {
        let ready = self.gate.lock().unwrap();
        if !*ready {
            return Err(Error::Unavailable);
        }
        let e = self.load(date)?.ok_or(Error::NotFound)?;
        if e.id != version {
            return Err(Error::Stale);
        }
        if n == 0 || n > e.photos.len() {
            return Err(Error::NotFound);
        }
        let parent = self.parent(date)?;
        if !io(std::fs::symlink_metadata(parent.join(date.to_string())))?.is_dir() {
            return Err(Error::Storage);
        }
        io(fs::read_bounded(
            &parent.join(format!("{date}/{n:02}.png")),
            32 * 1024 * 1024,
        ))?
        .map(|b| b.bytes)
        .ok_or(Error::NotFound)
    }
    fn publish(&self, e: &Entry, photos: &[PathBuf], cancel: &CancelToken) -> Result<(), Error> {
        e.validate().map_err(|_| Error::Invalid)?;
        let mut ready = self.gate.lock().unwrap();
        if !*ready {
            return Err(Error::Unavailable);
        }
        self.validate_existing(e.date)?;
        let r = self.transaction(e.date, Some((e, photos)), cancel);
        if r == Err(Error::Storage) {
            *ready = false;
        }
        r
    }
    fn delete(&self, date: DiaryDate, version: &str, confirmed: bool) -> Result<(), Error> {
        if !confirmed {
            return Err(Error::ConfirmationRequired);
        }
        let mut ready = self.gate.lock().unwrap();
        if !*ready {
            return Err(Error::Unavailable);
        }
        let e = self.load(date)?.ok_or(Error::NotFound)?;
        if e.id != version {
            return Err(Error::Stale);
        }
        self.validate_existing(date)?;
        let r = self.transaction(date, None, &CancelToken::default());
        if r.is_err() {
            *ready = false;
        }
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use diary_domain::illustrated::Photo;
    use std::os::unix::fs::PermissionsExt;
    fn entry(id: &str) -> Entry {
        Entry {
            id: id.into(),
            date: DiaryDate::parse("2026-11-13").unwrap(),
            created_at: "2026-11-13T00:00:00Z".into(),
            source_id: "07249746-e280-4d51-a86a-cbcb198ee102".into(),
            source_revision: 1,
            source_sha256: "a".repeat(64),
            agent_profile: "test".into(),
            image_engine: "local".into(),
            trigger: String::new(),
            directives: vec![],
            theme_count: 1,
            photos: vec![Photo {
                seed: 1,
                prompt: "a walk".into(),
            }],
            body: "A walk.\n\n![Walk](2026-11-13/01.png)\n".into(),
        }
    }
    const OLD: &str = "07249746-e280-4d51-a86a-cbcb198ee103";
    const NEW: &str = "07249746-e280-4d51-a86a-cbcb198ee104";
    #[test]
    fn missing_photo_directory_reads_do_not_repair_disk() {
        let t = tempfile::tempdir().unwrap();
        let s = Store::open(t.path().into()).unwrap();
        let e = entry(OLD);
        let parent = s.parent(e.date).unwrap();
        fs::create_new(&parent.join(format!("{}.md", e.date)), &e.render().unwrap()).unwrap();
        let photos = parent.join(e.date.to_string());
        assert!(s.read(e.date).is_err());
        assert!(!photos.exists());
        assert!(s.photo(e.date, OLD, 1).is_err());
        assert!(!photos.exists());
    }
    #[test]
    fn pair_replace_version_guard_cancel_and_delete() {
        let t = tempfile::tempdir().unwrap();
        let s = Store::open(t.path().into()).unwrap();
        let e = entry(OLD);
        let image = t.path().join("input.png");
        std::fs::write(&image, b"old").unwrap();
        s.publish(&e, std::slice::from_ref(&image), &CancelToken::default())
            .unwrap();
        assert_eq!(s.list().unwrap(), vec![e.date]);
        assert_eq!(s.photo(e.date, OLD, 1).unwrap(), b"old");
        let run = diary_application::agent::Run::new();
        run.request_cancel();
        assert_eq!(
            s.publish(&entry(NEW), std::slice::from_ref(&image), &run.token()),
            Err(Error::Cancelled)
        );
        assert_eq!(s.read(e.date).unwrap().unwrap().id, OLD);
        std::fs::write(&image, b"new").unwrap();
        s.publish(
            &entry(NEW),
            std::slice::from_ref(&image),
            &CancelToken::default(),
        )
        .unwrap();
        assert_eq!(s.photo(e.date, OLD, 1), Err(Error::Stale));
        assert_eq!(s.photo(e.date, NEW, 1).unwrap(), b"new");
        let p = s.parent(e.date).unwrap();
        assert_eq!(
            std::fs::metadata(p.join(format!("{}.md", e.date)))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert_eq!(
            s.delete(e.date, NEW, false),
            Err(Error::ConfirmationRequired)
        );
        assert_eq!(s.delete(e.date, OLD, true), Err(Error::Stale));
        s.delete(e.date, NEW, true).unwrap();
        assert!(s.read(e.date).unwrap().is_none());
    }
    #[test]
    fn recovery_rolls_forward_each_pair_rename_window() {
        for boundary in 0..=4 {
            let t = tempfile::tempdir().unwrap();
            let s = Store::open(t.path().into()).unwrap();
            let e = entry(OLD);
            let image = t.path().join("input.png");
            std::fs::write(&image, b"old").unwrap();
            s.publish(&e, std::slice::from_ref(&image), &CancelToken::default())
                .unwrap();
            let dir = s.pending().join(NEW);
            fs::ensure_dirs(&s.pending(), &[NEW, "photos"]).unwrap();
            fs::create_new(&dir.join("photos/01.png"), b"new").unwrap();
            fs::create_new(&dir.join("entry.md"), &entry(NEW).render().unwrap()).unwrap();
            fs::create_new(
                &dir.join("intent.json"),
                &serde_json::to_vec(&Intent {
                    date: e.date.to_string(),
                    delete: false,
                })
                .unwrap(),
            )
            .unwrap();
            let parent = s.parent(e.date).unwrap();
            let moves = [
                (parent.join(e.date.to_string()), dir.join("old-photos")),
                (dir.join("photos"), parent.join(e.date.to_string())),
                (parent.join(format!("{}.md", e.date)), dir.join("old.md")),
                (dir.join("entry.md"), parent.join(format!("{}.md", e.date))),
            ];
            for (a, b) in moves.iter().take(boundary) {
                std::fs::rename(a, b).unwrap();
            }
            drop(s);
            let s = Store::open(t.path().into()).unwrap();
            assert_eq!(s.read(e.date).unwrap().unwrap().id, NEW);
            assert_eq!(s.photo(e.date, NEW, 1).unwrap(), b"new");
            assert_eq!(std::fs::read_dir(s.pending()).unwrap().count(), 0);
        }
    }
    #[test]
    fn refuses_photo_directory_symlink_and_cleans_only_owned_staging() {
        let t = tempfile::tempdir().unwrap();
        let s = Store::open(t.path().into()).unwrap();
        let e = entry(OLD);
        let parent = s.parent(e.date).unwrap();
        std::os::unix::fs::symlink(t.path(), parent.join(e.date.to_string())).unwrap();
        fs::create_new(&parent.join(format!("{}.md", e.date)), &e.render().unwrap()).unwrap();
        assert!(s.read(e.date).is_err());
        assert!(s.photo(e.date, OLD, 1).is_err());
    }
}

#[cfg(test)]
mod cleanup_tests {
    use super::*;
    #[test]
    fn restart_cleans_abandoned_stage_without_touching_authentic() {
        let t = tempfile::tempdir().unwrap();
        let s = Store::open(t.path().into()).unwrap();
        fs::ensure_dirs(t.path(), &["assets", "diary", "authentic"]).unwrap();
        let authentic = t.path().join("assets/diary/authentic/sentinel.md");
        fs::create_new(&authentic, b"keep").unwrap();
        let id = "07249746-e280-4d51-a86a-cbcb198ee101";
        fs::ensure_dirs(&s.pending(), &[id]).unwrap();
        fs::create_new(&s.pending().join(id).join("entry.md"), b"unfinished").unwrap();
        drop(s);
        let s = Store::open(t.path().into()).unwrap();
        assert_eq!(std::fs::read_dir(s.pending()).unwrap().count(), 0);
        assert_eq!(std::fs::read(authentic).unwrap(), b"keep");
    }
}

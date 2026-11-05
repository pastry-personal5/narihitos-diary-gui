//! Role-owned rotating log directory: daily and size rotation, caps and age pruning.
//!
//! Synchronous and clock-injected; the asynchronous worker owns all calls into it.

use crate::LogError;
use crate::clock::Clock;
use crate::codec::MAX_LINE;
use crate::private_fs::{FsError, PrivateDir};
use diary_application::diagnostics::{OpaqueId, Role};
use std::{fs::File, io::Write, sync::Arc};

pub const MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;
pub const MAX_FILES: usize = 8;
pub const MAX_TOTAL_BYTES: u64 = 128 * 1024 * 1024;
/// Rotated files older than this (by modification time) are deleted; exactly 72 hours is kept.
pub const RETENTION_SECS: i64 = 72 * 3600;

#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub file_bytes: u64,
    pub max_files: usize,
    pub total_bytes: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            file_bytes: MAX_FILE_BYTES,
            max_files: MAX_FILES,
            total_bytes: MAX_TOTAL_BYTES,
        }
    }
}

struct Active {
    file: File,
    name: String,
    date: String,
    size: u64,
}

pub struct LogStore {
    dir: PrivateDir,
    role: Role,
    clock: Arc<dyn Clock>,
    limits: Limits,
    active: Option<Active>,
    _lock: File,
}

fn file_name(role: Role, date: &str, suffix: u32) -> String {
    if suffix == 0 {
        format!("{}-{date}.log", role.as_str())
    } else {
        format!("{}-{date}.{suffix}.log", role.as_str())
    }
}

/// Only canonical names of this role match: `role-YYYY-MM-DD.log` or `role-YYYY-MM-DD.N.log`.
pub(crate) fn parse_name(role: Role, name: &str) -> Option<(String, u32)> {
    let rest = name
        .strip_prefix(role.as_str())?
        .strip_prefix('-')?
        .strip_suffix(".log")?;
    let (date, suffix) = match rest.split_once('.') {
        Some((d, n)) => (d, n.parse::<u32>().ok().filter(|&n| n >= 1)?),
        None => (rest, 0),
    };
    if date.len() != 10 || OpaqueId::parse(date).is_none() {
        return None;
    }
    (file_name(role, date, suffix) == name).then(|| (date.to_owned(), suffix))
}

fn map(e: FsError) -> LogError {
    match e {
        FsError::Busy => LogError::Locked,
        _ => LogError::Unavailable,
    }
}

impl LogStore {
    pub fn open(
        workspace_dir: &PrivateDir,
        role: Role,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, LogError> {
        Self::open_with(workspace_dir, role, clock, Limits::default())
    }

    pub fn open_with(
        workspace_dir: &PrivateDir,
        role: Role,
        clock: Arc<dyn Clock>,
        limits: Limits,
    ) -> Result<Self, LogError> {
        let dir = workspace_dir
            .child("logs")
            .and_then(|d| d.child(role.as_str()))
            .map_err(map)?;
        let lock = dir.lock(".lock").map_err(map)?;
        let mut store = Self {
            dir,
            role,
            clock,
            limits,
            active: None,
            _lock: lock,
        };
        let _ = store.prune();
        Ok(store)
    }

    /// Appends one rendered line. `Err(Capped)` means the record was dropped, never that an
    /// older file was deleted to make room.
    pub fn append(&mut self, line: &str) -> Result<(), LogError> {
        if line.len() > MAX_LINE {
            return Err(LogError::Oversize);
        }
        let date = self.clock.now().date_string();
        let len = line.len() as u64;
        let rotate = match &self.active {
            Some(a) => a.date != date || a.size + len > self.limits.file_bytes,
            None => true,
        };
        if rotate {
            self.activate(&date, len)?;
        }
        let active = self.active.as_mut().ok_or(LogError::Unavailable)?;
        active
            .file
            .write_all(line.as_bytes())
            .map_err(|_| LogError::Unavailable)?;
        active.size += len;
        Ok(())
    }

    fn activate(&mut self, date: &str, len: u64) -> Result<(), LogError> {
        let _ = self.prune();
        // shortcut: rescans the directory on every rotation or capped retry; add counters
        // if profiling shows the retry path matters.
        let names = self.dir.list().map_err(map)?;
        let (mut count, mut total, mut last) = (0usize, 0u64, None::<u32>);
        for name in &names {
            let Some((d, suffix)) = parse_name(self.role, name) else {
                continue;
            };
            count += 1;
            total += self.dir.metadata(name).map_or(0, |m| m.len);
            if d == date {
                last = last.max(Some(suffix));
            }
        }
        if let Some(suffix) = last {
            let name = file_name(self.role, date, suffix);
            if let Ok(meta) = self.dir.metadata(&name)
                && meta.len + len <= self.limits.file_bytes
            {
                let file = self.dir.append(&name).map_err(map)?;
                self.active = Some(Active {
                    file,
                    name,
                    date: date.to_owned(),
                    size: meta.len,
                });
                return Ok(());
            }
        }
        if count >= self.limits.max_files || total >= self.limits.total_bytes {
            self.active = None;
            return Err(LogError::Capped);
        }
        let name = file_name(self.role, date, last.map_or(0, |s| s + 1));
        let file = self.dir.append(&name).map_err(map)?;
        self.active = Some(Active {
            file,
            name,
            date: date.to_owned(),
            size: 0,
        });
        Ok(())
    }

    /// Deletes this role's rotated files whose mtime is more than 72 hours old. Never touches
    /// the active file, other names, other roles or anything that is not a regular file.
    pub fn prune(&mut self) -> Result<usize, LogError> {
        let now = self.clock.now().epoch_secs;
        let active = self.active.as_ref().map(|a| a.name.clone());
        let mut deleted = 0;
        for name in self.dir.list().map_err(map)? {
            if parse_name(self.role, &name).is_none() || Some(&name) == active.as_ref() {
                continue;
            }
            let Ok(meta) = self.dir.metadata(&name) else {
                continue;
            };
            if now - meta.modified_epoch_secs > RETENTION_SECS && self.dir.remove(&name).is_ok() {
                deleted += 1;
            }
        }
        Ok(deleted)
    }
}

#[cfg(test)]
mod tests;

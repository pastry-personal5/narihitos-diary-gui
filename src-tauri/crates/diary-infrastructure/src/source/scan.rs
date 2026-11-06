use super::{
    adapter::outcome,
    read::{open_at, validated_root},
    root::{IdentityIndex, ResolvedRoot},
};
use crate::config::SourceKind;
use diary_application::source::*;
use diary_domain::source::path::{group_siblings, normalize};
use std::os::macos::fs::MetadataExt as DarwinMetadataExt;
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::{CStr, OsString},
    fs::File,
    os::{
        fd::IntoRawFd,
        unix::{ffi::OsStringExt, fs::MetadataExt},
    },
};

const MAX_CANDIDATES: usize = 10_000;
const MAX_ENTRIES: usize = 100_000;
struct Walker<'a> {
    root: &'a ResolvedRoot,
    kind: &'a SourceKind,
    generation: u64,
    report: ScanReport,
    // Hard stop (entry/candidate bound): unlike a skipped too-deep directory, siblings are abandoned.
    stopped: bool,
    entries: usize,
    candidates: usize,
    producers: BTreeMap<String, Vec<usize>>,
    identities: IdentityIndex,
    entry_limit: usize,
    candidate_limit: usize,
    directories: Option<&'a BTreeSet<String>>,
    cancelled: &'a dyn Fn() -> bool,
}

pub fn scan(root: &ResolvedRoot, kind: &SourceKind, generation: u64) -> ScanReport {
    scan_bounded(root, kind, generation, MAX_ENTRIES)
}
fn scan_bounded(
    root: &ResolvedRoot,
    kind: &SourceKind,
    generation: u64,
    entry_limit: usize,
) -> ScanReport {
    scan_selected(
        root,
        kind,
        generation,
        entry_limit,
        MAX_CANDIDATES,
        None,
        &|| false,
    )
}

/// Restrict descent before opening files; preserve the M9 collision/no-follow rules.
pub fn scan_selected(
    root: &ResolvedRoot,
    kind: &SourceKind,
    generation: u64,
    entry_limit: usize,
    candidate_limit: usize,
    directories: Option<&BTreeSet<String>>,
    cancelled: &dyn Fn() -> bool,
) -> ScanReport {
    let mut w = Walker {
        root,
        kind,
        generation,
        report: ScanReport::default(),
        stopped: false,
        entries: 0,
        candidates: 0,
        producers: BTreeMap::new(),
        identities: IdentityIndex::default(),
        entry_limit,
        candidate_limit,
        directories,
        cancelled,
    };
    match validated_root(root) {
        Ok(fd) => w.walk(fd, "", 0),
        Err(e) => w
            .report
            .failures
            .push((String::new(), SourceOutcome::Unavailable(e))),
    }
    for indices in w.producers.values().filter(|v| v.len() > 1) {
        for i in indices {
            w.report.listings[*i]
                .warnings
                .push(Warning::DuplicateProducerId);
        }
    }
    w.report
        .listings
        .sort_by(|a, b| a.token.key.cmp(&b.token.key));
    w.report.failures.sort_by_cached_key(|(p, _)| {
        normalize(p, root.insensitive)
            .map(|p| p.key)
            .unwrap_or_else(|_| p.clone())
    });
    w.report.collisions.sort();
    w.report.entries_seen = w.entries;
    w.report.candidates_seen = w.candidates;
    w.report
}
impl Walker<'_> {
    /// Best-effort: reading an evicted iCloud file makes macOS materialise it; a later import sees it.
    /// shortcut: one detached thread per file, unbounded by anything but the candidate limit.
    fn hydrate(&self, rel: &str) {
        let path = self.root.canonical.join(rel);
        std::thread::spawn(move || {
            let _ = std::fs::read(path);
        });
    }
    fn failure(&mut self, rel: String, e: Unavailable) {
        if e == Unavailable::NotDownloaded {
            self.hydrate(&rel);
        }
        self.report
            .failures
            .push((rel, SourceOutcome::Unavailable(e)));
    }
    fn walk(&mut self, dir: File, prefix: &str, depth: usize) {
        if (self.cancelled)() {
            self.report.incomplete = true;
            self.stopped = true;
        }
        if self.stopped {
            return;
        }
        let raw = dir.into_raw_fd();
        // SAFETY: ownership is transferred to fdopendir/closedir; names are copied
        // before the next readdir. We retain the directory FD for no-follow openat.
        let stream = unsafe { libc::fdopendir(raw) };
        if stream.is_null() {
            unsafe {
                libc::close(raw);
            }
            self.failure(prefix.into(), Unavailable::Io);
            return;
        }
        struct Stream(*mut libc::DIR);
        impl Drop for Stream {
            fn drop(&mut self) {
                unsafe {
                    libc::closedir(self.0);
                }
            }
        }
        let _guard = Stream(stream);
        let mut names = Vec::new();
        loop {
            // SAFETY: live directory stream; errno distinguishes EOF from failure.
            unsafe {
                *libc::__error() = 0;
            }
            let entry = unsafe { libc::readdir(stream) };
            if entry.is_null() {
                if unsafe { *libc::__error() } != 0 {
                    self.failure(prefix.into(), Unavailable::Io);
                    self.report.incomplete = true;
                }
                break;
            }
            let bytes = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }.to_bytes();
            if bytes == b"." || bytes == b".." {
                continue;
            }
            if self.entries == self.entry_limit {
                self.report.incomplete = true;
                self.stopped = true;
                break;
            }
            self.entries += 1;
            names.push(OsString::from_vec(bytes.to_vec()));
        }
        // On an entry bound, process only the sorted observed names but visibly mark
        // incomplete. Never claim this arbitrary directory enumeration is complete.
        let strings: Vec<_> = names.iter().map(|n| name_utf8(n).ok()).collect();
        let valid: Vec<_> = strings
            .iter()
            .map(|s| s.map(logical_name).unwrap_or(""))
            .collect();
        let groups = group_siblings(&valid, self.root.insensitive);
        let mut skip = BTreeSet::new();
        for group in groups {
            let mut collision = Vec::new();
            for i in group {
                skip.insert(i);
                collision.push(join(prefix, strings[i].unwrap_or(valid[i])));
            }
            collision.sort();
            self.report.collisions.push(collision);
        }
        let mut indices: Vec<_> = (0..names.len()).filter(|i| !skip.contains(i)).collect();
        indices.sort_by_cached_key(|i| {
            normalize(valid[*i], self.root.insensitive)
                .map(|p| p.key)
                .unwrap_or_default()
        });
        for i in indices {
            if (self.cancelled)() {
                self.report.incomplete = true;
                self.stopped = true;
                break;
            }
            let Some(name) = strings[i] else {
                self.failure(prefix.into(), Unavailable::InvalidPath);
                continue;
            };
            let placeholder = name
                .strip_prefix('.')
                .and_then(|s| s.strip_suffix(".icloud"))
                .filter(|s| s.ends_with(".md"));
            if name.starts_with('.') && placeholder.is_none() {
                continue;
            }
            let rel = join(prefix, placeholder.unwrap_or(name));
            let path = match normalize(&rel, self.root.insensitive) {
                Ok(p) => p,
                Err(_) => {
                    self.failure(prefix.into(), Unavailable::InvalidPath);
                    continue;
                }
            };
            if let Some(dirs) = self.directories {
                let relevant = if depth < 3 {
                    dirs.iter()
                        .any(|d| d == &rel || d.starts_with(&format!("{rel}/")))
                } else {
                    depth == 3 && dirs.contains(prefix)
                };
                if !relevant {
                    continue;
                }
            }
            let file = match open_at(raw, &names[i], false) {
                Ok(f) => f,
                Err(e) => {
                    if name.ends_with(".md") {
                        self.candidate_failure(rel, e);
                    } else if matches!(e, Unavailable::Denied | Unavailable::Io) {
                        // Possibly an unreadable directory: never claim a complete scan.
                        self.report.incomplete = true;
                    }
                    continue;
                }
            };
            let m = match file.metadata() {
                Ok(m) => m,
                Err(_) => {
                    self.failure(rel, Unavailable::Io);
                    continue;
                }
            };
            if m.dev() != self.root.dev {
                continue;
            }
            if m.is_dir() {
                if m.st_flags() & super::read::SF_DATALESS != 0 {
                    self.failure(rel, Unavailable::NotDownloaded);
                    continue;
                }
                if name.ends_with(".assets") {
                    continue;
                }
                if depth >= 8 {
                    self.report.incomplete = true;
                    continue;
                }
                // Reopen with O_DIRECTORY before descending; never follow a swap.
                match open_at(raw, &names[i], true) {
                    Ok(f) => self.walk(f, &rel, depth + 1),
                    Err(e) => self.failure(rel, e),
                }
                continue;
            }
            if placeholder.is_none() && !name.ends_with(".md") {
                continue;
            }
            if !self.admit() {
                break;
            }
            if placeholder.is_some() {
                self.failure(rel, Unavailable::NotDownloaded);
                continue;
            }
            let (result, producer, observed) =
                outcome(self.root, self.kind, &path, self.generation);
            if let Some(listing) = &observed
                && !self
                    .identities
                    .insert(&listing.id, self.root, &listing.token.key)
            {
                self.report.failures.push((
                    rel,
                    SourceOutcome::Conflict(ConflictKind::IdentityCollision),
                ));
                continue;
            }
            match result {
                SourceOutcome::Ok(listing) => {
                    if let Some(p) = producer {
                        self.producers
                            .entry(p)
                            .or_default()
                            .push(self.report.listings.len());
                    }
                    self.report.listings.push(*listing);
                }
                failure => {
                    if matches!(
                        failure,
                        SourceOutcome::Unsupported { viewable: true, .. }
                            | SourceOutcome::Conflict(ConflictKind::IdMismatch)
                    ) && let Some(listing) = observed
                    {
                        self.report.readable.push(listing);
                    }
                    if matches!(
                        failure,
                        SourceOutcome::Unavailable(Unavailable::NotDownloaded)
                    ) {
                        self.hydrate(&rel);
                    }
                    self.report.failures.push((rel, failure));
                }
            }
        }
    }
    fn admit(&mut self) -> bool {
        if self.candidates == self.candidate_limit {
            self.report.incomplete = true;
            self.stopped = true;
            false
        } else {
            self.candidates += 1;
            true
        }
    }
    fn candidate_failure(&mut self, rel: String, e: Unavailable) {
        if self.admit() {
            self.failure(rel, e);
        }
    }
}
fn name_utf8(name: &std::ffi::OsStr) -> Result<&str, diary_domain::source::path::PathError> {
    name.to_str()
        .ok_or(diary_domain::source::path::PathError::NonUtf8)
}
fn logical_name(name: &str) -> &str {
    name.strip_prefix('.')
        .and_then(|s| s.strip_suffix(".icloud"))
        .filter(|s| s.ends_with(".md"))
        .unwrap_or(name)
}
fn join(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.into()
    } else {
        format!("{prefix}/{name}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::tests::{AYUMI, Tree, ayumi};
    use std::{fs, os::unix::ffi::OsStringExt};
    #[test]
    fn collisions_remove_every_member_including_directories() {
        let mut tree = Tree::new();
        tree.write("a.md", &ayumi());
        tree.write("A.md", &ayumi());
        // Case twins can only exist on a sensitive test volume. Force the conservative
        // insensitive comparison in that case; real APFS twins are otherwise impossible.
        if fs::read_dir(tree.temp.path()).unwrap().count() == 2 {
            tree.root.insensitive = true;
            let report = scan(&tree.root, &SourceKind::Ayumi, 0);
            assert!(report.listings.is_empty());
            assert!(report.failures.is_empty());
            assert_eq!(
                report.collisions,
                vec![vec!["A.md".to_owned(), "a.md".to_owned()]]
            );
        }
        assert_eq!(group_siblings(&["D", "d", "é", "e\u{301}"], true).len(), 2);
    }
    #[test]
    fn entry_and_depth_bounds_are_visible_and_non_utf8_is_redacted() {
        let tree = Tree::new();
        for i in 0..12 {
            tree.write(&format!(".ignored-{i}"), b"");
        }
        let report = scan_bounded(&tree.root, &SourceKind::Ayumi, 0, 10);
        assert!(report.incomplete);
        assert!(report.listings.is_empty());
        let tree = Tree::new();
        tree.write("a/b/c/d/e/f/g/h/i/hidden.md", b"bad");
        let report = scan(&tree.root, &SourceKind::Ayumi, 0);
        assert!(report.incomplete);
        assert!(report.failures.is_empty());
        // A too-deep directory is marked incomplete but must not hide sorted-later siblings.
        let tree = Tree::new();
        tree.write("a/b/c/d/e/f/g/h/i/hidden.md", b"bad");
        tree.write(AYUMI, &ayumi());
        let report = scan(&tree.root, &SourceKind::Ayumi, 0);
        assert!(report.incomplete);
        assert_eq!(report.listings.len(), 1);
        // APFS rejects non-UTF-8 filenames with EPERM; test the exact OsStr
        // conversion boundary without asking the filesystem to represent one.
        let invalid = OsString::from_vec(vec![255]);
        assert_eq!(
            name_utf8(&invalid),
            Err(diary_domain::source::path::PathError::NonUtf8)
        );
    }
}

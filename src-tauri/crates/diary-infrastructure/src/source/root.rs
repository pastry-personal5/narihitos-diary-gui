use super::read::{ReadError, open_root};
use crate::config::Source;
use diary_domain::source::path::SourcePath;
use sha2::{Digest, Sha256};
use std::os::macos::fs::MetadataExt as DarwinMetadataExt;
use std::{
    fmt,
    os::{fd::AsRawFd, unix::fs::MetadataExt},
    path::PathBuf,
};

pub struct ResolvedRoot {
    pub(crate) canonical: PathBuf,
    pub insensitive: bool,
    pub dev: u64,
    pub ino: u64,
}
impl fmt::Debug for ResolvedRoot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResolvedRoot")
            .field("insensitive", &self.insensitive)
            .finish_non_exhaustive()
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootError {
    Missing,
    NotDirectory,
    Denied,
    Duplicate,
    Nested,
    NotDownloaded,
    NonUtf8,
    Io,
}
pub fn resolve_roots(sources: &[Source]) -> Vec<Result<ResolvedRoot, RootError>> {
    let mut roots: Vec<_> = sources.iter().map(|s| resolve(&s.root)).collect();
    let mut errors = vec![None; roots.len()];
    for i in 0..roots.len() {
        for j in i + 1..roots.len() {
            if let (Ok(a), Ok(b)) = (&roots[i], &roots[j]) {
                let err = if a.canonical == b.canonical || (a.dev, a.ino) == (b.dev, b.ino) {
                    Some(RootError::Duplicate)
                } else if a.canonical.starts_with(&b.canonical)
                    || b.canonical.starts_with(&a.canonical)
                {
                    Some(RootError::Nested)
                } else {
                    None
                };
                if let Some(e) = err {
                    errors[i] = Some(e);
                    errors[j] = Some(e);
                }
            }
        }
    }
    for (r, e) in roots.iter_mut().zip(errors) {
        if let Some(e) = e {
            *r = Err(e);
        }
    }
    roots
}
fn resolve(path: &std::path::Path) -> Result<ResolvedRoot, RootError> {
    let canonical = path.canonicalize().map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => RootError::Missing,
        std::io::ErrorKind::PermissionDenied => RootError::Denied,
        _ => RootError::Io,
    })?;
    if canonical.to_str().is_none() {
        return Err(RootError::NonUtf8);
    }
    // Open every canonical component without following a concurrent symlink swap.
    let file = open_root(&canonical).map_err(|e| match e {
        ReadError::Missing => RootError::Missing,
        ReadError::Denied => RootError::Denied,
        ReadError::NotRegular => RootError::NotDirectory,
        ReadError::NotDownloaded => RootError::NotDownloaded,
        _ => RootError::Io,
    })?;
    let m = file.metadata().map_err(|_| RootError::Io)?;
    if m.st_flags() & super::read::SF_DATALESS != 0 {
        return Err(RootError::NotDownloaded);
    }
    // SAFETY: fpathconf only inspects the live root descriptor; -1 is treated conservatively.
    let case = unsafe { libc::fpathconf(file.as_raw_fd(), libc::_PC_CASE_SENSITIVE) };
    Ok(ResolvedRoot {
        canonical,
        insensitive: case != 1,
        dev: m.dev(),
        ino: m.ino(),
    })
}
pub fn source_id(root: &ResolvedRoot, path: &SourcePath) -> String {
    let mut h = Sha256::new();
    for p in [root.canonical.to_str().expect("validated UTF-8"), &path.key] {
        h.update((p.len() as u64).to_be_bytes());
        h.update(p.as_bytes());
    }
    h.finalize()[..16]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// A public ID is only an index. Compare the full namespace/path tuple before
/// accepting it; even a deliberately forced shortened-hash collision is refused.
#[derive(Default)]
pub(super) struct IdentityIndex(std::collections::BTreeMap<String, (PathBuf, String)>);
impl IdentityIndex {
    pub fn insert(&mut self, id: &str, root: &ResolvedRoot, path: &str) -> bool {
        let tuple = (root.canonical.clone(), path.to_owned());
        match self.0.entry(id.into()) {
            std::collections::btree_map::Entry::Vacant(v) => {
                v.insert(tuple);
                true
            }
            std::collections::btree_map::Entry::Occupied(o) => o.get() == &tuple,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::tests::Tree;
    #[test]
    fn forced_hash_collision_checks_full_tuple() {
        let a = Tree::new();
        let b = Tree::new();
        let mut ids = IdentityIndex::default();
        assert!(ids.insert("forced", &a.root, "a.md"));
        assert!(ids.insert("forced", &a.root, "a.md"));
        assert!(!ids.insert("forced", &a.root, "b.md"));
        assert!(!ids.insert("forced", &b.root, "a.md"));
    }
}

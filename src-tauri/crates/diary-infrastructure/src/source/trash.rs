//! Producer-derived Trash preparation. No native side effects or capability enablement.
use super::{
    adapter::scalar,
    narihito_write,
    read::{FileDigest, ReadError, digest_member, read_fresh},
    root::{ResolvedRoot, resolve_roots},
    scan::scan_ownership,
};
use crate::config::{Source, SourceKind};
use diary_domain::source::{
    front_matter::{MAX_FILE, parse},
    path::normalize,
};
use std::{
    fmt,
    time::{Duration, Instant},
};

mod fault;
pub mod native;
pub mod service;
pub mod store;

pub const MAX_AUDIO_BYTES: u64 = 64 * 1024 * 1024;
/// Independent evidence gate. Neither source-save verification nor test fault flags
/// can enable ordinary binaries. T11 native/producer matrix passed 2026-11-06; see
/// the P1-M15 completion record and scripts/smoke/trash.py.
pub const TRASH_PAIR_VERIFIED: bool = true;
const MAX_ENTRIES: usize = 100_000;
const MAX_CANDIDATES: usize = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreparationError {
    Unsupported,
    UnsafePath,
    Unavailable,
    Changed,
    OwnershipAmbiguous,
    Limit,
    Cancelled,
    RootChanged,
}

impl From<ReadError> for PreparationError {
    fn from(value: ReadError) -> Self {
        match value {
            ReadError::InvalidPath | ReadError::NotRegular => Self::UnsafePath,
            ReadError::TooLarge => Self::Limit,
            ReadError::SourceChanging => Self::Changed,
            _ => Self::Unavailable,
        }
    }
}

/// Private preparation data. Debug deliberately omits paths and producer metadata.
pub struct PairPlan {
    pub root: ResolvedRoot,
    pub markdown_rel: String,
    pub markdown: FileDigest,
    pub audio_rel: String,
    /// `None` means ENOENT from a fully validated path, never a read/coordination error.
    pub audio: Option<FileDigest>,
    pub producer_id: String,
}
impl fmt::Debug for PairPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PairPlan")
            .field("audio_present", &self.audio.is_some())
            .finish_non_exhaustive()
    }
}

/// Derive the only two possible members from the pinned producer layout. The caller
/// supplies a durable source binding, never a client-provided path. Confirmation must
/// repeat preparation under the resource coordinator before admission/execution.
pub fn prepare_pair(
    sources: &[Source],
    source_index: usize,
    rel: &str,
    cancelled: &dyn Fn() -> bool,
) -> Result<PairPlan, PreparationError> {
    let started = Instant::now();
    let stop = || cancelled() || started.elapsed() >= Duration::from_secs(120);
    let source = sources
        .get(source_index)
        .ok_or(PreparationError::RootChanged)?;
    if !matches!(source.kind, SourceKind::Narihito) {
        return Err(PreparationError::Unsupported);
    }
    let roots: Vec<_> = resolve_roots(sources)
        .into_iter()
        .collect::<Result<_, _>>()
        .map_err(|_| PreparationError::RootChanged)?;
    let root = &roots[source_index];
    let path = normalize(rel, root.insensitive).map_err(|_| PreparationError::UnsafePath)?;
    let original = read_fresh(root, &path)?;
    if original.nlink != 1 {
        return Err(PreparationError::UnsafePath);
    }
    let checked =
        narihito_write::check(&original.bytes, rel).map_err(|_| PreparationError::Unsupported)?;
    if !checked.recording {
        return Err(PreparationError::Unsupported);
    }
    let stem = rel
        .strip_suffix(".md")
        .ok_or(PreparationError::Unsupported)?;
    let audio_rel = format!("{stem}.assets/{}.m4a", checked.id);
    let audio_path =
        normalize(&audio_rel, root.insensitive).map_err(|_| PreparationError::UnsafePath)?;
    let audio = optional_audio(root, &audio_path, &stop)?;
    if let (Some(expected), Some(observed)) = (&checked.audio_sha256, &audio) {
        let digest: String = observed.sha256.iter().map(|b| format!("{b:02x}")).collect();
        if expected != &digest {
            return Err(PreparationError::Changed);
        }
    }
    assess_ownership(
        &roots,
        sources,
        Some((source_index, rel)),
        &checked.id,
        &stop,
    )?;
    let markdown = digest_member(root, &path, MAX_FILE as u64, &stop)?;
    if markdown.sha256 != original.sha256
        || markdown.dev != original.dev
        || markdown.ino != original.ino
        || markdown.mtime_ns != original.mtime_ns
        || markdown.ctime_ns != original.ctime_ns
    {
        return Err(PreparationError::Changed);
    }
    // A new/replaced audio during the ownership scan changes the preview, including
    // the absent-to-present case. Missing audio remains a reserved plan member.
    if optional_audio(root, &audio_path, &stop)? != audio {
        return Err(PreparationError::Changed);
    }
    if stop() {
        return Err(PreparationError::Cancelled);
    }
    let root = roots
        .into_iter()
        .nth(source_index)
        .ok_or(PreparationError::RootChanged)?;
    Ok(PairPlan {
        root,
        markdown_rel: rel.into(),
        markdown,
        audio_rel,
        audio,
        producer_id: checked.id,
    })
}

fn optional_audio(
    root: &ResolvedRoot,
    path: &diary_domain::source::path::SourcePath,
    stop: &dyn Fn() -> bool,
) -> Result<Option<FileDigest>, PreparationError> {
    match digest_member(root, path, MAX_AUDIO_BYTES, stop) {
        Ok(digest) => Ok(Some(digest)),
        Err(ReadError::Missing) => {
            // Distinguish an absent member from a removed/repointed root.
            super::read::validated_root(root).map_err(|_| PreparationError::RootChanged)?;
            // Legacy iCloud placeholder spelling is unavailable content, never
            // evidence that the producer-owned audio is absent.
            let (parent, name) = path
                .spelling
                .rsplit_once('/')
                .ok_or(PreparationError::UnsafePath)?;
            let placeholder = normalize(&format!("{parent}/.{name}.icloud"), root.insensitive)
                .map_err(|_| PreparationError::UnsafePath)?;
            match super::read::open_member(root, &placeholder) {
                Ok(_) => return Err(PreparationError::Unavailable),
                Err(ReadError::Missing) => {}
                Err(e) => return Err(e.into()),
            }
            Ok(None)
        }
        Err(e) => Err(e.into()),
    }
}

fn assess_ownership(
    roots: &[ResolvedRoot],
    sources: &[Source],
    selected: Option<(usize, &str)>,
    id: &str,
    stop: &dyn Fn() -> bool,
) -> Result<(), PreparationError> {
    let mut entries = 0;
    let mut candidates = 0;
    for (index, (root, source)) in roots.iter().zip(sources).enumerate() {
        if stop() {
            return Err(PreparationError::Cancelled);
        }
        let report = scan_ownership(
            root,
            &source.kind,
            MAX_ENTRIES - entries,
            MAX_CANDIDATES - candidates,
            stop,
        );
        entries += report.entries_seen;
        candidates += report.candidates_seen;
        if report.incomplete
            || !report.failures.is_empty()
            || !report.collisions.is_empty()
            || !report.readable.is_empty()
        {
            return Err(PreparationError::OwnershipAmbiguous);
        }
        for listing in report.listings {
            if selected.is_some_and(|(selected, rel)| index == selected && listing.rel == rel) {
                continue;
            }
            if stop() {
                return Err(PreparationError::Cancelled);
            }
            let path = normalize(&listing.rel, root.insensitive)
                .map_err(|_| PreparationError::UnsafePath)?;
            let observation = read_fresh(root, &path)?;
            if observation.sha256 != listing.token.sha256 {
                return Err(PreparationError::Changed);
            }
            let parsed =
                parse(&observation.bytes).map_err(|_| PreparationError::OwnershipAmbiguous)?;
            // Duplicate producer identity or an explicit same audio claim is ambiguous,
            // even across roots/dates. Body links never authorize additional members.
            if scalar(&parsed, "id") == Some(id)
                || scalar(&parsed, "audio_filename") == Some(format!("{id}.m4a").as_str())
            {
                return Err(PreparationError::OwnershipAmbiguous);
            }
        }
    }
    Ok(())
}

/// Revalidate the immutable original plan, including retained audio after Markdown
/// moved. Replacement Markdown participates in ownership scanning; it is never a
/// continuation member and is never substituted for the original.
fn validate_plan(
    sources: &[Source],
    plan: &store::StoredPlan,
    ordinals: &[u8],
    markdown_moved: bool,
    stop: &dyn Fn() -> bool,
) -> Result<(), PreparationError> {
    if stop() {
        return Err(PreparationError::Cancelled);
    }
    let roots: Vec<_> = resolve_roots(sources)
        .into_iter()
        .collect::<Result<_, _>>()
        .map_err(|_| PreparationError::RootChanged)?;
    let index = roots
        .iter()
        .position(|r| r.canonical == plan.root && r.dev == plan.dev && r.ino == plan.ino)
        .ok_or(PreparationError::RootChanged)?;
    if !matches!(sources[index].kind, SourceKind::Narihito) {
        return Err(PreparationError::Unsupported);
    }
    let root = &roots[index];
    super::read::validated_root(root).map_err(|_| PreparationError::RootChanged)?;
    if ordinals.contains(&0) {
        let path = normalize(&plan.markdown_rel, root.insensitive)
            .map_err(|_| PreparationError::UnsafePath)?;
        if digest_member(root, &path, MAX_FILE as u64, stop)? != plan.markdown {
            return Err(PreparationError::Changed);
        }
    }
    if ordinals.contains(&1) || plan.audio.is_none() {
        let path = normalize(&plan.audio_rel, root.insensitive)
            .map_err(|_| PreparationError::UnsafePath)?;
        if optional_audio(root, &path, stop)? != plan.audio {
            return Err(PreparationError::Changed);
        }
    }
    assess_ownership(
        &roots,
        sources,
        if markdown_moved {
            None
        } else {
            Some((index, &plan.markdown_rel))
        },
        &plan.producer_id,
        stop,
    )?;
    if stop() {
        return Err(PreparationError::Cancelled);
    }
    Ok(())
}

#[cfg(test)]
mod tests;

//! Captured-zone imports over the existing bounded, no-follow scanner.
use super::{
    root::{RootError, resolve_roots},
    scan::scan_selected,
};
use crate::{
    config::Source,
    zone::{SystemZone, ZoneError},
};
use diary_application::{
    imports::*,
    source::{ScanReport, SourceListing, SourceOutcome, SourceView, Unavailable, ViewError},
};
use diary_domain::{
    authentic::DiaryDate,
    source::{
        stamp::{Dating, StampError},
        window::{ImportZone, WindowError, Zone, import_window, include},
    },
};
use std::{
    collections::BTreeMap,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

type Capture = dyn Fn() -> Result<(ImportZone, SystemZone), ZoneError> + Send + Sync;
pub struct Store {
    sources: Vec<Source>,
    repository: Arc<dyn ImportRepository>,
    capture: Arc<Capture>,
}
impl Store {
    pub fn new(sources: Vec<Source>, repository: Arc<dyn ImportRepository>) -> Self {
        Self {
            sources,
            repository,
            capture: Arc::new(SystemZone::capture),
        }
    }
    fn scan(
        &self,
        context: ImportContext,
        zone: &SystemZone,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<ImportListing, ImportError> {
        let window = import_window(context.date, zone).map_err(window_error)?;
        let mut scan = ScanReport::default();
        let mut navigation = Navigation::default();
        let mut remaining_bytes: usize = 16 * 1024 * 1024;
        let mut candidates: usize = 0;
        let mut entries = 0;
        let mut roots_by_namespace = BTreeMap::new();
        let mut bindings = Vec::new();
        for (source, root) in self.sources.iter().zip(resolve_roots(&self.sources)) {
            check(cancelled)?;
            let group_id = crate::lifecycle::random_uuid().map_err(|_| ImportError::Storage)?;
            navigation.groups.push(SourceGroup {
                id: group_id.clone(),
                alias: source.alias.clone(),
                kind: match source.kind {
                    crate::config::SourceKind::Ayumi => "ayumi",
                    crate::config::SourceKind::Narihito => "narihito",
                }
                .into(),
            });
            let root = match root {
                Ok(root) => root,
                Err(e) => {
                    let code = match e {
                        RootError::Missing => Unavailable::Missing,
                        RootError::Denied => Unavailable::Denied,
                        RootError::NotDownloaded => Unavailable::NotDownloaded,
                        RootError::NotDirectory => Unavailable::NotRegular,
                        _ => Unavailable::InvalidPath,
                    };
                    scan.failures
                        .push((String::new(), SourceOutcome::Unavailable(code)));
                    navigation.failure_groups.push(group_id);
                    scan.incomplete = true;
                    continue;
                }
            };
            let report = scan_selected(
                &root,
                &source.kind,
                context.config_generation,
                100_000 - entries,
                10_000 - candidates,
                None,
                cancelled,
            );
            entries += report.entries_seen;
            candidates += report.candidates_seen;
            scan.entries_seen = entries;
            scan.candidates_seen = candidates;
            scan.incomplete |= report.incomplete;
            for group in report.collisions {
                let bytes = group.iter().map(String::len).sum::<usize>();
                if bytes > remaining_bytes {
                    scan.incomplete = true;
                    break;
                }
                remaining_bytes -= bytes;
                navigation.collision_groups.push(group_id.clone());
                scan.collisions.push(group);
            }
            for (path, outcome) in report.failures {
                if path.len() + 128 > remaining_bytes {
                    scan.incomplete = true;
                    break;
                }
                remaining_bytes -= path.len() + 128;
                navigation.failure_groups.push(group_id.clone());
                scan.failures.push((path, outcome));
            }
            let supported_count = report.listings.len();
            for (i, mut listing) in report
                .listings
                .into_iter()
                .chain(report.readable)
                .enumerate()
            {
                let bytes = listing.rel.len() + listing.token.key.len() + 512;
                if bytes > remaining_bytes {
                    scan.incomplete = true;
                    break;
                }
                remaining_bytes -= bytes;
                roots_by_namespace.insert(listing.token.root_namespace, root.canonical.clone());
                listing.token.import_id = Some(context.id.clone());
                bindings.push((
                    listing.id.clone(),
                    listing.token.root_namespace,
                    listing.token.key.clone(),
                ));
                // Display only: a stamp the zone cannot show (e.g. year 9999 in a positive
                // offset) must not fail the whole-library import.
                let header = choose(&listing.dating, Authority::Header)
                    .and_then(|t| zone.display(t.unix_ms).ok());
                let filename = choose(&listing.dating, Authority::Filename)
                    .and_then(|t| zone.display(t.unix_ms).ok());
                navigation.sources.insert(
                    listing.id.clone(),
                    SourceDisplay {
                        group_id: group_id.clone(),
                        header,
                        filename,
                        included: None,
                        authority: None,
                    },
                );
                if i < supported_count {
                    scan.listings.push(listing);
                } else {
                    scan.readable.push(listing);
                }
            }
            if candidates >= 10_000 || entries >= 100_000 || remaining_bytes < 512 {
                scan.incomplete = true;
                break;
            }
        }
        check(cancelled)?;
        self.repository.bind_sources(&bindings)?;
        let mut result = ImportListing {
            context,
            included: Vec::new(),
            warnings: Vec::new(),
            misplaced: Vec::new(),
            scan,
            navigation: Some(navigation),
        };
        for source in result.scan.listings.clone() {
            if misplaced(&source) {
                result.misplaced.push(source.id.clone());
            }
            let resolution = self
                .repository
                .resolution(&source.id, &source.token.sha256)?
                .filter(|r| choose(&source.dating, r.authority) == Some(r.instant));
            let instant = match source.dating {
                Dating::Agreed(t) => Some(t),
                _ => resolution.as_ref().map(|r| r.instant),
            };
            if let Some(instant) = instant {
                if include(&window, zone, instant.unix_ms).map_err(window_error)? {
                    let display = result
                        .navigation
                        .as_mut()
                        .expect("new snapshot")
                        .sources
                        .get_mut(&source.id)
                        .expect("scan source");
                    display.included = Some(zone.display(instant.unix_ms).map_err(window_error)?);
                    display.authority = resolution.map(|r| r.authority);
                    result.included.push(IncludedSource { source, instant });
                }
            } else {
                result.warnings.push(source);
            }
        }
        result.included.sort_by(|a, b| {
            (
                a.instant.unix_ms,
                &roots_by_namespace[&a.source.token.root_namespace],
                &a.source.token.key,
            )
                .cmp(&(
                    b.instant.unix_ms,
                    &roots_by_namespace[&b.source.token.root_namespace],
                    &b.source.token.key,
                ))
        });
        result.warnings.sort_by(|a, b| {
            (&roots_by_namespace[&a.token.root_namespace], &a.token.key)
                .cmp(&(&roots_by_namespace[&b.token.root_namespace], &b.token.key))
        });
        Ok(result)
    }
}
fn check(cancelled: &dyn Fn() -> bool) -> Result<(), ImportError> {
    if cancelled() {
        Err(ImportError::Cancelled)
    } else {
        Ok(())
    }
}
fn window_error(e: WindowError) -> ImportError {
    match e {
        WindowError::InvalidDiaryDate | WindowError::Range => ImportError::InvalidDiaryDate,
        WindowError::Zone => ImportError::TimezoneDatabase,
    }
}
fn zone_error(e: ZoneError) -> ImportError {
    match e {
        ZoneError::Unnamed => ImportError::UnnamedZone,
        _ => ImportError::TimezoneDatabase,
    }
}
fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64
}
fn misplaced(source: &SourceListing) -> bool {
    matches!(
        source.dating,
        Dating::HeaderOnly {
            filename: StampError::DirMismatch,
            ..
        } | Dating::Undated {
            filename: StampError::DirMismatch,
            ..
        }
    )
}
// A read descriptor is publishable only when its corresponding failure row survived
// the scan budget. Equal relative names under distinct roots must not be conflated.
fn published_readable<'a>(
    snapshot: &'a ImportListing,
    source: &'a SourceListing,
) -> Option<&'a SourceListing> {
    let nav = snapshot.navigation.as_ref()?;
    let group = &nav.sources.get(&source.id)?.group_id;
    snapshot
        .scan
        .failures
        .iter()
        .zip(&nav.failure_groups)
        .any(|((key, outcome), owner)| {
            key == &source.rel
                && owner == group
                && matches!(
                    outcome,
                    SourceOutcome::Unsupported { viewable: true, .. }
                        | SourceOutcome::Conflict(
                            diary_application::source::ConflictKind::IdMismatch
                        )
                )
        })
        .then_some(source)
}
impl ImportStore for Store {
    fn today(&self) -> Result<DiaryDate, ImportError> {
        let (_, zone) = (self.capture)().map_err(zone_error)?;
        zone.local_date(now()).map_err(window_error)
    }
    fn read(
        &self,
        import_id: &str,
        source_id: &str,
        generation: u64,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<SourceView, ViewError> {
        if cancelled() {
            return Err(ViewError::Stale);
        }
        let snapshot = self
            .repository
            .snapshot(import_id)
            .map_err(|_| ViewError::Stale)?;
        if snapshot.navigation.is_none() || snapshot.context.config_generation != generation {
            return Err(ViewError::Stale);
        }
        let source = snapshot
            .included
            .iter()
            .map(|entry| &entry.source)
            .chain(&snapshot.warnings)
            .chain(
                snapshot
                    .scan
                    .readable
                    .iter()
                    .filter_map(|s| published_readable(&snapshot, s)),
            )
            .find(|s| s.id == source_id)
            .ok_or(ViewError::Stale)?;
        for (configured, root) in self.sources.iter().zip(resolve_roots(&self.sources)) {
            if cancelled() {
                return Err(ViewError::Stale);
            }
            let root = match root {
                Ok(root) => root,
                Err(e) => {
                    let nav = snapshot.navigation.as_ref().expect("checked");
                    let group = &nav.sources.get(source_id).ok_or(ViewError::Stale)?.group_id;
                    if nav
                        .groups
                        .iter()
                        .any(|g| &g.id == group && g.alias == configured.alias)
                    {
                        return Err(ViewError::Outcome(SourceOutcome::Unavailable(match e {
                            RootError::Missing => Unavailable::Missing,
                            RootError::Denied => Unavailable::Denied,
                            RootError::NotDownloaded => Unavailable::NotDownloaded,
                            _ => Unavailable::InvalidPath,
                        })));
                    }
                    continue;
                }
            };
            let path = diary_domain::source::path::normalize(&source.rel, root.insensitive)
                .map_err(|_| ViewError::Stale)?;
            if super::root::source_id(&root, &path) != source.id {
                continue;
            }
            let result = super::adapter::read_view(
                &root,
                &configured.kind,
                &path,
                generation,
                &source.token,
            )?;
            if cancelled() {
                return Err(ViewError::Stale);
            }
            return Ok(result);
        }
        Err(ViewError::Stale)
    }

    fn import_date(
        &self,
        id: &str,
        date: DiaryDate,
        generation: u64,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<ImportListing, ImportError> {
        check(cancelled)?;
        let (zone, rules) = (self.capture)().map_err(zone_error)?;
        let context = ImportContext {
            id: id.into(),
            date,
            zone,
            config_generation: generation,
            completed_at: None,
        };
        // Zone capture must be durable before scanning, including invalid-date attempts.
        self.repository.begin(&context)?;
        let outcome = self
            .scan(context, &rules, cancelled)
            .and_then(|r| check(cancelled).map(|()| r));
        let mut result = match outcome {
            Ok(r) => r,
            Err(e) => {
                // Best effort: an abandoned incomplete row is invisible but never reclaimed.
                let _ = self.repository.discard(id);
                return Err(e);
            }
        };
        let at = now();
        if let Err(e) = self.repository.complete(&result, at) {
            let _ = self.repository.discard(id);
            return Err(e);
        }
        result.context.completed_at = Some(at);
        Ok(result)
    }
    fn list(
        &self,
        import_id: &str,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<ImportListing, ImportError> {
        check(cancelled)?;
        let result = self.repository.snapshot(import_id)?;
        check(cancelled)?;
        Ok(result)
    }
    fn resolve(
        &self,
        import_id: &str,
        source_id: &str,
        digest: &[u8; 32],
        authority: Authority,
        generation: u64,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), ImportError> {
        // Refuse stale configuration before reading the snapshot or source.
        if self.repository.context(import_id)?.config_generation != generation {
            return Err(ImportError::Stale);
        }
        let result = self.list(import_id, cancelled)?;
        let source = result
            .included
            .iter()
            .map(|entry| &entry.source)
            .chain(&result.warnings)
            .find(|s| s.id == source_id)
            .ok_or(ImportError::NotFound)?;
        if &source.token.sha256 != digest {
            return Err(ImportError::Stale);
        }
        // Only this snapshot identity is re-observed. Never rescan or substitute a
        // different source when the original path/root is missing or replaced.
        let mut fresh = None;
        for (configured, root) in self.sources.iter().zip(resolve_roots(&self.sources)) {
            check(cancelled)?;
            let Ok(root) = root else { continue };
            let path = diary_domain::source::path::normalize(&source.rel, root.insensitive)
                .map_err(|_| ImportError::Stale)?;
            if super::root::source_id(&root, &path) != source.id {
                continue;
            }
            let (outcome, mut observed) =
                super::adapter::observe(&root, &configured.kind, &path, generation)
                    .map_err(|_| ImportError::Stale)?;
            observed.token.import_id.clone_from(&source.token.import_id);
            if !matches!(outcome, SourceOutcome::Ok(_)) || observed.token != source.token {
                return Err(ImportError::Stale);
            }
            fresh = Some(observed);
            break;
        }
        let source = fresh.as_ref().ok_or(ImportError::Stale)?;
        if misplaced(source) && authority == Authority::Filename {
            return Err(ImportError::FilenameDirectoryMismatch);
        }
        let instant = choose(&source.dating, authority).ok_or(ImportError::InvalidAuthority)?;
        check(cancelled)?;
        self.repository.save_resolution(
            source_id,
            digest,
            &Resolution {
                authority,
                instant,
                decided_at: now(),
            },
        )
    }
}

#[cfg(test)]
mod tests;

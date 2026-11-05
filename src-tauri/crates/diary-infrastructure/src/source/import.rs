//! Captured-zone imports over the existing bounded, no-follow scanner.
use super::{
    root::{RootError, resolve_roots},
    scan::scan_selected,
};
use crate::{
    config::Source,
    zone::{SYSTEM_DATABASE, SystemZone, ZoneError},
};
use diary_application::{
    imports::*,
    source::{ScanReport, SourceListing, SourceOutcome, Unavailable},
};
use diary_domain::{
    authentic::DiaryDate,
    source::{
        stamp::{Dating, StampError},
        window::{ImportZone, WindowError, Zone, import_window, include},
    },
};
use std::{
    collections::{BTreeMap, BTreeSet},
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
        zone: &dyn Zone,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<ImportListing, ImportError> {
        let window = import_window(context.date, zone).map_err(window_error)?;
        let dirs: BTreeSet<_> = window
            .utc_dates
            .iter()
            .map(|d| format!("{:04}/{:02}/{:02}", d.year(), d.month(), d.day()))
            .collect();
        let mut scan = ScanReport::default();
        let mut remaining_bytes: usize = 16 * 1024 * 1024;
        let mut candidates: usize = 0;
        let mut entries = 0;
        let mut roots_by_namespace = BTreeMap::new();
        for (source, root) in self.sources.iter().zip(resolve_roots(&self.sources)) {
            check(cancelled)?;
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
                Some(&dirs),
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
                scan.collisions.push(group);
            }
            for (path, outcome) in report.failures {
                if path.len() + 128 > remaining_bytes {
                    scan.incomplete = true;
                    break;
                }
                remaining_bytes -= path.len() + 128;
                scan.failures.push((path, outcome));
            }
            for mut listing in report.listings {
                let bytes = listing.rel.len() + listing.token.key.len() + 512;
                if bytes > remaining_bytes {
                    scan.incomplete = true;
                    break;
                }
                remaining_bytes -= bytes;
                roots_by_namespace.insert(listing.token.root_namespace, root.canonical.clone());
                listing.token.import_id = Some(context.id.clone());
                self.repository.bind_source(
                    &listing.id,
                    &listing.token.root_namespace,
                    &listing.token.key,
                )?;
                scan.listings.push(listing);
            }
            if candidates >= 10_000 || entries >= 100_000 || remaining_bytes < 512 {
                scan.incomplete = true;
                break;
            }
        }
        check(cancelled)?;
        let mut result = ImportListing {
            context,
            included: Vec::new(),
            warnings: Vec::new(),
            misplaced: Vec::new(),
            scan,
        };
        for source in result.scan.listings.clone() {
            if misplaced(&source) {
                result.misplaced.push(source.id.clone());
            }
            let instant = match source.dating {
                Dating::Agreed(t) => Some(t),
                _ => self
                    .repository
                    .resolution(&source.id, &source.token.sha256)?
                    .filter(|r| choose(&source.dating, r.authority) == Some(r.instant))
                    .map(|r| r.instant),
            };
            if let Some(instant) = instant {
                if include(&window, zone, instant.unix_ms).map_err(window_error)? {
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
impl ImportStore for Store {
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
        let mut result = self.scan(context, &rules, cancelled)?;
        check(cancelled)?;
        let at = now();
        self.repository.complete(id, at)?;
        result.context.completed_at = Some(at);
        Ok(result)
    }
    fn list(
        &self,
        import_id: &str,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<ImportListing, ImportError> {
        let context = self.repository.context(import_id)?;
        let (captured, rules) =
            SystemZone::from_database(std::path::Path::new(SYSTEM_DATABASE), &context.zone.name)
                .map_err(zone_error)?;
        // Never silently reinterpret old provenance using a newly installed tzdb.
        if captured != context.zone {
            return Err(ImportError::TimezoneDatabase);
        }
        self.scan(context, &rules, cancelled)
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
        let result = self.list(import_id, cancelled)?;
        if result.context.config_generation != generation {
            return Err(ImportError::Stale);
        }
        let source = result
            .scan
            .listings
            .iter()
            .find(|s| s.id == source_id)
            .ok_or(ImportError::NotFound)?;
        if &source.token.sha256 != digest {
            return Err(ImportError::Stale);
        }
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

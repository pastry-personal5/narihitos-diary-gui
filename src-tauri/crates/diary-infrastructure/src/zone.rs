//! Captured IANA rules loaded from the same system database as the recorded version.
use diary_domain::{
    authentic::DiaryDate,
    source::window::{ImportZone, WindowError, Zone},
};
use jiff::{
    Timestamp,
    civil::Date,
    tz::{AmbiguousOffset, TimeZone, TimeZoneDatabase},
};
use std::{io::Read, path::Path};

pub(crate) const SYSTEM_DATABASE: &str = "/var/db/timezone/zoneinfo";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoneError {
    Unnamed,
    Database,
    Version,
}

pub struct SystemZone(TimeZone);

impl SystemZone {
    pub fn capture() -> Result<(ImportZone, Self), ZoneError> {
        Self::capture_from(Path::new("/etc/localtime"), Path::new(SYSTEM_DATABASE))
    }

    pub(crate) fn capture_from(
        localtime: &Path,
        database: &Path,
    ) -> Result<(ImportZone, Self), ZoneError> {
        // Read macOS's named system link afresh for every import. Jiff's system()
        // cache lasts five minutes and would miss immediate travel/Settings changes.
        let database = database.canonicalize().map_err(|_| ZoneError::Database)?;
        let target = localtime.canonicalize().map_err(|_| ZoneError::Unnamed)?;
        let name = target
            .strip_prefix(&database)
            .ok()
            .and_then(|p| p.to_str())
            .filter(|s| !s.is_empty() && s.len() <= 128)
            .ok_or(ZoneError::Unnamed)?;
        let captured = Self::from_database(&database, name)?;
        if localtime.canonicalize().map_err(|_| ZoneError::Unnamed)? != target {
            return Err(ZoneError::Unnamed);
        }
        Ok(captured)
    }

    pub fn from_database(directory: &Path, name: &str) -> Result<(ImportZone, Self), ZoneError> {
        // Resolve the system symlink once, so a tzdb installation cannot mix versions.
        let directory = directory.canonicalize().map_err(|_| ZoneError::Database)?;
        let version = read_version(&directory)?;
        let db = TimeZoneDatabase::from_dir(&directory).map_err(|_| ZoneError::Database)?;
        let zone = db.get(name).map_err(|_| ZoneError::Database)?;
        if zone.iana_name().is_none() {
            return Err(ZoneError::Unnamed);
        }
        if read_version(&directory)? != version {
            return Err(ZoneError::Version);
        }
        Ok((
            ImportZone {
                name: name.to_owned(),
                database_version: version,
            },
            Self(zone),
        ))
    }
}

fn read_version(directory: &Path) -> Result<String, ZoneError> {
    let mut value = String::new();
    std::fs::File::open(directory.join("+VERSION"))
        .map_err(|_| ZoneError::Version)?
        .take(65)
        .read_to_string(&mut value)
        .map_err(|_| ZoneError::Version)?;
    let value = value.trim();
    if value.is_empty()
        || value.len() > 32
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".-_".contains(&b))
    {
        return Err(ZoneError::Version);
    }
    Ok(value.to_owned())
}

fn civil(date: DiaryDate) -> Result<Date, WindowError> {
    Date::new(date.year() as i16, date.month() as i8, date.day() as i8)
        .map_err(|_| WindowError::Range)
}
fn diary(date: Date) -> Result<DiaryDate, WindowError> {
    DiaryDate::from_ymd(date.year() as u16, date.month() as u8, date.day() as u8)
        .ok_or(WindowError::Range)
}
impl Zone for SystemZone {
    fn local_date(&self, unix_ms: i64) -> Result<DiaryDate, WindowError> {
        let timestamp = Timestamp::from_millisecond(unix_ms).map_err(|_| WindowError::Range)?;
        diary(self.0.to_datetime(timestamp).date())
    }

    fn earliest_start(&self, date: DiaryDate) -> Result<Option<i64>, WindowError> {
        let date = civil(date)?;
        let ambiguous = self.0.to_ambiguous_timestamp(date.at(0, 0, 0, 0));
        let start = match ambiguous.offset() {
            AmbiguousOffset::Gap { .. } => {
                // Midnight can lie in the middle of a gap that began on the
                // previous date. Shifting midnight by the gap's duration would
                // miss the first real instants. Use the transition itself.
                let before = ambiguous.earlier().map_err(|_| WindowError::Zone)?;
                self.0
                    .following(before)
                    .next()
                    .ok_or(WindowError::Zone)?
                    .timestamp()
            }
            _ => ambiguous.earlier().map_err(|_| WindowError::Zone)?,
        };
        if self.0.to_datetime(start).date() != date {
            return Ok(None);
        }
        Ok(Some(start.as_millisecond()))
    }
}

#[cfg(test)]
mod tests;

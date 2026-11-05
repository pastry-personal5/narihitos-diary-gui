//! Civil-date import sets. Zone rules are supplied by an adapter, never by an offset guess.
use crate::authentic::DiaryDate;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportZone {
    pub name: String,
    pub database_version: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowError {
    InvalidDiaryDate,
    Range,
    Zone,
}

pub trait Zone {
    fn local_date(&self, unix_ms: i64) -> Result<DiaryDate, WindowError>;
    /// Earliest real instant on this date; `None` means the entire date was skipped.
    fn earliest_start(&self, date: DiaryDate) -> Result<Option<i64>, WindowError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportWindow {
    pub dates: [DiaryDate; 3],
    pub start_ms: i64,
    pub end_ms: i64,
    pub utc_dates: Vec<DiaryDate>,
}

fn next_valid(zone: &dyn Zone, mut date: DiaryDate) -> Result<i64, WindowError> {
    // Bound even a broken adapter. Real IANA skipped-date transitions skip one day.
    for _ in 0..8 {
        if let Some(start) = zone.earliest_start(date)? {
            return Ok(start);
        }
        date = date.next().ok_or(WindowError::Range)?;
    }
    Err(WindowError::Zone)
}

pub fn import_window(date: DiaryDate, zone: &dyn Zone) -> Result<ImportWindow, WindowError> {
    if zone.earliest_start(date)?.is_none() {
        return Err(WindowError::InvalidDiaryDate);
    }
    let before = date.previous().ok_or(WindowError::Range)?;
    let after = date.next().ok_or(WindowError::Range)?;
    let start_ms = next_valid(zone, before)?;
    let end_ms = next_valid(zone, after.next().ok_or(WindowError::Range)?)?;
    if end_ms <= start_ms {
        return Err(WindowError::Zone);
    }
    let first = utc_date(start_ms)?;
    let last = utc_date(end_ms - 1)?;
    let mut utc_dates = vec![first];
    while *utc_dates.last().expect("nonempty") < last {
        if utc_dates.len() >= 8 {
            return Err(WindowError::Zone);
        }
        utc_dates.push(utc_dates.last().unwrap().next().ok_or(WindowError::Range)?);
    }
    Ok(ImportWindow {
        dates: [before, date, after],
        start_ms,
        end_ms,
        utc_dates,
    })
}

pub fn include(window: &ImportWindow, zone: &dyn Zone, unix_ms: i64) -> Result<bool, WindowError> {
    Ok(window.dates.contains(&zone.local_date(unix_ms)?))
}

/// Gregorian epoch-day conversion; UTC directory enumeration has no zone dependency.
pub fn utc_date(unix_ms: i64) -> Result<DiaryDate, WindowError> {
    let z = unix_ms.div_euclid(86_400_000) + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    let year = u16::try_from(year).map_err(|_| WindowError::Range)?;
    DiaryDate::from_ymd(year, month as u8, day as u8).ok_or(WindowError::Range)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::stamp::header_created_at;
    struct Fake {
        starts: Vec<(DiaryDate, Option<i64>)>,
    }
    impl Zone for Fake {
        fn earliest_start(&self, date: DiaryDate) -> Result<Option<i64>, WindowError> {
            self.starts
                .iter()
                .find(|(d, _)| *d == date)
                .map(|(_, t)| *t)
                .ok_or(WindowError::Zone)
        }
        fn local_date(&self, instant: i64) -> Result<DiaryDate, WindowError> {
            self.starts
                .iter()
                .filter_map(|(d, t)| t.map(|t| (*d, t)))
                .rev()
                .find(|(_, t)| *t <= instant)
                .map(|(d, _)| d)
                .ok_or(WindowError::Zone)
        }
    }
    #[test]
    fn calendar_set_accepts_variable_days_and_skipped_neighbours() {
        let base = header_created_at("2024-02-28T00:00:00Z").unwrap().unix_ms;
        let d = DiaryDate::parse("2024-02-29").unwrap();
        for durations in [
            [24, 24, 24],
            [23, 24, 25],
            [25, 23, 24],
            [24, 24, 0],
            [0, 24, 24],
        ] {
            let dates = [
                d.previous().unwrap(),
                d,
                d.next().unwrap(),
                d.next().unwrap().next().unwrap(),
            ];
            let mut start = base;
            let mut starts = Vec::new();
            for (i, date) in dates.into_iter().enumerate() {
                starts.push((
                    date,
                    if i < 3 && durations[i] == 0 {
                        None
                    } else {
                        Some(start)
                    },
                ));
                if i < 3 {
                    start += durations[i] * 3_600_000;
                }
            }
            let zone = Fake { starts };
            let w = import_window(d, &zone).unwrap();
            assert_eq!((w.start_ms, w.end_ms), (base, start));
            for t in (base..start).step_by(60_000) {
                assert!(include(&w, &zone, t).unwrap());
            }
            assert!(!include(&w, &zone, start).unwrap());
        }
        assert_eq!(
            DiaryDate::parse("2024-01-01")
                .unwrap()
                .previous()
                .unwrap()
                .to_string(),
            "2023-12-31"
        );
        assert_eq!(DiaryDate::parse("9999-12-31").unwrap().next(), None);
        assert_eq!(DiaryDate::parse("0001-01-01").unwrap().previous(), None);
        assert_eq!(utc_date(-1).unwrap().to_string(), "1969-12-31");
    }
}

//! Injectable local clock. Tests never read the system clock.

/// A wall-clock instant with its local calendar fields and UTC offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalStamp {
    pub year: i32,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub epoch_secs: i64,
    pub offset_secs: i32,
    pub millis: u16,
}

// Howard Hinnant's civil-calendar algorithms (days since 1970-01-01).
fn civil_from_days(z: i64) -> (i32, u8, u8) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u8;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u8;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y as i32, m, d)
}

fn days_from_civil(y: i32, m: u8, d: u8) -> i64 {
    let y = i64::from(y) - i64::from(m <= 2);
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let m = i64::from(m);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

impl LocalStamp {
    pub fn from_epoch(epoch_secs: i64, offset_secs: i32, millis: u16) -> Self {
        let local = epoch_secs + i64::from(offset_secs);
        let (year, month, day) = civil_from_days(local.div_euclid(86_400));
        let s = local.rem_euclid(86_400);
        Self {
            year,
            month,
            day,
            hour: (s / 3600) as u8,
            minute: (s % 3600 / 60) as u8,
            second: (s % 60) as u8,
            epoch_secs,
            offset_secs,
            millis,
        }
    }

    pub fn date_string(&self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }

    pub fn rfc3339(&self) -> String {
        let sign = if self.offset_secs < 0 { '-' } else { '+' };
        let off = self.offset_secs.unsigned_abs();
        format!(
            "{}T{:02}:{:02}:{:02}.{:03}{sign}{:02}:{:02}",
            self.date_string(),
            self.hour,
            self.minute,
            self.second,
            self.millis,
            off / 3600,
            off % 3600 / 60
        )
    }

    /// Parses exactly the format `rfc3339` produces; returns epoch milliseconds.
    pub fn parse_rfc3339(s: &str) -> Option<i64> {
        let b = s.as_bytes();
        if b.len() != 29
            || b[4] != b'-'
            || b[7] != b'-'
            || b[10] != b'T'
            || b[13] != b':'
            || b[16] != b':'
            || b[19] != b'.'
            || b[26] != b':'
            || !matches!(b[23], b'+' | b'-')
        {
            return None;
        }
        let num = |a: usize, z: usize| -> Option<i64> {
            let t = s.get(a..z)?;
            t.bytes()
                .all(|c| c.is_ascii_digit())
                .then(|| t.parse().ok())?
        };
        let (y, mo, d) = (num(0, 4)?, num(5, 7)?, num(8, 10)?);
        let (h, mi, sec, ms) = (num(11, 13)?, num(14, 16)?, num(17, 19)?, num(20, 23)?);
        let (oh, om) = (num(24, 26)?, num(27, 29)?);
        if !(1..=12).contains(&mo)
            || !(1..=31).contains(&d)
            || h > 23
            || mi > 59
            || sec > 59
            || om > 59
            || oh > 23
        {
            return None;
        }
        let days = days_from_civil(y as i32, mo as u8, d as u8);
        if civil_from_days(days) != (y as i32, mo as u8, d as u8) {
            return None; // for example February 30
        }
        let off = (oh * 3600 + om * 60) * if b[23] == b'-' { -1 } else { 1 };
        Some(((days * 86_400 + h * 3600 + mi * 60 + sec - off) * 1000) + ms)
    }
}

pub trait Clock: Send + Sync {
    fn now(&self) -> LocalStamp;
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> LocalStamp {
        let mut ts = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        // SAFETY: valid out-pointer to a timespec.
        unsafe { libc::clock_gettime(libc::CLOCK_REALTIME, &mut ts) };
        let secs: libc::time_t = ts.tv_sec;
        // SAFETY: zeroed tm is a valid out buffer for localtime_r.
        let mut tm: libc::tm = unsafe { std::mem::zeroed() };
        let offset = if unsafe { libc::localtime_r(&secs, &mut tm) }.is_null() {
            0
        } else {
            tm.tm_gmtoff as i32
        };
        LocalStamp::from_epoch(secs, offset, (ts.tv_nsec / 1_000_000) as u16)
    }
}

/// Manually advanced clock for deterministic tests.
#[cfg(test)]
pub struct FakeClock(pub std::sync::Mutex<LocalStamp>);

#[cfg(test)]
impl FakeClock {
    pub fn at(rfc3339: &str) -> std::sync::Arc<Self> {
        let ms = LocalStamp::parse_rfc3339(rfc3339).unwrap();
        let offset = {
            let sign = if &rfc3339[23..24] == "-" { -1 } else { 1 };
            sign * (rfc3339[24..26].parse::<i32>().unwrap() * 3600
                + rfc3339[27..29].parse::<i32>().unwrap() * 60)
        };
        std::sync::Arc::new(Self(std::sync::Mutex::new(LocalStamp::from_epoch(
            ms.div_euclid(1000),
            offset,
            ms.rem_euclid(1000) as u16,
        ))))
    }
    pub fn set(&self, epoch_secs: i64, offset_secs: i32) {
        *self.0.lock().unwrap() = LocalStamp::from_epoch(epoch_secs, offset_secs, 0);
    }
    pub fn advance(&self, secs: i64) {
        let mut g = self.0.lock().unwrap();
        *g = LocalStamp::from_epoch(g.epoch_secs + secs, g.offset_secs, g.millis);
    }
}

#[cfg(test)]
impl Clock for FakeClock {
    fn now(&self) -> LocalStamp {
        *self.0.lock().unwrap()
    }
}

#[cfg(test)]
mod tests;

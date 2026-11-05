#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Precision {
    Second,
    Milli,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Instant {
    pub unix_ms: i64,
    pub precision: Precision,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StampError {
    Format,
    Offset,
    Calendar,
    DirMismatch,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Dating {
    Agreed(Instant),
    Disagree {
        header: Instant,
        filename: Instant,
    },
    HeaderOnly {
        header: Instant,
        filename: StampError,
    },
    FilenameOnly {
        filename: Instant,
        header: StampError,
    },
    Undated {
        header: StampError,
        filename: StampError,
    },
}
impl Dating {
    pub fn from_evidence(
        header: Result<Instant, StampError>,
        filename: Result<Instant, StampError>,
    ) -> Self {
        match (header, filename) {
            (Ok(h), Ok(f)) if compare(h, f) => Self::Agreed(h),
            (Ok(header), Ok(filename)) => Self::Disagree { header, filename },
            (Ok(header), Err(filename)) => Self::HeaderOnly { header, filename },
            (Err(header), Ok(filename)) => Self::FilenameOnly { filename, header },
            (Err(header), Err(filename)) => Self::Undated { header, filename },
        }
    }
}
fn number(s: &str, start: usize, end: usize) -> Result<i64, StampError> {
    let p = s.get(start..end).ok_or(StampError::Format)?;
    if !p.bytes().all(|c| c.is_ascii_digit()) {
        return Err(StampError::Format);
    }
    p.parse().map_err(|_| StampError::Format)
}
fn instant(y: i64, m: i64, d: i64, h: i64, min: i64, sec: i64, ms: i64) -> Result<i64, StampError> {
    if !(1000..=9999).contains(&y)
        || h > 23
        || min > 59
        || sec > 59
        || crate::authentic::DiaryDate::from_ymd(y as u16, m as u8, d as u8).is_none()
    {
        return Err(StampError::Calendar);
    }
    // Proleptic Gregorian civil-to-days, epoch 1970-01-01.
    let y = y - i64::from(m <= 2);
    let era = y.div_euclid(400);
    let yo = y - era * 400;
    let mp = m + if m > 2 { -3 } else { 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let days = era * 146097 + yo * 365 + yo / 4 - yo / 100 + doy - 719468;
    Ok(((days * 24 + h) * 60 + min) * 60000 + sec * 1000 + ms)
}
pub fn header_created_at(s: &str) -> Result<Instant, StampError> {
    let b = s.as_bytes();
    if b.len() < 19
        || b[4] != b'-'
        || b[7] != b'-'
        || b[10] != b'T'
        || b[13] != b':'
        || b[16] != b':'
    {
        return Err(StampError::Format);
    }
    let mut end = 19;
    let mut ms = 0;
    let mut precision = Precision::Second;
    if b.get(end) == Some(&b'.') {
        end += 1;
        let start = end;
        while b.get(end).is_some_and(u8::is_ascii_digit) {
            end += 1;
        }
        let count = end - start;
        if !(1..=3).contains(&count) {
            return Err(StampError::Format);
        }
        ms = number(s, start, end)? * 10_i64.pow((3 - count) as u32);
        precision = Precision::Milli;
    }
    let offset = match s.get(end..) {
        Some("Z") => 0,
        Some(t)
            if t.len() == 6
                && matches!(t.as_bytes()[0], b'+' | b'-')
                && t.as_bytes()[3] == b':' =>
        {
            let h = number(t, 1, 3)?;
            let m = number(t, 4, 6)?;
            if h > 23 || m > 59 {
                return Err(StampError::Offset);
            }
            (h * 60 + m) * 60000 * if t.starts_with('-') { -1 } else { 1 }
        }
        _ => return Err(StampError::Offset),
    };
    Ok(Instant {
        unix_ms: instant(
            number(s, 0, 4)?,
            number(s, 5, 7)?,
            number(s, 8, 10)?,
            number(s, 11, 13)?,
            number(s, 14, 16)?,
            number(s, 17, 19)?,
            ms,
        )? - offset,
        precision,
    })
}
fn stem_base(s: &str, dir: (i32, u8, u8)) -> Result<Instant, StampError> {
    let b = s.as_bytes();
    if b.len() < 19
        || b[4] != b'-'
        || b[7] != b'-'
        || b[10] != b'_'
        || b[13] != b'-'
        || b[16] != b'-'
    {
        return Err(StampError::Format);
    }
    let (y, m, d) = (number(s, 0, 4)?, number(s, 5, 7)?, number(s, 8, 10)?);
    let unix_ms = instant(
        y,
        m,
        d,
        number(s, 11, 13)?,
        number(s, 14, 16)?,
        number(s, 17, 19)?,
        0,
    )?;
    if (y as i32, m as u8, d as u8) != dir {
        return Err(StampError::DirMismatch);
    }
    Ok(Instant {
        unix_ms,
        precision: Precision::Second,
    })
}
pub fn ayumi_stem(s: &str, dir: (i32, u8, u8)) -> Result<Instant, StampError> {
    if s.len() != 23 || s.as_bytes()[19] != b'-' {
        return Err(StampError::Format);
    }
    let mut t = stem_base(s, dir)?;
    t.unix_ms += number(s, 20, 23)?;
    t.precision = Precision::Milli;
    Ok(t)
}
pub fn valid_uuid(s: &str) -> bool {
    s.len() == 36
        && s.bytes().enumerate().all(|(i, b)| match i {
            8 | 13 | 18 | 23 => b == b'-',
            14 => b == b'4',
            19 => matches!(b, b'8' | b'9' | b'a' | b'b'),
            _ => b.is_ascii_digit() || (b'a'..=b'f').contains(&b),
        })
}
pub fn narihito_stem(s: &str, dir: (i32, u8, u8)) -> Result<(Instant, String), StampError> {
    if s.len() != 56
        || s.as_bytes()[19] != b'-'
        || !valid_uuid(s.get(20..).ok_or(StampError::Format)?)
    {
        return Err(StampError::Format);
    }
    Ok((stem_base(s, dir)?, s[20..].into()))
}
pub fn compare(h: Instant, f: Instant) -> bool {
    if h.precision == Precision::Second || f.precision == Precision::Second {
        h.unix_ms.div_euclid(1000) == f.unix_ms.div_euclid(1000)
    } else {
        h.unix_ms == f.unix_ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_offsets_precision_and_calendar() {
        assert_eq!(
            header_created_at("1970-01-01T00:00:00Z").unwrap().unix_ms,
            0
        );
        let h = header_created_at("2024-02-29T09:00:00.123+09:00").unwrap();
        assert_eq!(
            h,
            ayumi_stem("2024-02-29_00-00-00-123", (2024, 2, 29)).unwrap()
        );
        assert!(compare(
            h,
            header_created_at("2024-02-29T00:00:00Z").unwrap()
        ));
        assert!(!compare(
            h,
            header_created_at("2024-02-29T00:00:01Z").unwrap()
        ));
        for s in [
            "2025-02-29T00:00:00Z",
            "2100-02-29T00:00:00Z",
            "2024-13-01T00:00:00Z",
            "2024-01-01T24:00:00Z",
            "2024-01-01T00:00:60Z",
        ] {
            assert_eq!(header_created_at(s), Err(StampError::Calendar));
        }
        for s in [
            "2024-01-01T00:00:00",
            "2024-01-01T00:00:00+0900",
            "2024-01-01T00:00:00+24:00",
            "2024-01-01T00:00:00+00:60",
        ] {
            assert_eq!(header_created_at(s), Err(StampError::Offset));
        }
        assert_eq!(
            header_created_at("2024-01-01T00:00:00.1234Z"),
            Err(StampError::Format)
        );
        assert_eq!(
            ayumi_stem("2024-02-29_00-00-00-123", (2024, 3, 1)),
            Err(StampError::DirMismatch)
        );
        let stem = "2024-02-29_00-00-00-01234567-89ab-4def-8123-456789abcdef";
        assert!(narihito_stem(stem, (2024, 2, 29)).is_ok());
        assert!(narihito_stem(&stem.to_uppercase(), (2024, 2, 29)).is_err());
        assert!(narihito_stem(&stem.replace("4def", "3def"), (2024, 2, 29)).is_err());
    }
    #[test]
    fn independent_evidence() {
        let a = header_created_at("2024-01-01T00:00:00Z").unwrap();
        let b = header_created_at("2024-01-01T00:00:01Z").unwrap();
        assert!(
            matches!(Dating::from_evidence(Ok(a),Ok(b)),Dating::Disagree{header,filename} if header==a && filename==b)
        );
        assert!(matches!(
            Dating::from_evidence(Ok(a), Err(StampError::Format)),
            Dating::HeaderOnly { .. }
        ));
        assert!(matches!(
            Dating::from_evidence(Err(StampError::Offset), Ok(b)),
            Dating::FilenameOnly { .. }
        ));
        assert!(matches!(
            Dating::from_evidence(Err(StampError::Offset), Err(StampError::Format)),
            Dating::Undated { .. }
        ));
    }
}

//! Bounded Gregorian civil navigation. No clocks, elapsed-time arithmetic or zones.
use crate::authentic::DiaryDate;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Movement {
    Month,
    PreviousMonth,
    NextMonth,
    PreviousDay,
    NextDay,
    PreviousWeek,
    NextWeek,
    Today,
}
impl Movement {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "month" => Self::Month,
            "previous_month" => Self::PreviousMonth,
            "next_month" => Self::NextMonth,
            "previous_day" => Self::PreviousDay,
            "next_day" => Self::NextDay,
            "previous_week" => Self::PreviousWeek,
            "next_week" => Self::NextWeek,
            "today" => Self::Today,
            _ => return None,
        })
    }
}

pub fn move_date(anchor: DiaryDate, today: DiaryDate, movement: Movement) -> Option<DiaryDate> {
    use Movement::*;
    match movement {
        Month => Some(anchor),
        Today => Some(today),
        PreviousDay => anchor.previous(),
        NextDay => anchor.next(),
        PreviousWeek | NextWeek => (0..7).try_fold(anchor, |d, _| {
            if movement == PreviousWeek {
                d.previous()
            } else {
                d.next()
            }
        }),
        PreviousMonth | NextMonth => {
            let (year, month) = match (movement, anchor.month()) {
                (PreviousMonth, 1) => (anchor.year().checked_sub(1)?, 12),
                (PreviousMonth, m) => (anchor.year(), m - 1),
                (_, 12) => (anchor.year().checked_add(1)?, 1),
                (_, m) => (anchor.year(), m + 1),
            };
            (1..=anchor.day())
                .rev()
                .find_map(|d| DiaryDate::from_ymd(year, month, d))
        }
    }
}

/// Sunday is 0. 0001-01-01 was Monday in the proleptic Gregorian calendar.
pub fn weekday(date: DiaryDate) -> u8 {
    let y = u32::from(date.year()) - 1;
    let before_year = 365 * y + y / 4 - y / 100 + y / 400;
    let before_month: u32 = (1..date.month())
        .map(|m| {
            (28..=31)
                .rev()
                .find(|d| DiaryDate::from_ymd(date.year(), m, *d).is_some())
                .unwrap() as u32
        })
        .sum();
    ((before_year + before_month + u32::from(date.day())) % 7) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    fn d(s: &str) -> DiaryDate {
        DiaryDate::parse(s).unwrap()
    }
    #[test]
    fn month_clamping_leaps_and_year_boundaries() {
        for (from, movement, to) in [
            ("2024-01-31", Movement::NextMonth, "2024-02-29"),
            ("2100-03-31", Movement::PreviousMonth, "2100-02-28"),
            ("2024-12-31", Movement::NextDay, "2025-01-01"),
            ("2024-03-01", Movement::PreviousWeek, "2024-02-23"),
            ("2024-12-29", Movement::NextWeek, "2025-01-05"),
        ] {
            assert_eq!(move_date(d(from), d(from), movement), Some(d(to)));
        }
        for (date, movement) in [
            ("0001-01-01", Movement::PreviousDay),
            ("0001-01-31", Movement::PreviousMonth),
            ("9999-12-31", Movement::NextDay),
            ("9999-12-01", Movement::NextMonth),
            ("9999-12-30", Movement::NextWeek),
        ] {
            assert_eq!(move_date(d(date), d(date), movement), None);
        }
        assert_eq!(weekday(d("0001-01-01")), 1);
        assert_eq!(weekday(d("2024-02-29")), 4);
        assert_eq!(weekday(d("2026-11-05")), 4);
    }
}

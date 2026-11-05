use super::*;

fn ms(s: &str) -> i64 {
    LocalStamp::parse_rfc3339(s).unwrap()
}

#[test]
fn rfc3339_formats_offset_east_and_west() {
    assert_eq!(
        LocalStamp::from_epoch(0, 9 * 3600, 5).rfc3339(),
        "1970-01-01T09:00:00.005+09:00"
    );
    assert_eq!(
        LocalStamp::from_epoch(0, -(5 * 3600 + 1800), 0).rfc3339(),
        "1969-12-31T18:30:00.000-05:30"
    );
}

#[test]
fn date_changes_at_local_midnight_not_utc_midnight() {
    let last = ms("2026-11-05T23:59:59.000+09:00") / 1000;
    assert_eq!(
        LocalStamp::from_epoch(last, 9 * 3600, 0).date_string(),
        "2026-11-05"
    );
    assert_eq!(
        LocalStamp::from_epoch(last + 1, 9 * 3600, 0).date_string(),
        "2026-11-06"
    );
}

#[test]
fn dst_day_has_23_or_25_hours() {
    let day = |a: &str, b: &str| (ms(b) - ms(a)) / 3_600_000;
    assert_eq!(
        day(
            "2026-11-01T00:00:00.000-04:00",
            "2026-11-02T00:00:00.000-05:00"
        ),
        25
    );
    assert_eq!(
        day(
            "2026-03-08T00:00:00.000-05:00",
            "2026-03-09T00:00:00.000-04:00"
        ),
        23
    );
}

#[test]
fn parse_rfc3339_round_trips_and_rejects_malformed() {
    let s = LocalStamp::from_epoch(1_793_000_000, 3600, 42);
    assert_eq!(
        LocalStamp::parse_rfc3339(&s.rfc3339()),
        Some(1_793_000_000_042)
    );
    for bad in [
        "2026-11-05 09:00:00.000+09:00",
        "2026-11-05t09:00:00.000+09:00",
        "2026-11-05T09:00:00.000+0900",
        "2026-13-05T09:00:00.000+09:00",
        "2026-11-05T25:00:00.000+09:00",
        "2026-11-05T09:00:00.000+09:00x",
        "2026-11-05T09:00:00+09:00",
        "",
    ] {
        assert_eq!(LocalStamp::parse_rfc3339(bad), None, "{bad}");
    }
}

#[test]
fn system_clock_reports_plausible_now() {
    let now = SystemClock.now();
    assert!(now.epoch_secs > 1_700_000_000);
    assert!(LocalStamp::parse_rfc3339(&now.rfc3339()).is_some());
}

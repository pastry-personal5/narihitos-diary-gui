use super::*;
use diary_domain::source::{
    stamp::header_created_at,
    window::{import_window, include},
};

fn date(s: &str) -> DiaryDate {
    DiaryDate::parse(s).unwrap()
}
fn ms(s: &str) -> i64 {
    header_created_at(s).unwrap().unix_ms
}
fn zone(name: &str) -> SystemZone {
    SystemZone::from_database(Path::new(SYSTEM_DATABASE), name)
        .unwrap()
        .1
}

#[test]
fn system_capture_records_named_rules_and_version_and_rejects_missing_version() {
    let (provenance, rules) = SystemZone::capture().unwrap();
    assert!(!provenance.name.is_empty());
    assert_eq!(
        provenance.database_version,
        read_version(Path::new(SYSTEM_DATABASE)).unwrap()
    );
    assert!(rules.earliest_start(date("2024-02-29")).unwrap().is_some());
    let temp = tempfile::tempdir().unwrap();
    assert_eq!(
        SystemZone::from_database(temp.path(), "UTC").err(),
        Some(ZoneError::Version)
    );
    std::fs::write(temp.path().join("+VERSION"), "x".repeat(100)).unwrap();
    assert_eq!(
        SystemZone::from_database(temp.path(), "UTC").err(),
        Some(ZoneError::Version)
    );
}

#[test]
fn complete_inclusion_sets_and_directory_bounds() {
    // Exact expected UTC boundaries for each three-date set. Check every minute,
    // including adjacent excluded days, rather than a few included examples.
    for (name, selected, first, end) in [
        (
            "UTC",
            "2024-01-01",
            "2023-12-31T00:00:00Z",
            "2024-01-03T00:00:00Z",
        ),
        (
            "Asia/Seoul",
            "2024-02-29",
            "2024-02-27T15:00:00Z",
            "2024-03-01T15:00:00Z",
        ),
        (
            "Asia/Kolkata",
            "2024-02-29",
            "2024-02-27T18:30:00Z",
            "2024-03-01T18:30:00Z",
        ),
        (
            "America/New_York",
            "2024-03-10",
            "2024-03-09T05:00:00Z",
            "2024-03-12T04:00:00Z",
        ),
        (
            "America/New_York",
            "2024-11-03",
            "2024-11-02T04:00:00Z",
            "2024-11-05T05:00:00Z",
        ),
        (
            "Australia/Lord_Howe",
            "2024-10-06",
            "2024-10-04T13:30:00Z",
            "2024-10-07T13:00:00Z",
        ),
        (
            "America/Sao_Paulo",
            "2015-10-18",
            "2015-10-17T03:00:00Z",
            "2015-10-20T02:00:00Z",
        ),
        (
            "America/Havana",
            "2024-11-03",
            "2024-11-02T04:00:00Z",
            "2024-11-05T05:00:00Z",
        ),
        (
            "Pacific/Apia",
            "2011-12-29",
            "2011-12-28T10:00:00Z",
            "2011-12-30T10:00:00Z",
        ),
        (
            "Pacific/Apia",
            "2011-12-31",
            "2011-12-30T10:00:00Z",
            "2012-01-01T10:00:00Z",
        ),
    ] {
        let z = zone(name);
        let w = import_window(date(selected), &z).unwrap();
        let (first, end) = (ms(first), ms(end));
        assert_eq!((w.start_ms, w.end_ms), (first, end), "{name} {selected}");
        for t in ((first - 86_400_000)..(end + 86_400_000)).step_by(60_000) {
            assert_eq!(
                include(&w, &z, t).unwrap(),
                (first..end).contains(&t),
                "{name} {selected} {t}"
            );
        }
        assert!(!include(&w, &z, first - 1).unwrap());
        assert!(include(&w, &z, end - 1).unwrap());
        assert!(!include(&w, &z, end).unwrap());
        assert_eq!(
            w.utc_dates.first(),
            Some(&diary_domain::source::window::utc_date(first).unwrap())
        );
        assert_eq!(
            w.utc_dates.last(),
            Some(&diary_domain::source::window::utc_date(end - 1).unwrap())
        );
    }
}

#[test]
fn skipped_selected_date_and_midnight_disambiguation() {
    assert_eq!(
        import_window(date("2011-12-30"), &zone("Pacific/Apia")),
        Err(WindowError::InvalidDiaryDate)
    );
    assert_eq!(
        zone("America/Havana")
            .earliest_start(date("2024-11-03"))
            .unwrap(),
        Some(ms("2024-11-03T04:00:00Z"))
    );
    assert_eq!(
        zone("America/Sao_Paulo")
            .earliest_start(date("2015-10-18"))
            .unwrap(),
        Some(ms("2015-10-18T03:00:00Z"))
    );
}

#[test]
fn capture_reads_zone_changes_without_a_process_cache_and_rejects_unnamed_files() {
    let temp = tempfile::tempdir().unwrap();
    let link = temp.path().join("localtime");
    let db = Path::new(SYSTEM_DATABASE);
    std::os::unix::fs::symlink(db.join("Asia/Seoul"), &link).unwrap();
    assert_eq!(
        SystemZone::capture_from(&link, db).unwrap().0.name,
        "Asia/Seoul"
    );
    std::fs::remove_file(&link).unwrap();
    std::os::unix::fs::symlink(db.join("America/New_York"), &link).unwrap();
    assert_eq!(
        SystemZone::capture_from(&link, db).unwrap().0.name,
        "America/New_York"
    );
    std::fs::remove_file(&link).unwrap();
    std::fs::copy(db.join("Asia/Seoul"), &link).unwrap();
    assert_eq!(
        SystemZone::capture_from(&link, db).err(),
        Some(ZoneError::Unnamed)
    );
}

#[test]
fn gap_spanning_midnight_uses_transition_instead_of_shifting_midnight() {
    let z = SystemZone(TimeZone::posix("AAA0BBB,M3.2.0/-0:30,M11.1.0/0").unwrap());
    assert_eq!(
        z.earliest_start(date("2024-03-10")).unwrap(),
        Some(ms("2024-03-09T23:30:00Z"))
    );
    assert_eq!(
        z.local_date(ms("2024-03-09T23:29:59.999Z")).unwrap(),
        date("2024-03-09")
    );
    assert_eq!(
        z.local_date(ms("2024-03-09T23:30:00Z")).unwrap(),
        date("2024-03-10")
    );
}

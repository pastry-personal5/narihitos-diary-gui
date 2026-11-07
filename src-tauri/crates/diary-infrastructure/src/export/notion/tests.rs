use super::{
    client::{Client, Error},
    fake::{FakeNotion, Fault},
    http::{Http, Method},
    *,
};
use crate::{lifecycle::WorkspaceLock, workspace::Workspace};
use diary_application::export::{Applied, ExportAdapter, Observation, Unavailable, Write};
use diary_domain::authentic::DiaryDate;
use serde_json::{Value, json};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

const ROOT: &str = "0123456789abcdef0123456789abcdef";

struct Env {
    _temp: tempfile::TempDir,
    _lock: WorkspaceLock,
    ws: Workspace,
    fake: Arc<FakeNotion>,
    slept: Arc<Mutex<Vec<Duration>>>,
    notion: Notion,
}
fn env_with(page_size: usize) -> Env {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap().join("ws");
    std::fs::create_dir(&root).unwrap();
    let lock = WorkspaceLock::acquire(&root).unwrap();
    let ws = Workspace::open(&lock).unwrap();
    let fake = Arc::new(FakeNotion::new(ROOT).with_page_size(page_size));
    let slept = Arc::new(Mutex::new(Vec::new()));
    let log = slept.clone();
    let client = Client::new(
        fake.clone(),
        Arc::new(move |d| log.lock().unwrap().push(d)),
        Duration::ZERO,
    );
    let notion = ws.notion(client, ROOT).unwrap();
    Env {
        _temp: temp,
        _lock: lock,
        ws,
        fake,
        slept,
        notion,
    }
}
fn env() -> Env {
    env_with(100)
}
fn d(t: &str) -> DiaryDate {
    DiaryDate::parse(t).unwrap()
}
fn file(body: &str) -> Vec<u8> {
    format!("---\nid: x\n---\n{body}").into_bytes()
}
impl Env {
    fn seen(&self, date: &str) -> Observation {
        self.notion.observe(&d(date))
    }
    fn put(&self, date: &str, body: &str) -> Applied {
        let seen = self.seen(date);
        let write = if seen.present {
            Write::Replace
        } else {
            Write::Create
        };
        self.notion.apply(&d(date), &file(body), write, &seen)
    }
    fn titles(&self, parent: &str) -> Vec<String> {
        self.fake
            .children(parent)
            .iter()
            .map(|c| {
                self.fake
                    .title_of(c)
                    .unwrap_or_else(|| self.fake.text_of(c))
            })
            .collect()
    }
    fn month(&self) -> String {
        let year = self.fake.children(ROOT)[0].clone();
        self.fake.children(&year)[0].clone()
    }
    /// The text of each day heading in the month page, in page order.
    fn days(&self) -> Vec<String> {
        self.fake
            .children(&self.month())
            .iter()
            .filter(|c| self.fake.kind_of(c) == "heading_2")
            .map(|c| self.fake.text_of(c))
            .collect()
    }
    /// The ID of the first day heading whose text starts with `day`.
    fn heading_id(&self, day: &str) -> String {
        self.fake
            .children(&self.month())
            .into_iter()
            .find(|c| self.fake.kind_of(c) == "heading_2" && self.fake.text_of(c).starts_with(day))
            .unwrap()
    }
    /// The text of the blocks after the day's heading, up to the next day heading.
    fn range_texts(&self, day: &str) -> Vec<String> {
        let heading = self.heading_id(day);
        self.fake
            .children(&self.month())
            .into_iter()
            .skip_while(|c| *c != heading)
            .skip(1)
            .take_while(|c| self.fake.kind_of(c) != "heading_2")
            .map(|c| self.fake.text_of(&c))
            .collect()
    }
    fn digest_ok(&self, date: &str, body: &str) -> bool {
        self.seen(date).digest == self.notion.digest_of(&file(body)).unwrap()
    }
}

#[test]
fn create_builds_the_hierarchy_and_exact_content() {
    let e = env();
    assert_eq!(e.seen("2026-11-07"), Observation::absent());
    let body = "# Day\n\nHello *world*.\n\n- a\n  - b\n";
    assert_eq!(e.put("2026-11-07", body), Applied::Done);
    assert_eq!(e.titles(ROOT), ["2026"]);
    let year = e.fake.children(ROOT)[0].clone();
    assert_eq!(e.titles(&year), ["2026-11"]);
    assert_eq!(e.days(), ["2026-11-07 (SAT)"]);
    assert!(e.digest_ok("2026-11-07", body));
    let steps = e.notion.steps.states("2026-11-07");
    assert_eq!(steps.len(), 4);
    assert!(steps.iter().all(|(_, s)| s == "done"));
}

#[test]
fn days_stay_ascending_for_first_middle_and_last_insertion() {
    let e = env();
    for day in [
        "2026-11-10",
        "2026-11-05",
        "2026-11-07",
        "2026-11-20",
        "2026-11-01",
    ] {
        assert_eq!(
            e.put(day, &format!("entry {day}\n")),
            Applied::Done,
            "{day}"
        );
    }
    assert_eq!(
        e.days(),
        [
            "2026-11-01 (SUN)",
            "2026-11-05 (THU)",
            "2026-11-07 (SAT)",
            "2026-11-10 (TUE)",
            "2026-11-20 (FRI)"
        ]
    );
    for day in ["01", "05", "07", "10", "20"] {
        assert_eq!(
            e.range_texts(&format!("2026-11-{day}")),
            [format!("entry 2026-11-{day}")]
        );
    }
    // One year page and one month page only.
    assert_eq!(e.fake.children(ROOT).len(), 1);
}

#[test]
fn pagination_reads_every_page_of_a_long_month() {
    let e = env_with(2);
    for day in 1..=9 {
        assert_eq!(e.put(&format!("2026-11-0{day}"), "x\n"), Applied::Done);
    }
    assert!(e.seen("2026-11-09").present);
    assert!(e.seen("2026-11-01").present);
    assert!(e.fake.count("list") > 9);
}

fn day_heading(text: &str) -> Value {
    json!({"object":"block","type":"heading_2","heading_2":{"rich_text":[{"type":"text","text":{"content":text}}]}})
}

#[test]
fn duplicate_and_disordered_hierarchy_is_a_structural_conflict() {
    let e = env();
    e.put("2026-11-07", "x\n");
    e.fake.external_page(ROOT, "2026");
    assert_eq!(
        e.seen("2026-11-07").unavailable,
        Some(Unavailable::Structural)
    );

    // A day listed twice with different text (a bare duplicate heading is an unfinished move).
    let e = env();
    e.put("2026-11-07", "x\n");
    e.add_day("2026-11-07 (SAT)", &["different"]);
    assert_eq!(
        e.seen("2026-11-07").unavailable,
        Some(Unavailable::Structural)
    );
}

#[test]
fn a_legacy_dated_toggle_is_a_structural_conflict() {
    let e = env();
    e.put("2026-11-07", "x\n");
    let toggle = json!({"object":"block","type":"toggle","toggle":{"rich_text":[{"type":"text","text":{"content":"2026-11-03"}}]}});
    e.fake.external_block(&e.month(), &toggle, None);
    assert_eq!(
        e.seen("2026-11-07").unavailable,
        Some(Unavailable::Structural)
    );
    assert_eq!(
        e.notion.apply(
            &d("2026-11-09"),
            &file("y\n"),
            Write::Create,
            &Observation::absent()
        ),
        Applied::Failed
    );
}

#[test]
fn observing_an_absent_date_never_writes() {
    let e = env();
    e.put("2026-11-03", "a\n");
    e.put("2026-11-10", "b\n");
    let before = e.fake.count("append") + e.fake.count("delete") + e.fake.count("create_page");
    assert_eq!(e.seen("2026-11-05"), Observation::absent());
    let after = e.fake.count("append") + e.fake.count("delete") + e.fake.count("create_page");
    assert_eq!(before, after);
}

#[test]
fn replace_inserts_a_new_range_after_the_old_one_then_removes_the_old_one() {
    let e = env();
    e.put("2026-11-07", "one\n\ntwo\n");
    let old = e.heading_id("2026-11-07");
    assert_eq!(e.put("2026-11-07", "three\n"), Applied::Done);
    // Exactly one day remains, with a new heading ID (O10).
    assert_eq!(e.days(), ["2026-11-07 (SAT)"]);
    assert_ne!(e.heading_id("2026-11-07"), old);
    assert!(!e.fake.children(&e.month()).contains(&old));
    assert_eq!(e.range_texts("2026-11-07"), ["three"]);
    assert!(e.digest_ok("2026-11-07", "three\n"));
    // A neighbouring day keeps its position relative to the replaced one.
    e.put("2026-11-09", "later\n");
    e.put("2026-11-07", "four\n");
    assert_eq!(e.days(), ["2026-11-07 (SAT)", "2026-11-09 (MON)"]);
    assert_eq!(e.range_texts("2026-11-07"), ["four"]);
    assert_eq!(e.range_texts("2026-11-09"), ["later"]);
}

#[test]
fn a_crash_between_the_new_range_and_the_old_delete_keeps_the_old_text() {
    // The delete of the old range is lost before it commits.
    let e = env();
    e.put("2026-11-07", "keep me\n");
    e.fake.fault("delete", Fault::Drop);
    assert_eq!(e.put("2026-11-07", "new text\n"), Applied::Unknown);
    assert_eq!(e.range_texts("2026-11-07"), ["keep me"]);
    // Both same-date days now exist: reported, not guessed at.
    assert_eq!(e.days().len(), 2);
    assert_eq!(
        e.seen("2026-11-07").unavailable,
        Some(Unavailable::Structural)
    );
    assert_eq!(e.fake.count("delete"), 1);

    // A refusal while the new range is being filled leaves the old one untouched too.
    let e = env();
    e.put("2026-11-07", "keep me\n");
    e.fake.fault_after("append", 1, Fault::Throttle(90));
    assert_eq!(e.put("2026-11-07", "new text\n"), Applied::Unknown);
    assert_eq!(e.range_texts("2026-11-07"), ["keep me"]);
    assert_eq!(e.fake.count("delete"), 0);
}

#[test]
fn a_foreign_block_in_a_day_is_a_structural_conflict_with_no_write() {
    let e = env();
    e.put("2026-11-07", "one\n");
    e.fake
        .external_page(&e.month(), "Notes the user keeps here");
    assert_eq!(
        e.seen("2026-11-07").unavailable,
        Some(Unavailable::Structural)
    );
    let before = e.fake.count("append") + e.fake.count("delete");
    let expected = Observation::absent();
    assert_eq!(
        e.notion
            .apply(&d("2026-11-07"), &file("two\n"), Write::Replace, &expected),
        Applied::Failed
    );
    assert_eq!(e.fake.count("append") + e.fake.count("delete"), before);
    assert_eq!(e.fake.children(&e.month()).len(), 3);
}

#[test]
fn an_external_edit_between_observe_and_apply_fails_without_writing() {
    let e = env();
    e.put("2026-11-07", "one\n");
    let seen = e.seen("2026-11-07");
    let child = e.fake.children(&e.month())[1].clone();
    e.fake.edit_text(&child, "changed outside");
    let before = e.fake.count("append") + e.fake.count("delete");
    assert_eq!(
        e.notion
            .apply(&d("2026-11-07"), &file("new\n"), Write::Replace, &seen),
        Applied::Failed
    );
    assert_eq!(e.fake.count("append") + e.fake.count("delete"), before);
    assert_ne!(e.seen("2026-11-07").fingerprint, seen.fingerprint);
    assert_eq!(e.fake.text_of(&child), "changed outside");
}

#[test]
fn long_unicode_entries_chunk_and_round_trip() {
    let e = env();
    let long = "한😀".repeat(3_000);
    let many: String = (0..250).map(|i| format!("para {i}\n\n")).collect();
    let body = format!("{long}\n\n{many}");
    assert_eq!(e.put("2026-11-07", &body), Applied::Done);
    assert!(e.digest_ok("2026-11-07", &body));
    // 251 top-level blocks need three requests of at most 100 children.
    assert_eq!(e.fake.count("append") - 1, 3);
}

#[test]
fn a_short_throttle_waits_and_retries_but_a_long_one_stops_cleanly() {
    let e = env();
    e.fake.fault("create_page", Fault::Throttle(2));
    assert_eq!(e.put("2026-11-07", "x\n"), Applied::Done);
    assert!(e.slept.lock().unwrap().iter().any(|d| d.as_secs() == 2));

    let e = env();
    e.fake.fault("create_page", Fault::Throttle(90));
    assert_eq!(e.put("2026-11-07", "x\n"), Applied::Failed);
    assert!(e.fake.children(ROOT).is_empty());
    assert_eq!(e.fake.count("create_page"), 1);

    // After a write has landed, a refusal leaves a partial section: unknown, never failed.
    let e = env();
    e.fake.fault("append", Fault::Throttle(90));
    assert_eq!(e.put("2026-11-07", "x\n"), Applied::Unknown);
}

#[test]
fn a_lost_reply_is_unknown_never_replayed_and_the_read_back_sees_the_commit() {
    let e = env();
    e.fake.fault("append", Fault::LoseReply);
    // The first append is the day heading; its reply is lost.
    assert_eq!(e.put("2026-11-07", "x\n"), Applied::Unknown);
    assert_eq!(e.fake.count("append"), 1);
    assert_eq!(e.days(), ["2026-11-07 (SAT)"]);

    let e = env();
    e.fake.fault("create_page", Fault::Drop);
    assert_eq!(e.put("2026-11-07", "x\n"), Applied::Unknown);
    assert_eq!(e.fake.count("create_page"), 1);
    assert!(e.fake.children(ROOT).is_empty());
}

#[test]
fn server_errors_on_writes_are_not_retried_but_reads_are() {
    let e = env();
    e.fake.fault("create_page", Fault::Status(503));
    assert_eq!(e.put("2026-11-07", "x\n"), Applied::Unknown);
    assert_eq!(e.fake.count("create_page"), 1);

    let e = env();
    e.fake.fault("list", Fault::Status(503));
    assert_eq!(e.seen("2026-11-07"), Observation::absent());
    assert!(e.fake.count("list") >= 2);

    let e = env();
    e.fake.fault("create_page", Fault::Status(403));
    assert_eq!(e.put("2026-11-07", "x\n"), Applied::Failed);
}

#[test]
fn credential_and_network_problems_are_typed() {
    let e = env();
    e.fake.fault("list", Fault::Status(401));
    assert_eq!(
        e.seen("2026-11-07").unavailable,
        Some(Unavailable::Unauthorized)
    );
    e.fake.fault("list", Fault::NotSent);
    assert_eq!(e.seen("2026-11-07").unavailable, Some(Unavailable::Network));
    e.fake.fault("list", Fault::Throttle(120));
    assert_eq!(
        e.seen("2026-11-07").unavailable,
        Some(Unavailable::Throttled)
    );
}

#[test]
fn the_client_reports_the_wait_and_never_exposes_bodies() {
    let fake = Arc::new(FakeNotion::new(ROOT));
    let client = Client::new(fake.clone(), Arc::new(|_| {}), Duration::ZERO);
    fake.fault("list", Fault::Throttle(61));
    assert_eq!(
        client.call(Method::Get, &format!("/v1/blocks/{ROOT}/children"), None),
        Err(Error::Throttled(61))
    );
    let huge = json!({"x": "a".repeat(client::MAX_REQUEST_BYTES)});
    assert_eq!(
        client.call(Method::Patch, "/v1/blocks/x/children", Some(&huge)),
        Err(Error::TooLarge)
    );
}

#[test]
fn workspace_identity_is_fetched_lazily_and_gates_the_destination_key() {
    let e = env();
    assert_eq!(e.fake.count("me"), 0, "construction makes no request");
    assert_eq!(e.notion.root(), format!("{ROOT}@pending"));
    e.fake.fault("me", Fault::Status(401));
    assert_eq!(e.notion.ready(), Err(Unavailable::Unauthorized));
    assert_eq!(e.notion.root(), format!("{ROOT}@pending"));
    // The next attempt succeeds; the root then carries the workspace and is cached.
    assert_eq!(e.notion.ready(), Ok(()));
    assert_eq!(e.notion.root(), format!("{ROOT}@workspace-1"));
    let calls = e.fake.count("me");
    e.notion.ready().unwrap();
    assert_eq!(e.fake.count("me"), calls);

    // While the identity is unknown, observing reports why and reads nothing else.
    let e = env();
    e.fake.fault("me", Fault::NotSent);
    assert_eq!(e.seen("2026-11-07").unavailable, Some(Unavailable::Network));
    assert_eq!(e.fake.count("list"), 0);
}

#[test]
fn a_malformed_root_page_id_registers_nothing() {
    let e = env();
    let client = Client::new(
        e.fake.clone() as Arc<dyn Http>,
        Arc::new(|_| {}),
        Duration::ZERO,
    );
    assert!(e.ws.notion(client, "not-a-page-id").is_none());
}

#[test]
fn projection_warnings_are_reported_without_changing_the_digest_contract() {
    let e = env();
    assert_eq!(e.notion.warnings(&file("| a |\n|---|\n| 1 |\n")), 1);
    assert_eq!(e.notion.warnings(&file("plain\n")), 0);
    let _: Value = json!(null);
}

#[test]
fn concurrent_exports_never_create_duplicate_year_or_month_pages() {
    let e = Arc::new(env());
    let handles: Vec<_> = ["2026-11-02", "2026-11-09", "2026-11-16", "2026-11-23"]
        .into_iter()
        .map(|day| {
            let e = e.clone();
            std::thread::spawn(move || e.put(day, "x\n"))
        })
        .collect();
    for h in handles {
        assert_eq!(h.join().unwrap(), Applied::Done);
    }
    assert_eq!(e.fake.children(ROOT).len(), 1);
    let year = e.fake.children(ROOT)[0].clone();
    assert_eq!(e.fake.children(&year).len(), 1);
    assert_eq!(
        e.days(),
        [
            "2026-11-02 (MON)",
            "2026-11-09 (MON)",
            "2026-11-16 (MON)",
            "2026-11-23 (MON)"
        ]
    );
}

#[test]
fn a_preamble_before_the_first_day_survives_every_write() {
    let e = env();
    e.put("2026-11-07", "seven\n");
    let pre = e.fake.external_block(
        &e.month(),
        &json!({"object":"block","type":"paragraph","paragraph":{"rich_text":[{"type":"text","text":{"content":"preamble"}}]}}),
        Some(0),
    );
    e.put("2026-11-05", "five\n");
    e.put("2026-11-01", "one\n");
    e.put("2026-11-05", "five again\n");
    assert_eq!(e.fake.children(&e.month())[0], pre);
    assert_eq!(
        e.days(),
        ["2026-11-01 (SUN)", "2026-11-05 (THU)", "2026-11-07 (SAT)"]
    );
    assert_eq!(e.range_texts("2026-11-01"), ["one"]);
    assert_eq!(e.range_texts("2026-11-05"), ["five again"]);
}

#[test]
fn an_empty_body_is_a_heading_with_no_siblings() {
    let e = env();
    assert_eq!(e.put("2026-11-07", ""), Applied::Done);
    assert!(e.seen("2026-11-07").present);
    assert!(e.range_texts("2026-11-07").is_empty());
    assert_eq!(e.put("2026-11-07", "text\n"), Applied::Done);
    assert_eq!(e.range_texts("2026-11-07"), ["text"]);
    assert_eq!(e.put("2026-11-07", ""), Applied::Done);
    assert!(e.range_texts("2026-11-07").is_empty());
    assert_eq!(e.days(), ["2026-11-07 (SAT)"]);
}

#[test]
fn body_headings_do_not_split_the_range() {
    let e = env();
    assert_eq!(e.put("2026-11-07", "# A\n\n## B\n\ntext\n"), Applied::Done);
    assert_eq!(e.days(), ["2026-11-07 (SAT)"]);
    assert_eq!(e.range_texts("2026-11-07"), ["A", "B", "text"]);
    assert!(e.digest_ok("2026-11-07", "# A\n\n## B\n\ntext\n"));
}

#[test]
fn the_weekday_suffix_does_not_decide_which_day_a_heading_is() {
    let e = env();
    e.put("2026-11-07", "x\n");
    e.fake
        .external_block(&e.month(), &day_heading("2026-11-09 (FRI)"), None);
    // The hand-typed suffix is wrong (09 is a Monday) but the date still identifies the day.
    assert!(e.seen("2026-11-09").present);
    assert_eq!(e.put("2026-11-09", "y\n"), Applied::Done);
    assert_eq!(e.days(), ["2026-11-07 (SAT)", "2026-11-09 (MON)"]);
    assert_eq!(e.range_texts("2026-11-09"), ["y"]);
}

#[test]
fn a_hand_typed_undated_heading_two_is_absorbed_into_the_previous_day() {
    let e = env();
    e.put("2026-11-05", "five\n");
    let before = e.seen("2026-11-05");
    e.fake
        .external_block(&e.month(), &day_heading("Notes"), None);
    let after = e.seen("2026-11-05");
    assert!(after.present);
    assert_ne!(after.fingerprint, before.fingerprint);
    assert_ne!(after.digest, before.digest);
    assert_eq!(e.days(), ["2026-11-05 (THU)", "Notes"]);
}

fn paragraph(text: &str) -> Value {
    json!({"object":"block","type":"paragraph","paragraph":{"rich_text":[{"type":"text","text":{"content":text}}]}})
}
impl Env {
    /// A day added at the end of the month page by hand, whatever its date.
    fn add_day(&self, title: &str, body: &[&str]) {
        self.fake
            .external_block(&self.month(), &day_heading(title), None);
        for text in body {
            self.fake
                .external_block(&self.month(), &paragraph(text), None);
        }
    }
}
const ASCENDING: [&str; 5] = [
    "2026-11-01 (SUN)",
    "2026-11-03 (TUE)",
    "2026-11-05 (THU)",
    "2026-11-07 (SAT)",
    "2026-11-09 (MON)",
];
fn page_with_03_last(e: &Env) {
    for day in ["01", "05", "07"] {
        assert_eq!(
            e.put(&format!("2026-11-{day}"), &format!("text {day}\n")),
            Applied::Done
        );
    }
    e.add_day("2026-11-03 (TUE)", &["text 03"]);
}

#[test]
fn observing_a_disordered_page_is_read_only_and_not_structural() {
    let e = env();
    page_with_03_last(&e);
    let writes =
        |e: &Env| e.fake.count("append") + e.fake.count("delete") + e.fake.count("create_page");
    let before = writes(&e);
    assert!(e.seen("2026-11-03").present);
    assert_eq!(e.seen("2026-11-09"), Observation::absent());
    assert_eq!(e.seen("2026-11-05").unavailable, None);
    assert_eq!(writes(&e), before);
}

#[test]
fn one_misplaced_day_moves_alone_and_the_requested_day_is_written() {
    let e = env();
    page_with_03_last(&e);
    let (kept_05, kept_07) = (e.heading_id("2026-11-05"), e.heading_id("2026-11-07"));
    let (appends, deletes) = (e.fake.count("append"), e.fake.count("delete"));
    assert_eq!(e.put("2026-11-09", "text 09\n"), Applied::Done);
    assert_eq!(e.days(), ASCENDING);
    for day in ["01", "03", "05", "07", "09"] {
        assert_eq!(
            e.range_texts(&format!("2026-11-{day}")),
            [format!("text {day}")]
        );
    }
    // The days that were in order keep their blocks; only 03 was copied and its original removed.
    assert_eq!(e.heading_id("2026-11-05"), kept_05);
    assert_eq!(e.heading_id("2026-11-07"), kept_07);
    assert_eq!(
        e.fake.count("append") - appends,
        4,
        "copy heading + chunk, then 09's heading + chunk"
    );
    assert_eq!(
        e.fake.count("delete") - deletes,
        2,
        "the original's paragraph and heading"
    );
}

#[test]
fn an_ascending_page_costs_no_extra_requests() {
    let e = env();
    for day in ["01", "05", "07"] {
        e.put(&format!("2026-11-{day}"), "x\n");
    }
    let (appends, deletes) = (e.fake.count("append"), e.fake.count("delete"));
    e.put("2026-11-09", "x\n");
    assert_eq!(e.fake.count("append") - appends, 2);
    assert_eq!(e.fake.count("delete"), deletes);
}

#[test]
fn a_reversed_page_moves_the_minimum() {
    let e = env();
    e.put("2026-11-05", "text 05\n");
    let kept = e.heading_id("2026-11-05");
    for (title, day) in [
        ("2026-11-04 (WED)", "04"),
        ("2026-11-03 (TUE)", "03"),
        ("2026-11-02 (MON)", "02"),
        ("2026-11-01 (SUN)", "01"),
    ] {
        e.add_day(title, &[&format!("text {day}")]);
    }
    let deletes = e.fake.count("delete");
    assert_eq!(e.put("2026-11-09", "text 09\n"), Applied::Done);
    assert_eq!(
        e.days(),
        [
            "2026-11-01 (SUN)",
            "2026-11-02 (MON)",
            "2026-11-03 (TUE)",
            "2026-11-04 (WED)",
            "2026-11-05 (THU)",
            "2026-11-09 (MON)"
        ]
    );
    assert_eq!(e.heading_id("2026-11-05"), kept);
    assert_eq!(
        e.fake.count("delete") - deletes,
        8,
        "four days, body and heading each"
    );
    for day in ["01", "02", "03", "04", "05", "09"] {
        assert_eq!(
            e.range_texts(&format!("2026-11-{day}")),
            [format!("text {day}")]
        );
    }
}

#[test]
fn the_requested_day_may_itself_be_the_moved_one() {
    let e = env();
    page_with_03_last(&e);
    assert_eq!(e.put("2026-11-03", "rewritten\n"), Applied::Done);
    assert_eq!(e.days(), &ASCENDING[..4]);
    assert_eq!(e.range_texts("2026-11-03"), ["rewritten"]);
    assert!(e.digest_ok("2026-11-03", "rewritten\n"));
}

#[test]
fn a_crash_between_copy_and_delete_is_finished_by_the_next_export() {
    let e = env();
    page_with_03_last(&e);
    e.fake.fault("delete", Fault::Drop);
    assert_eq!(e.put("2026-11-09", "text 09\n"), Applied::Unknown);
    // The verified copy and the original both exist; nothing is lost.
    assert_eq!(
        e.days()
            .iter()
            .filter(|d| d.starts_with("2026-11-03"))
            .count(),
        2
    );
    assert_eq!(e.put("2026-11-09", "text 09\n"), Applied::Done);
    assert_eq!(e.days(), ASCENDING);
    assert_eq!(e.range_texts("2026-11-03"), ["text 03"]);
}

#[test]
fn a_crash_in_the_middle_of_the_delete_is_finished_by_the_next_export() {
    let e = env();
    for day in ["01", "05", "07"] {
        e.put(&format!("2026-11-{day}"), "x\n");
    }
    e.add_day("2026-11-03 (TUE)", &["first", "second"]);
    e.fake.fault_after("delete", 1, Fault::Drop);
    assert_eq!(e.put("2026-11-09", "x\n"), Applied::Unknown);
    assert_eq!(e.put("2026-11-09", "x\n"), Applied::Done);
    assert_eq!(e.days(), ASCENDING);
    assert_eq!(e.range_texts("2026-11-03"), ["first", "second"]);
}

#[test]
fn a_lost_reply_on_the_copy_heading_leaves_a_repairable_empty_copy() {
    let e = env();
    page_with_03_last(&e);
    e.fake.fault("append", Fault::LoseReply);
    assert_eq!(e.put("2026-11-09", "text 09\n"), Applied::Unknown);
    assert_eq!(e.put("2026-11-09", "text 09\n"), Applied::Done);
    assert_eq!(e.days(), ASCENDING);
    assert_eq!(e.range_texts("2026-11-03"), ["text 03"]);
}

fn assert_refused_without_writing(e: &Env) {
    let writes =
        |e: &Env| e.fake.count("append") + e.fake.count("delete") + e.fake.count("create_page");
    let before = writes(e);
    assert_eq!(e.put("2026-11-09", "text 09\n"), Applied::Failed);
    assert_eq!(writes(e), before);
}

#[test]
fn a_formatted_day_is_not_moved() {
    let e = env();
    for day in ["01", "05", "07"] {
        e.put(&format!("2026-11-{day}"), "x\n");
    }
    e.fake
        .external_block(&e.month(), &day_heading("2026-11-03 (TUE)"), None);
    e.fake.external_block(
        &e.month(),
        &json!({"object":"block","type":"paragraph","paragraph":{"rich_text":[{"type":"text","text":{"content":"bold"},"annotations":{"bold":true}}]}}),
        None,
    );
    assert_refused_without_writing(&e);
}

#[test]
fn a_foreign_block_in_a_moved_day_refuses() {
    let e = env();
    page_with_03_last(&e);
    e.fake
        .external_page(&e.month(), "Notes the user keeps here");
    assert_refused_without_writing(&e);
}

#[test]
fn two_different_texts_for_one_day_refuse() {
    let e = env();
    page_with_03_last(&e);
    e.add_day("2026-11-03 (TUE)", &["a different text"]);
    assert_refused_without_writing(&e);
}

#[test]
fn a_moved_day_with_nested_items_keeps_an_equal_digest() {
    let e = env();
    for day in ["01", "05", "07"] {
        e.put(&format!("2026-11-{day}"), "x\n");
    }
    e.fake
        .external_block(&e.month(), &day_heading("2026-11-03 (TUE)"), None);
    e.fake.external_block(
        &e.month(),
        &json!({"object":"block","type":"bulleted_list_item","bulleted_list_item":{
            "rich_text":[{"type":"text","text":{"content":"a"}}],
            "children":[{"object":"block","type":"bulleted_list_item","bulleted_list_item":{"rich_text":[{"type":"text","text":{"content":"b"}}]}}]}}),
        None,
    );
    assert_eq!(e.put("2026-11-09", "x\n"), Applied::Done);
    assert_eq!(e.days()[1], "2026-11-03 (TUE)");
    assert!(e.digest_ok("2026-11-03", "- a\n  - b\n"));
}

#[test]
fn a_long_month_in_small_api_pages_is_reordered() {
    let e = env_with(2);
    page_with_03_last(&e);
    assert_eq!(e.put("2026-11-09", "text 09\n"), Applied::Done);
    assert_eq!(e.days(), ASCENDING);
}

#[test]
fn a_preamble_never_moves_during_a_reorder() {
    let e = env();
    page_with_03_last(&e);
    let pre = e
        .fake
        .external_block(&e.month(), &paragraph("preamble"), Some(0));
    assert_eq!(e.put("2026-11-09", "text 09\n"), Applied::Done);
    assert_eq!(e.fake.children(&e.month())[0], pre);
    assert_eq!(e.days(), ASCENDING);
}

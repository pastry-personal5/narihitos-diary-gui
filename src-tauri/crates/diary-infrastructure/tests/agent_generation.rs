//! Shared generation fixtures: `ok-*.json` must validate, `bad-*.json` must fail `invalid_output`.
//! Adapter conformance tests (P1-M17 onward) reuse these files.
use diary_application::agent::AgentErrorCode;
use diary_infrastructure::agent::output::validate_generation;
use std::fs;

#[test]
fn generation_fixtures_match_their_names() {
    let dir = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/agent/generation"
    );
    let (mut ok, mut bad) = (0, 0);
    for e in fs::read_dir(dir).unwrap() {
        let e = e.unwrap();
        let name = e.file_name().into_string().unwrap();
        if name.starts_with('.') {
            continue; // Finder's .DS_Store is not a fixture
        }
        let bytes = fs::read(e.path()).unwrap();
        let r = validate_generation(&bytes, 2);
        if name.starts_with("ok-") && name.ends_with(".json") {
            r.unwrap_or_else(|_| panic!("{name} must validate"));
            ok += 1;
        } else if name.starts_with("bad-") && name.ends_with(".json") {
            assert_eq!(
                r.expect_err(&name).0,
                AgentErrorCode::InvalidOutput,
                "{name}"
            );
            bad += 1;
        } else {
            panic!("fixture {name} must be named ok-*.json or bad-*.json");
        }
    }
    assert!(ok >= 2 && bad >= 15, "ok={ok} bad={bad}");
}

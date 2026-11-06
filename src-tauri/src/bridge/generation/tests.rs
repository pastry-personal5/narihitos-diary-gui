use super::*;
use crate::bridge::authentic::tests::{fixture, http};
use serde_json::{Value, json};
fn view() -> Value {
    json!({"job":{"id":"j","sequence":3,"state":"running"},"date":"2026-11-06","profile":"main","locale":"ko-KR","count":2,"stale_baseline":false,
    "candidates":[{"index":1,"id":"j-1","status":"ready","error_code":null},{"index":2,"id":null,"status":"running","error_code":null}],
    "progress":{"kind":"generation","job_id":"j","request_id":"r","instance_id":"i","session_id":"s","origin_date":"2026-11-06","profile_id":"main","locale":"ko-KR","baseline_revision":null,"candidate_index":2,"requested_count":2,"finished_count":1,"ready_count":1,"failed_count":0,"cancelled_count":0,"not_started_count":0,"phase":"receiving_output","elapsed_ms":10,"last_agent_activity_at":10,"cancellation_requested":false,"candidate_ids":["j-1"],"error_code":null},"provisional":"hello"})
}
fn candidate() -> Value {
    json!({"id":"j-1","job_id":"j","index":1,"date":"2026-11-06","draft_markdown":"hello","claim_support":[{"claim":"a","snippet_ids":["snippet-1"]}],"warnings":[]})
}
fn input() -> GenerationInput {
    GenerationInput {
        request_id: "r".into(),
        date: "2026-11-06".into(),
        profile: "main".into(),
        locale: "ko-KR".into(),
        count: 2,
        selections: vec![Selection {
            import_id: "import".into(),
            source_id: "source".into(),
        }],
    }
}
#[tokio::test]
async fn generation_commands_use_fixed_routes_and_typed_payloads() {
    let (_t, b, task) = fixture(vec![
        http(
            "202 Accepted",
            json!({"job":{"id":"j","sequence":2,"state":"running"}}),
        ),
        http("200 OK", view()),
        http("200 OK", candidate()),
        http("200 OK", json!({})),
    ]);
    b.connect().await.unwrap();
    b.start_generation(input()).await.unwrap();
    assert_eq!(b.get_generation("j".into()).await.unwrap().count, 2);
    assert_eq!(
        b.read_candidate("j-1".into()).await.unwrap().draft_markdown,
        "hello"
    );
    b.discard_generation("j".into()).await.unwrap();
    let r = task.join().unwrap();
    assert!(r[1].starts_with("POST /api/v1/generations HTTP/1.1"));
    assert!(r[1].contains("\"import_id\":\"import\""));
    assert!(r[2].starts_with("GET /api/v1/jobs/j/result HTTP/1.1"));
    assert!(r[3].starts_with("GET /api/v1/candidates/j-1 HTTP/1.1"));
    assert!(r[4].starts_with("DELETE /api/v1/generations/j HTTP/1.1"));
}
#[tokio::test]
async fn malformed_input_does_not_reach_network() {
    let (_t, b, _task) = fixture(vec![]);
    for mutate in [0, 1, 2, 3] {
        let mut i = input();
        match mutate {
            0 => i.count = 0,
            1 => i.date = "bad".into(),
            2 => i.selections[0].source_id = "../secret".into(),
            _ => i.locale = "raw text".into(),
        };
        assert_eq!(
            b.start_generation(i).await.err().unwrap().code,
            "invalid_input"
        );
    }
    assert_eq!(
        b.get_generation("../x".into()).await.err().unwrap().code,
        "invalid_input"
    );
    assert_eq!(
        b.read_candidate("../x".into()).await.err().unwrap().code,
        "invalid_input"
    );
    assert_eq!(
        b.discard_generation("../x".into())
            .await
            .err()
            .unwrap()
            .code,
        "invalid_input"
    );
}
#[tokio::test]
async fn invalid_progress_and_candidate_content_are_rejected() {
    let mut cases = Vec::new();
    for (key, v) in [
        ("phase", json!("PRIVATE_TEXT")),
        ("ready_count", json!(255)),
        ("candidate_ids", json!(["wrong"])),
        ("origin_date", json!("2026-11-07")),
        ("error_code", json!("PRIVATE_ERROR")),
    ] {
        let mut bad = view();
        bad["progress"][key] = v;
        cases.push(http("200 OK", bad));
    }
    let mut bad = candidate();
    bad["draft_markdown"] = json!("x".repeat(MAX_TEXT_BYTES + 1));
    cases.push(http("200 OK", bad));
    let (_t, b, _task) = fixture(cases);
    b.connect().await.unwrap();
    for _ in 0..5 {
        assert_eq!(
            b.get_generation("j".into()).await.err().unwrap().code,
            "unavailable"
        );
    }
    assert_eq!(
        b.read_candidate("j-1".into()).await.err().unwrap().code,
        "unavailable"
    );
}
#[tokio::test]
async fn discard_required_is_a_closed_visible_error() {
    let (_t, b, _task) = fixture(vec![http(
        "409 Conflict",
        json!({"error":{"code":"discard_required","message":"PRIVATE_ERROR","retryable":false}}),
    )]);
    b.connect().await.unwrap();
    assert_eq!(
        b.start_generation(input()).await.err().unwrap().code,
        "discard_required"
    );
}

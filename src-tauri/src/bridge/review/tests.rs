use super::*;
use crate::bridge::authentic::tests::{fixture, http};
use serde_json::{Value, json};

const R: &str = "11111111-1111-4111-8111-111111111111";
const O: &str = "22222222-2222-4222-8222-222222222222";

fn view() -> Value {
    json!({"review_id":R,"date":"2026-11-07","candidate_id":"j-1","revision":null,"set_digest":"a".repeat(64),
        "body":"한글 B\n","hunks":[{"id":"h1","kind":"replace","label":null,"from":3,"to":5,"text":"B2\n"},
        {"id":"h2","kind":"insert","label":"heading","from":5,"to":5,"text":"# X\n"}]})
}
fn start_input() -> StartReviewInput {
    StartReviewInput {
        review_id: R.into(),
        date: "2026-11-07".into(),
        candidate_id: "j-1".into(),
    }
}
fn decide_input() -> DecideReviewInput {
    DecideReviewInput {
        review_id: R.into(),
        operation_id: O.into(),
        hunk_id: "h1".into(),
        action: "accept".into(),
        expected_revision: None,
        expected_set_digest: "a".repeat(64),
    }
}

#[tokio::test]
async fn review_commands_use_fixed_routes() {
    let (_t, b, task) = fixture(vec![
        http("201 Created", view()),
        http("200 OK", json!({"receipt":null,"view":view()})),
        http("200 OK", json!({})),
    ]);
    b.connect().await.unwrap();
    assert_eq!(b.start_review(start_input()).await.unwrap().hunks.len(), 2);
    assert!(
        b.decide_review(decide_input())
            .await
            .unwrap()
            .view
            .is_some()
    );
    b.end_review(R.into()).await.unwrap();
    let r = task.join().unwrap();
    assert!(r[1].starts_with("POST /api/v1/reviews HTTP/1.1"));
    assert!(r[2].starts_with(&format!("POST /api/v1/reviews/{R}/decisions HTTP/1.1")));
    assert!(r[2].contains("\"action\":\"accept\""));
    assert!(r[3].starts_with(&format!("DELETE /api/v1/reviews/{R} HTTP/1.1")));
}

#[tokio::test]
async fn malformed_input_never_reaches_the_network() {
    let (_t, b, _task) = fixture(vec![]);
    let mut s = start_input();
    s.candidate_id = "../x".into();
    assert_eq!(b.start_review(s).await.err().unwrap().code, "invalid_input");
    for n in 0..4 {
        let mut d = decide_input();
        match n {
            0 => d.action = "merge".into(),
            1 => d.operation_id = "nope".into(),
            2 => d.expected_set_digest = "short".into(),
            _ => d.hunk_id = "a/b".into(),
        }
        assert_eq!(
            b.decide_review(d).await.err().unwrap().code,
            "invalid_input"
        );
    }
    assert_eq!(
        b.end_review("../x".into()).await.err().unwrap().code,
        "invalid_input"
    );
}

#[tokio::test]
async fn unsound_views_are_rejected() {
    for n in 0..3 {
        let mut v = view();
        match n {
            0 => v["hunks"][0]["to"] = json!(99),
            1 => v["hunks"][1]["from"] = json!(1),
            _ => v["hunks"][0]["kind"] = json!("merge"),
        }
        let (_t, b, _task) = fixture(vec![http("201 Created", v)]);
        b.connect().await.unwrap();
        assert_eq!(
            b.start_review(start_input()).await.err().unwrap().code,
            "unavailable"
        );
    }
}

#[tokio::test]
async fn review_error_codes_pass_through() {
    let (_t, b, _task) = fixture(vec![http(
        "409 Conflict",
        json!({"error":{"code":"review_invalidated","message":"x","retryable":false}}),
    )]);
    b.connect().await.unwrap();
    assert_eq!(
        b.decide_review(decide_input()).await.err().unwrap().code,
        "review_invalidated"
    );
}

use crate::{
    ServerState, router,
    tests::{request, state},
};
use axum::body::{Body, to_bytes};
use axum::http::StatusCode;
use diary_application::agent::*;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;
use tower::ServiceExt;

const SESSION: &str = "07249746-e280-4d51-a86a-cbcb198ee101";

struct Profiles;
impl ProfileDirectory for Profiles {
    fn list(&self) -> Vec<ProfileView> {
        vec![]
    }
    fn check(&self, alias: &str) -> Option<ProfileView> {
        let ready = alias != "off";
        (alias != "none").then(|| ProfileView {
            alias: alias.into(),
            kind: AgentKind::Codex,
            status: Some(if ready {
                ProfileStatus::Ready
            } else {
                ProfileStatus::Unavailable
            }),
            reason: None,
            chat: Capability::Ready,
            generate: Capability::Ready,
        })
    }
}

#[derive(Clone, Copy)]
enum Mode_ {
    Reply,
    Block,
    Fail(AgentErrorCode),
}
struct Runner(Mode_);
impl AgentRunner for Runner {
    fn check(&self, _: &ProfileHandle) -> ProfileCheck {
        unreachable!("not used")
    }
    fn run(
        &self,
        request: AgentRequest,
        events: &mut dyn EventSink,
        cancel: &CancelToken,
    ) -> Result<AgentOutput, AgentError> {
        events.event(AgentEvent::Phase(Phase::WaitingForOutput));
        events.event(AgentEvent::Delta("par".into()));
        match self.0 {
            Mode_::Reply => {
                let Mode::Chat(c) = request.mode() else {
                    panic!("chat only")
                };
                // The packet carries the held transcript and the document flag.
                let unsaved = c.document().is_some_and(|d| d.unsaved);
                Ok(AgentOutput::Reply(format!(
                    "reply {} {unsaved}",
                    c.transcript().len()
                )))
            }
            Mode_::Block => {
                while !cancel.is_cancelled() {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Ok(AgentOutput::Reply("late".into()))
            }
            Mode_::Fail(code) => Err(code.into()),
        }
    }
}

fn setup(mode: Mode_) -> Arc<ServerState> {
    let mut s = state();
    ServerState::set_profiles(&mut s, Arc::new(Profiles));
    ServerState::set_agent_runner(&mut s, Arc::new(Runner(mode)));
    s.coordinator
        .lock()
        .unwrap()
        .open(SESSION, s.now())
        .unwrap();
    s
}
async fn call(
    s: &Arc<ServerState>,
    method: &str,
    path: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut r = request(method, path);
    r.headers_mut()
        .insert("x-diary-session", SESSION.parse().unwrap());
    if let Some(b) = body {
        *r.body_mut() = Body::from(b.to_string());
        r.headers_mut()
            .insert("content-type", "application/json".parse().unwrap());
    }
    let response = router(s.clone()).oneshot(r).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 4 << 20).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}
async fn open(s: &Arc<ServerState>) -> String {
    let (st, v) = call(
        s,
        "POST",
        "/api/v1/chats",
        Some(json!({"profile":"main","locale":"en-US"})),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED);
    v["chat_id"].as_str().unwrap().to_owned()
}
async fn send(s: &Arc<ServerState>, chat: &str, rid: &str, msg: &str) -> (StatusCode, Value) {
    call(
        s,
        "POST",
        &format!("/api/v1/chats/{chat}/turns"),
        Some(json!({"request_id":rid,"message":msg,
            "document":{"kind":"authentic","date":"2026-11-06","unsaved":true,"body":"dear diary"}})),
    )
    .await
}
async fn until(s: &Arc<ServerState>, chat: &str, state: &str) -> Value {
    for _ in 0..400 {
        let (_, v) = call(s, "GET", &format!("/api/v1/chats/{chat}"), None).await;
        if v["turn"]["state"] == state {
            return v;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("turn never reached {state}");
}

#[tokio::test]
async fn open_refuses_unknown_unready_and_malformed() {
    let s = setup(Mode_::Reply);
    for (body, status, code) in [
        (
            json!({"profile":"none","locale":"en-US"}),
            StatusCode::NOT_FOUND,
            "not_found",
        ),
        (
            json!({"profile":"off","locale":"en-US"}),
            StatusCode::CONFLICT,
            "profile_not_ready",
        ),
        (
            json!({"profile":"main","locale":"not a locale"}),
            StatusCode::BAD_REQUEST,
            "invalid_input",
        ),
    ] {
        let (st, v) = call(&s, "POST", "/api/v1/chats", Some(body)).await;
        assert_eq!(
            (st, v["error"]["code"].as_str().or(v["code"].as_str())),
            (status, Some(code)),
            "{v}"
        );
    }
}

#[tokio::test]
async fn turn_replies_and_transcript_survives_reload_within_lease() {
    let s = setup(Mode_::Reply);
    let chat = open(&s).await;
    let (st, v) = send(&s, &chat, "r1", "hello").await;
    assert_eq!(st, StatusCode::ACCEPTED);
    let job = v["job"]["id"].as_str().unwrap().to_owned();
    let v = until(&s, &chat, "done").await;
    // Reload: a fresh GET returns the same transcript, with the unsaved flag reaching the agent.
    assert_eq!(v["transcript"][0], json!({"role":"user","text":"hello"}));
    assert_eq!(
        v["transcript"][1],
        json!({"role":"assistant","text":"reply 0 true"})
    );
    assert!(v["turn"]["provisional"].is_null());
    let (_, j) = call(&s, "GET", &format!("/api/v1/jobs/{job}"), None).await;
    assert_eq!(j["state"], "succeeded");
    // The next turn carries the first exchange.
    send(&s, &chat, "r2", "again").await;
    let mut v = until(&s, &chat, "done").await;
    while v["transcript"].as_array().unwrap().len() < 4 {
        tokio::time::sleep(Duration::from_millis(10)).await;
        v = until(&s, &chat, "done").await;
    }
    assert_eq!(v["transcript"][3]["text"], "reply 2 true");
}

#[tokio::test]
async fn retry_returns_same_job_and_new_request_is_busy_while_running() {
    let s = setup(Mode_::Block);
    let chat = open(&s).await;
    let (_, a) = send(&s, &chat, "r1", "hello").await;
    let (st, b) = send(&s, &chat, "r1", "hello").await;
    assert_eq!(
        (st, &b["job"]["id"]),
        (StatusCode::ACCEPTED, &a["job"]["id"])
    );
    let (st, v) = send(&s, &chat, "r1", "changed").await;
    assert_eq!(st, StatusCode::CONFLICT, "{v}");
    let (st, _) = send(&s, &chat, "r2", "second").await;
    assert_eq!(st, StatusCode::CONFLICT);
    let job = a["job"]["id"].as_str().unwrap();
    call(&s, "POST", &format!("/api/v1/jobs/{job}/cancel"), None).await;
    until(&s, &chat, "cancelled").await;
}

#[tokio::test]
async fn cancel_wins_over_late_success_and_leaves_transcript_empty() {
    let s = setup(Mode_::Block);
    let chat = open(&s).await;
    let (_, a) = send(&s, &chat, "r1", "hello").await;
    let job = a["job"]["id"].as_str().unwrap();
    let (st, c) = call(&s, "POST", &format!("/api/v1/jobs/{job}/cancel"), None).await;
    assert_eq!(
        (st, c["state"].as_str()),
        (StatusCode::OK, Some("cancelled"))
    );
    let v = until(&s, &chat, "cancelled").await;
    assert!(v["transcript"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn failure_reports_a_code_and_no_text() {
    let s = setup(Mode_::Fail(AgentErrorCode::Refusal));
    let chat = open(&s).await;
    send(&s, &chat, "r1", "secret message").await;
    let v = until(&s, &chat, "failed").await;
    assert_eq!(v["turn"]["error_code"], "refusal");
    assert!(v["transcript"].as_array().unwrap().is_empty());
    assert!(!v.to_string().contains("secret message"));
    let (_, jobs) = call(&s, "GET", "/api/v1/jobs", None).await;
    assert_eq!(jobs["jobs"][0]["state"], "failed");
}

#[tokio::test]
async fn oversize_message_is_context_and_nothing_is_admitted() {
    let s = setup(Mode_::Reply);
    let chat = open(&s).await;
    let (st, v) = send(&s, &chat, "r1", &"a".repeat(MAX_USER_MESSAGE_BYTES + 1)).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{v}");
    let (_, jobs) = call(&s, "GET", "/api/v1/jobs", None).await;
    assert!(jobs["jobs"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn transcript_erased_on_close_and_lease_end_cancels_the_run() {
    let s = setup(Mode_::Reply);
    let chat = open(&s).await;
    send(&s, &chat, "r1", "hi").await;
    until(&s, &chat, "done").await;
    let (st, _) = call(&s, "DELETE", &format!("/api/v1/chats/{chat}"), None).await;
    assert_eq!(st, StatusCode::OK);
    let (st, _) = call(&s, "GET", &format!("/api/v1/chats/{chat}"), None).await;
    assert_eq!(st, StatusCode::NOT_FOUND);

    let s = setup(Mode_::Block);
    let chat = open(&s).await;
    let (_, a) = send(&s, &chat, "r1", "hi").await;
    s.lease_ended();
    // The runner only returns once its token is cancelled; the job must reach a terminal state.
    let job = a["job"]["id"].as_str().unwrap().to_owned();
    for _ in 0..400 {
        let state = s
            .coordinator
            .lock()
            .unwrap()
            .get(SESSION, s.now(), &job)
            .unwrap()
            .state;
        if state.is_terminal() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("lease end did not cancel the run");
}

#[tokio::test]
async fn without_a_runner_chat_is_unavailable() {
    let mut s = state();
    ServerState::set_profiles(&mut s, Arc::new(Profiles));
    s.coordinator
        .lock()
        .unwrap()
        .open(SESSION, s.now())
        .unwrap();
    let (st, _) = call(
        &s,
        "POST",
        "/api/v1/chats",
        Some(json!({"profile":"main","locale":"en-US"})),
    )
    .await;
    assert_eq!(st, StatusCode::SERVICE_UNAVAILABLE);
}

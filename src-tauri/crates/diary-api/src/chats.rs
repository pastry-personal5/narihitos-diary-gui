//! Memory-only AI chat. The server holds one chat per client lease: fixed profile and locale, the
//! transcript and at most one running turn. Events carry only job state; clients query the chat
//! for the stage and provisional text, so a lost event never loses text. Responses and errors
//! carry codes, never model text beyond the validated reply itself.
use super::{JobDto, ServerState, app_error, failure, imports};
use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use diary_application::JobState;
use diary_application::agent::{AgentErrorCode, ChatDocument, DocumentKind, Locale, Phase, Role};
use diary_application::chat::{Outcome, workflow};
use diary_domain::authentic::DiaryDate;
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;

/// Request bodies carry up to a 1 MiB document plus the message.
pub(super) const BODY_LIMIT: usize = 1_300_000;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Open {
    profile: String,
    locale: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Doc {
    kind: String,
    date: String,
    unsaved: bool,
    body: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Turn {
    request_id: String,
    message: String,
    document: Option<Doc>,
}

pub(super) fn phase(p: Option<Phase>) -> Value {
    json!(p.map(|p| match p {
        Phase::PreparingContext => "preparing_context",
        Phase::StartingAgent => "starting_agent",
        Phase::WaitingForOutput => "waiting_for_output",
        Phase::ReceivingOutput => "receiving_output",
        Phase::ValidatingOutput => "validating_output",
        Phase::BetweenCandidates => "between_candidates",
        Phase::Stopping => "stopping",
        Phase::Finished => "finished",
    }))
}

fn runner(
    s: &ServerState,
) -> Result<Arc<dyn diary_application::agent::AgentRunner + Send + Sync>, Response> {
    s.agent_runner
        .clone()
        .ok_or_else(|| failure(StatusCode::SERVICE_UNAVAILABLE, "unavailable"))
}
fn json_body<T>(b: Result<Json<T>, JsonRejection>) -> Result<T, Response> {
    imports::body(b)
}

pub(super) async fn open(
    State(s): State<Arc<ServerState>>,
    headers: HeaderMap,
    b: Result<Json<Open>, JsonRejection>,
) -> Response {
    let session = match imports::authorize(&s, &headers) {
        Ok(id) => id,
        Err(r) => return r,
    };
    let b = match json_body(b) {
        Ok(b) => b,
        Err(r) => return r,
    };
    let (Ok(locale), true) = (Locale::parse(&b.locale), imports::opaque(&b.profile)) else {
        return failure(StatusCode::BAD_REQUEST, "invalid_input");
    };
    if runner(&s).is_err() {
        return failure(StatusCode::SERVICE_UNAVAILABLE, "unavailable");
    }
    let Some(profiles) = s.profiles.clone() else {
        return failure(StatusCode::SERVICE_UNAVAILABLE, "unavailable");
    };
    let chat = tokio::task::spawn_blocking(move || {
        workflow::open(
            &*profiles,
            &b.profile,
            locale,
            Uuid::new_v4().to_string(),
            session,
        )
    })
    .await;
    let chat = match chat {
        Ok(Ok(chat)) => chat,
        Ok(Err(e)) => return workflow_error(&s, e),
        Err(_) => return failure(StatusCode::NOT_FOUND, "not_found"),
    };
    let response =
        json!({"chat_id":chat.id,"profile":chat.profile.alias,"locale":chat.locale.as_str()});
    workflow::replace(&mut s.chat.lock().expect("chat poisoned"), chat);
    (StatusCode::CREATED, Json(response)).into_response()
}

fn preparation_failure(
    s: &ServerState,
    kind: diary_application::agent::AgentKind,
    error: diary_application::agent::AgentError,
) -> Response {
    use diary_application::diagnostics::{
        ActionTag, AgentDetail, ErrorTag, EventCode, Level, Record,
    };
    let mut detail = AgentDetail::new(kind);
    detail.action = Some(ActionTag::Chat);
    detail.code = Some(error.0);
    s.log(
        Record::new(EventCode::AgentBlocked, Level::Warn)
            .agent(detail)
            .error(if error.0 == AgentErrorCode::Context {
                ErrorTag::Context
            } else {
                ErrorTag::Unavailable
            }),
    );
    failure(StatusCode::UNPROCESSABLE_ENTITY, error.0.as_str())
}

fn workflow_error(s: &ServerState, error: workflow::Error) -> Response {
    match error {
        workflow::Error::NotFound => failure(StatusCode::NOT_FOUND, "not_found"),
        workflow::Error::ProfileNotReady => failure(StatusCode::CONFLICT, "profile_not_ready"),
        workflow::Error::Packet(e) => failure(StatusCode::UNPROCESSABLE_ENTITY, e.0.as_str()),
        workflow::Error::Preparation { kind, error } => preparation_failure(s, kind, error),
        workflow::Error::Coordination(e) => app_error(e),
    }
}

pub(super) async fn turn(
    State(s): State<Arc<ServerState>>,
    Path(chat_id): Path<String>,
    headers: HeaderMap,
    b: Result<Json<Turn>, JsonRejection>,
) -> Response {
    let session = match imports::authorize(&s, &headers) {
        Ok(id) => id,
        Err(r) => return r,
    };
    let b = match json_body(b) {
        Ok(b) => b,
        Err(r) => return r,
    };
    if !imports::opaque(&b.request_id) || b.message.trim().is_empty() {
        return failure(StatusCode::BAD_REQUEST, "invalid_input");
    }
    let document = match b.document.map(|d| {
        let kind = match d.kind.as_str() {
            "authentic" => DocumentKind::Authentic,
            "source" => DocumentKind::Source,
            _ => return None,
        };
        Some(ChatDocument {
            kind,
            date: DiaryDate::parse(&d.date)?,
            unsaved: d.unsaved,
            body: d.body,
        })
    }) {
        Some(None) => return failure(StatusCode::BAD_REQUEST, "invalid_input"),
        Some(Some(d)) => Some(d),
        None => None,
    };
    let runner = match runner(&s) {
        Ok(r) => r,
        Err(r) => return r,
    };
    let input = workflow::TurnRequest {
        chat_id,
        request_id: b.request_id,
        message: b.message,
        document,
    };
    let request = match input.request(&s.chat.lock().expect("chat poisoned"), &session) {
        Ok(request) => request,
        Err(e) => return workflow_error(&s, e),
    };
    let prepared = match input.prepare(request, &*runner) {
        Ok(prepared) => prepared,
        Err(e) => return workflow_error(&s, e),
    };
    let job_id = Uuid::new_v4().to_string();
    let mut c = s.coordinator.lock().expect("coordinator poisoned");
    let job = match prepared.admit(&mut c, &session, s.now(), &job_id) {
        Ok(j) => j,
        Err(e) => return workflow_error(&s, e),
    };
    let response = json!({"job":JobDto::from(job.clone())});
    if job.state != JobState::Queued {
        return (StatusCode::ACCEPTED, Json(response)).into_response();
    }
    let started = prepared.start(
        &mut c,
        &mut s.chat.lock().expect("chat poisoned"),
        &session,
        s.now(),
        &job,
    );
    let workflow::StartedTurn {
        running,
        prepared,
        run,
        shared,
    } = match started {
        workflow::StartOutcome::Running(started) => *started,
        workflow::StartOutcome::Closed(cancelled) => {
            if let Some(job) = cancelled {
                imports::publish(&s, &mut c, &session, &job);
            }
            return failure(StatusCode::NOT_FOUND, "not_found");
        }
    };
    imports::publish(&s, &mut c, &session, &running);
    drop(c);
    tokio::task::spawn_blocking(move || {
        let notify_state = s.clone();
        let (notify_session, notify_job) = (session.clone(), job.id.clone());
        let mut notify = move || {
            let mut c = notify_state
                .coordinator
                .lock()
                .expect("coordinator poisoned");
            if let Ok(j) = c.touch(&notify_session, notify_state.now(), &notify_job) {
                imports::publish(&notify_state, &mut c, &notify_session, &j);
            }
        };
        let clock_state = s.clone();
        let result = diary_application::run::execute(
            &*runner,
            prepared,
            &run,
            &shared,
            &move || clock_state.now(),
            &mut notify,
        );
        let terminal = workflow::finish(
            &mut s.chat.lock().expect("chat poisoned"),
            &job.id,
            result,
            s.now(),
        );
        let mut c = s.coordinator.lock().expect("coordinator poisoned");
        if let Ok(j) = c.transition(&session, s.now(), &job.id, terminal) {
            imports::publish(&s, &mut c, &session, &j);
        }
    });
    (StatusCode::ACCEPTED, Json(response)).into_response()
}

pub(super) async fn get(
    State(s): State<Arc<ServerState>>,
    Path(chat_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let session = match imports::authorize(&s, &headers) {
        Ok(id) => id,
        Err(r) => return r,
    };
    let guard = s.chat.lock().expect("chat poisoned");
    let Some(chat) = guard
        .as_ref()
        .filter(|c| c.id == chat_id && c.session == session)
    else {
        return failure(StatusCode::NOT_FOUND, "not_found");
    };
    let turn = chat.turn().map(|t| {
        let snap = t.shared.lock().expect("snapshot poisoned");
        let (state, code) = match t.outcome {
            Outcome::Running => ("running", None),
            Outcome::Done => ("done", None),
            Outcome::Failed(AgentErrorCode::Cancelled) => ("cancelled", None),
            Outcome::Failed(c) => ("failed", Some(c.as_str())),
        };
        json!({"job_id":t.job_id,"state":state,"error_code":code,"stage":phase(snap.phase),
            "revision":snap.revision,
            "provisional":(t.outcome == Outcome::Running).then(|| snap.text.clone())})
    });
    Json(json!({
        "chat_id":chat.id,"profile":chat.profile.alias,"type":chat.profile.kind.asset_dir(),
        "locale":chat.locale.as_str(),
        "transcript":chat.transcript().iter().map(|t| json!({"role":match t.role {
            Role::User => "user", Role::Assistant => "assistant"}, "text":t.text})).collect::<Vec<_>>(),
        "turn":turn,
    }))
    .into_response()
}

pub(super) async fn close(
    State(s): State<Arc<ServerState>>,
    Path(chat_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let session = match imports::authorize(&s, &headers) {
        Ok(id) => id,
        Err(r) => return r,
    };
    workflow::close(
        &mut s.chat.lock().expect("chat poisoned"),
        &session,
        &chat_id,
    );
    Json(json!({})).into_response()
}

#[cfg(test)]
mod tests;

/// Discovery never includes transcript, document, prompt or provisional output.
pub(super) fn progress(
    s: &ServerState,
    session: &str,
    job: &diary_application::Job,
) -> Option<Value> {
    let guard = s.chat.lock().expect("chat poisoned");
    let chat = guard.as_ref().filter(|c| c.session == session)?;
    let turn = chat.turn().filter(|t| t.job_id == job.id)?;
    let request_id = turn.request_id.as_ref()?;
    let snap = turn.shared.lock().expect("snapshot poisoned");
    let code = match turn.outcome {
        Outcome::Failed(code) => Some(code.as_str()),
        _ => None,
    };
    Some(json!({
        "kind":"chat", "job_id":job.id, "request_id":request_id,
        "instance_id":s.instance_id, "session_id":session, "chat_id":chat.id,
        "origin_date":turn.origin_date, "profile_id":chat.profile.alias,
        "locale":chat.locale.as_str(), "phase":phase(snap.phase),
        "elapsed_ms":turn.finished_at.unwrap_or_else(||s.now()).saturating_sub(turn.started_at),
        "last_agent_activity_at":snap.last_activity_at,
        "silence_ms":turn.finished_at.unwrap_or_else(||s.now()).saturating_sub(snap.last_activity_at.unwrap_or(turn.started_at)),
        "cancellation_requested":turn.run.token().is_cancelled(), "error_code":code
    }))
}

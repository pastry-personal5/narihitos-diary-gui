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
use diary_application::agent::{
    AgentErrorCode, AgentOutput, AgentRequest, Capability, ChatDocument, DocumentKind, Locale,
    Mode, Phase, ProfileHandle, ProfileStatus, PromptSelection, Role, Run,
};
use diary_application::chat::{Chat, Outcome};
use diary_domain::authentic::DiaryDate;
use serde::Deserialize;
use serde_json::{Value, json};
use std::hash::{Hash, Hasher};
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

/// A 256-bit request fingerprint for idempotency only. shortcut: not cryptographic (std SipHash
/// with four prefixes); upgrade if a digest ever has to resist an adversarial client.
fn digest(parts: &[&str]) -> String {
    (0u8..4)
        .map(|k| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            k.hash(&mut h);
            parts.hash(&mut h);
            format!("{:016x}", h.finish())
        })
        .collect()
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
    let alias = b.profile;
    let view = tokio::task::spawn_blocking(move || profiles.check(&alias)).await;
    let Ok(Some(view)) = view else {
        return failure(StatusCode::NOT_FOUND, "not_found");
    };
    if view.status != Some(ProfileStatus::Ready) || view.chat != Capability::Ready {
        return failure(StatusCode::CONFLICT, "profile_not_ready");
    }
    let chat = Chat::new(
        Uuid::new_v4().to_string(),
        session,
        ProfileHandle {
            alias: view.alias.clone(),
            kind: view.kind,
        },
        locale,
    );
    let response = json!({"chat_id":chat.id,"profile":view.alias,"locale":chat.locale.as_str()});
    // One chat per lease: opening another ends the old one, including any running turn.
    if let Some(old) = s.chat.lock().expect("chat poisoned").replace(chat) {
        old.cancel();
    }
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
    let document = match b.document.as_ref().map(|d| {
        let kind = match d.kind.as_str() {
            "authentic" => DocumentKind::Authentic,
            "source" => DocumentKind::Source,
            _ => return None,
        };
        Some(ChatDocument {
            kind,
            date: DiaryDate::parse(&d.date)?,
            unsaved: d.unsaved,
            body: d.body.clone(),
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
    let (profile, locale, packet) = {
        let chat = s.chat.lock().expect("chat poisoned");
        let Some(chat) = chat
            .as_ref()
            .filter(|c| c.id == chat_id && c.session == session)
        else {
            return failure(StatusCode::NOT_FOUND, "not_found");
        };
        match chat.packet(&b.message, document) {
            Ok(p) => (chat.profile.clone(), chat.locale.clone(), p),
            Err(e) => return failure(StatusCode::UNPROCESSABLE_ENTITY, e.0.as_str()),
        }
    };
    let kind = profile.kind;
    let request = AgentRequest::new(profile, locale, Mode::Chat(packet));
    let definition = match runner.resolve(PromptSelection::for_request(&request)) {
        Ok(prompt) => prompt,
        Err(e) => return preparation_failure(&s, kind, e),
    };
    let prepared = match definition.prepare(request) {
        Ok(prepared) => prepared,
        Err(e) => return preparation_failure(&s, kind, e),
    };
    let doc_text = b.document.as_ref().map_or("", |d| d.body.as_str());
    let doc_meta = b.document.as_ref().map_or(String::new(), |d| {
        format!("{}|{}|{}", d.kind, d.date, d.unsaved)
    });
    let key = digest(&[&chat_id, &b.message, &doc_meta, doc_text]);
    let reserved = b.message.len() + doc_text.len() + 4096;
    let job_id = Uuid::new_v4().to_string();
    let mut c = s.coordinator.lock().expect("coordinator poisoned");
    let job = match c.admit_agent(&session, s.now(), &b.request_id, &key, &job_id, reserved) {
        Ok(j) => j,
        Err(e) => return app_error(e),
    };
    let response = json!({"job":JobDto::from(job.clone())});
    if job.state != JobState::Queued {
        return (StatusCode::ACCEPTED, Json(response)).into_response();
    }
    let run = Run::new();
    let shared = diary_application::run::Shared::default();
    {
        let mut chat = s.chat.lock().expect("chat poisoned");
        match chat.as_mut().filter(|c| c.id == chat_id) {
            Some(chat) => {
                chat.start(
                    &job.id,
                    &b.message,
                    Run::from(run.token()),
                    shared.clone(),
                    prepared.provenance().clone(),
                );
                chat.observe_admission(
                    b.request_id.clone(),
                    b.document.as_ref().map(|d| d.date.clone()),
                    s.now(),
                );
            }
            None => {
                // The chat was closed between the check and admission.
                if let Ok(j) = c.transition(&session, s.now(), &job.id, JobState::Cancelled) {
                    imports::publish(&s, &mut c, &session, &j);
                }
                return failure(StatusCode::NOT_FOUND, "not_found");
            }
        }
    }
    let running = c
        .transition(&session, s.now(), &job.id, JobState::Running)
        .expect("new admission");
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
        let terminal = match &result {
            Ok(AgentOutput::Reply(_)) => JobState::Succeeded,
            Err(e) if e.0 == AgentErrorCode::Cancelled => JobState::Cancelled,
            _ => JobState::Failed,
        };
        if let Some(chat) = s.chat.lock().expect("chat poisoned").as_mut() {
            chat.finish(&job.id, result);
            chat.observe_completion(&job.id, s.now());
        }
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
    let mut guard = s.chat.lock().expect("chat poisoned");
    if guard
        .as_ref()
        .is_some_and(|c| c.id == chat_id && c.session == session)
        && let Some(old) = guard.take()
    {
        old.cancel();
    }
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

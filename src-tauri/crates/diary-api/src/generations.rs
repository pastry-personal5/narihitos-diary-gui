//! Session-owned candidate admission and bounded reads. No diary/model text enters diagnostics.
#![allow(clippy::result_large_err)]
use super::*;
use axum::extract::rejection::JsonRejection;
use diary_application::{
    agent::*,
    generation::{self, Slot},
};
use diary_domain::authentic::DiaryDate;
use std::{
    hash::{Hash, Hasher},
    sync::atomic::Ordering,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Selection {
    import_id: String,
    source_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Start {
    request_id: String,
    date: String,
    selections: Vec<Selection>,
    profile: String,
    locale: String,
    count: u8,
}
fn error(e: generation::Error) -> Response {
    use generation::Error::*;
    match e {
        Coordination(e) => app_error(e),
        EmptySelection => failure(StatusCode::UNPROCESSABLE_ENTITY, "empty_selection"),
        ChangedSource => failure(StatusCode::CONFLICT, "reimport_required"),
        InvalidInput => failure(StatusCode::BAD_REQUEST, "invalid_input"),
        Context => failure(StatusCode::UNPROCESSABLE_ENTITY, "context"),
        ProfileNotReady => failure(StatusCode::CONFLICT, "profile_not_ready"),
        Unavailable => failure(StatusCode::SERVICE_UNAVAILABLE, "unavailable"),
        DiscardRequired => failure(StatusCode::CONFLICT, "discard_required"),
        NotFound => failure(StatusCode::NOT_FOUND, "result_unavailable"),
        Busy => failure(StatusCode::CONFLICT, "busy"),
    }
}
fn fingerprint(b: &Start) -> String {
    let mut pairs: Vec<_> = b
        .selections
        .iter()
        .map(|s| (&s.import_id, &s.source_id))
        .collect();
    pairs.sort();
    (0u8..4)
        .map(|k| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (k, &b.date, &pairs, &b.profile, &b.locale, b.count).hash(&mut h);
            format!("{:016x}", h.finish())
        })
        .collect()
}
pub(super) fn progress(s: &ServerState, session: &str, job: &Job) -> Option<Value> {
    let g = s
        .generations
        .lock()
        .expect("generations poisoned")
        .get(session, &job.id)?;
    let g = g.lock().expect("generation poisoned");
    let snap = g.shared.lock().expect("snapshot poisoned");
    Some(progress_locked(s, session, &g, &snap))
}
fn progress_locked(
    s: &ServerState,
    session: &str,
    g: &generation::Generation,
    snap: &diary_application::run::Snapshot,
) -> Value {
    let c = g.counts();
    json!({"kind":"generation","job_id":g.job_id,"request_id":g.request_id,"instance_id":s.instance_id,"session_id":session,
        "origin_date":g.context.date().to_string(),"profile_id":g.context.profile().alias,"locale":g.context.locale().as_str(),
        "baseline_revision":g.context.baseline().map(|r|json!({"sequence":r.sequence,"sha256":r.sha256})),
        "candidate_index":g.candidate_index,"requested_count":c.requested,"finished_count":c.finished,"ready_count":c.ready,"failed_count":c.failed,"cancelled_count":c.cancelled,"not_started_count":c.not_started,
        "phase":chats::phase(snap.phase),"elapsed_ms":g.finished_at.unwrap_or_else(||s.now()).saturating_sub(g.started_at),"last_agent_activity_at":snap.last_activity_at,"cancellation_requested":g.cancellation_requested(),
        "candidate_ids":g.slots.iter().enumerate().filter(|(_,slot)|**slot==Slot::Ready).map(|(i,_)|g.candidate_id(i)).collect::<Vec<_>>(),"error_code":g.error.map(AgentErrorCode::as_str)})
}
fn publish(s: &ServerState, c: &mut Coordinator, session: &str, job: &Job, ready: bool) {
    let kind = if ready {
        diary_application::EventKind::CandidateReady
    } else {
        diary_application::EventKind::JobChanged
    };
    let bytes=serde_json::to_vec(&json!({"instance_id":s.instance_id,"session_id":session,"event_id":u64::MAX,"job_id":job.id,"job_sequence":job.sequence,"kind":kind.as_str(),"payload":JobDto::from(job.clone())})).expect("fixed DTO").len();
    let _ = c.publish_kind(session, s.now(), &job.id, bytes, kind);
}
pub(super) async fn start(
    State(s): State<Arc<ServerState>>,
    headers: HeaderMap,
    b: Result<Json<Start>, JsonRejection>,
) -> Response {
    let session = match imports::authorize(&s, &headers) {
        Ok(id) => id,
        Err(r) => return r,
    };
    let b = match imports::body(b) {
        Ok(b) => b,
        Err(r) => return r,
    };
    let (Some(date), Ok(locale)) = (DiaryDate::parse(&b.date), Locale::parse(&b.locale)) else {
        return failure(StatusCode::BAD_REQUEST, "invalid_input");
    };
    if !imports::opaque(&b.request_id)
        || !imports::opaque(&b.profile)
        || !(1..=5).contains(&b.count)
        || b.selections
            .iter()
            .any(|p| !imports::opaque(&p.import_id) || !imports::opaque(&p.source_id))
    {
        return failure(StatusCode::BAD_REQUEST, "invalid_input");
    }
    let digest = fingerprint(&b);
    // Authenticate first, then recover an admission before any fresh source/profile/cap checks.
    {
        let mut c = s.coordinator.lock().expect("coordinator poisoned");
        match c.lookup_request(&session, s.now(), &b.request_id, &digest) {
            Ok(Some(job)) => {
                if s.generations
                    .lock()
                    .expect("generations poisoned")
                    .get(&session, &job.id)
                    .is_none()
                {
                    return failure(StatusCode::NOT_FOUND, "result_unavailable");
                };
                return (StatusCode::ACCEPTED, Json(json!({"job":JobDto::from(job)})))
                    .into_response();
            }
            Err(e) => return app_error(e),
            Ok(None) => {}
        }
        if c.closing(&session, s.now()).unwrap_or(true)
            || c.active_agent(&session, s.now()).is_ok_and(|j| j.is_some())
        {
            return failure(StatusCode::CONFLICT, "busy");
        }
    }
    if b.selections.is_empty() {
        return failure(StatusCode::UNPROCESSABLE_ENTITY, "empty_selection");
    }
    let (Some(profiles), Some(runner), Some(imports), Some(authentic)) = (
        s.profiles.clone(),
        s.agent_runner.clone(),
        s.imports.clone(),
        s.authentic.clone(),
    ) else {
        return failure(StatusCode::SERVICE_UNAVAILABLE, "unavailable");
    };
    let capture_s = s.clone();
    let capture_session = session.clone();
    let alias = b.profile.clone();
    let captured = tokio::task::spawn_blocking(move || {
        let view = profiles
            .check(&alias)
            .ok_or(generation::Error::ProfileNotReady)?;
        if view.status != Some(ProfileStatus::Ready) || view.generate != Capability::Ready {
            return Err(generation::Error::ProfileNotReady);
        }
        let generation = capture_s.config_generation.load(Ordering::Acquire);
        let f = generation::capture(
            generation::Capture {
                date,
                selections: b
                    .selections
                    .into_iter()
                    .map(|p| generation::Selection {
                        import_id: p.import_id,
                        source_id: p.source_id,
                    })
                    .collect(),
                profile: ProfileHandle {
                    alias: view.alias,
                    kind: view.kind,
                },
                locale,
                count: b.count,
                config_generation: generation,
            },
            &*imports,
            &*authentic,
            &|| imports::cancelled(&capture_s, &capture_session),
        )?;
        if capture_s.config_generation.load(Ordering::Acquire) != generation {
            return Err(generation::Error::ChangedSource);
        }
        Ok(f)
    })
    .await;
    let frozen = match captured {
        Ok(Ok(f)) => f,
        Ok(Err(e)) => return error(e),
        Err(_) => return failure(StatusCode::SERVICE_UNAVAILABLE, "unavailable"),
    };
    let job_id = Uuid::new_v4().to_string();
    let mut c = s.coordinator.lock().expect("coordinator poisoned");
    let admitted = s.generations.lock().expect("generations poisoned").admit(
        &mut c,
        generation::Admission {
            session: &session,
            now: s.now(),
            request_id: &b.request_id,
            digest: &digest,
            job_id: &job_id,
        },
        frozen,
    );
    let (job, g) = match admitted {
        Ok(v) => v,
        Err(e) => return error(e),
    };
    // A simultaneous identical submission may have won while capture was running.
    if job.id != job_id {
        return (StatusCode::ACCEPTED, Json(json!({"job":JobDto::from(job)}))).into_response();
    }
    let running = c
        .transition(&session, s.now(), &job.id, JobState::Running)
        .expect("new admission");
    publish(&s, &mut c, &session, &running, false);
    drop(c);
    let response = json!({"job":JobDto::from(running)});
    tokio::task::spawn_blocking(move || {
        let mut last_ready = 0;
        let mut notify = || {
            let ready = g.lock().expect("generation poisoned").counts().ready;
            let mut c = s.coordinator.lock().expect("coordinator poisoned");
            if let Ok(job) = c.touch(&session, s.now(), &job.id) {
                publish(&s, &mut c, &session, &job, ready > last_ready)
            } else {
                g.lock().expect("generation poisoned").cancel();
            }
            last_ready = ready;
        };
        generation::execute(&g, &*runner, &|| s.now(), &mut notify);
        let state = g
            .lock()
            .expect("generation poisoned")
            .terminal
            .expect("execution settled");
        let mut c = s.coordinator.lock().expect("coordinator poisoned");
        if let Ok(job) = c.transition(&session, s.now(), &job.id, state) {
            let _ = s.generations.lock().expect("generations poisoned").settle(
                &mut c,
                &session,
                s.now(),
                &job.id,
            );
            publish(&s, &mut c, &session, &job, false);
            log_finished(&s, &session, &job, &g);
        }
    });
    (StatusCode::ACCEPTED, Json(response)).into_response()
}
fn log_finished(s: &ServerState, session: &str, job: &Job, g: &generation::Shared) {
    let g = g.lock().expect("generation poisoned");
    let counts = g.counts();
    let mut detail = diary_application::diagnostics::AgentDetail::new(g.context.profile().kind);
    detail.action = Some(diary_application::diagnostics::ActionTag::Generate);
    detail.phase = Some(diary_application::diagnostics::PhaseTag::Finished);
    detail.code = g.error;
    detail.candidate_index = g.candidate_index.map(u32::from);
    detail.requested = Some(counts.requested.into());
    detail.ready = Some(counts.ready.into());
    detail.failed = Some(counts.failed.into());
    detail.cancelled = Some(counts.cancelled.into());
    detail.not_started = Some(counts.not_started.into());
    let mut record = Record::new(
        EventCode::AgentRun,
        if job.state == JobState::Failed {
            Level::Warn
        } else {
            Level::Info
        },
    )
    .agent(detail);
    record.job = OpaqueId::parse(&job.id);
    record.session = OpaqueId::parse(session);
    s.log(record);
}
async fn view(s: Arc<ServerState>, headers: HeaderMap, id: String, preview: bool) -> Response {
    let session = match imports::authorize(&s, &headers) {
        Ok(id) => id,
        Err(r) => return r,
    };
    let job = match s
        .coordinator
        .lock()
        .expect("coordinator poisoned")
        .get(&session, s.now(), &id)
    {
        Ok(j) => j,
        Err(e) => return app_error(e),
    };
    let Some(g) = s
        .generations
        .lock()
        .expect("generations poisoned")
        .get(&session, &id)
    else {
        return failure(StatusCode::NOT_FOUND, "result_unavailable");
    };
    let (date, baseline, profile, locale, count, slots, provisional, progress) = {
        let g = g.lock().expect("generation poisoned");
        let snap = g.shared.lock().expect("snapshot poisoned");
        (g.context.date(),g.context.baseline().cloned(),g.context.profile().alias.clone(),g.context.locale().as_str().to_owned(),g.context.count(),g.slots.iter().enumerate().map(|(i,slot)|json!({"index":i+1,"id":(*slot==Slot::Ready).then(||g.candidate_id(i)),"status":match slot {Slot::NotStarted=>"not_started",Slot::Running=>"running",Slot::Ready=>"ready",Slot::Failed(_)=>"failed",Slot::Cancelled=>"cancelled"},"error_code":if let Slot::Failed(code)=slot {Some(code.as_str())}else{None}})).collect::<Vec<_>>(),(preview&&g.terminal.is_none()&&g.candidate_index.is_some()).then(||snap.text.clone()),progress_locked(&s,&session,&g,&snap))
    };
    let Some(authentic) = s.authentic.clone() else {
        return failure(StatusCode::SERVICE_UNAVAILABLE, "unavailable");
    };
    let current = tokio::task::spawn_blocking(move || authentic.load(&date)).await;
    let current = match current {
        Ok(Ok(diary_application::authentic::Loaded::Virtual(_))) => None,
        Ok(Ok(diary_application::authentic::Loaded::File { revision, .. })) => Some(revision),
        _ => return failure(StatusCode::SERVICE_UNAVAILABLE, "unavailable"),
    };
    if let Err(r) = imports::authorize(&s, &headers) {
        return r;
    }
    if s.generations
        .lock()
        .expect("generations poisoned")
        .get(&session, &id)
        .is_none()
    {
        return failure(StatusCode::NOT_FOUND, "result_unavailable");
    }
    let mut value = json!({"job":JobDto::from(job.clone()),"date":date.to_string(),"profile":profile,"locale":locale,"count":count,"stale_baseline":baseline!=current,"candidates":slots,"progress":progress});
    if preview {
        value["provisional"] = json!(provisional)
    }
    Json(value).into_response()
}
pub(super) async fn get(
    State(s): State<Arc<ServerState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    view(s, headers, id, false).await
}
pub(super) async fn result(
    State(s): State<Arc<ServerState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    view(s, headers, id, true).await
}
pub(super) async fn discard(
    State(s): State<Arc<ServerState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let session = match imports::authorize(&s, &headers) {
        Ok(id) => id,
        Err(r) => return r,
    };
    let mut c = s.coordinator.lock().expect("coordinator poisoned");
    match s.generations.lock().expect("generations poisoned").discard(
        &mut c,
        &session,
        s.now(),
        &id,
    ) {
        Ok(()) => Json(json!({})).into_response(),
        Err(e) => error(e),
    }
}
pub(super) async fn candidate(
    State(s): State<Arc<ServerState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let session = match imports::authorize(&s, &headers) {
        Ok(id) => id,
        Err(r) => return r,
    };
    let Some((job_id, index)) = id.rsplit_once('-').and_then(|(job, i)| {
        i.parse::<usize>()
            .ok()
            .filter(|i| (1..=5).contains(i))
            .map(|i| (job, i - 1))
    }) else {
        return failure(StatusCode::BAD_REQUEST, "invalid_input");
    };
    let Some(g) = s
        .generations
        .lock()
        .expect("generations poisoned")
        .get(&session, job_id)
    else {
        return failure(StatusCode::NOT_FOUND, "result_unavailable");
    };
    let g = g.lock().expect("generation poisoned");
    let Some(candidate) = g.candidates.get(index).and_then(Option::as_ref) else {
        return failure(StatusCode::NOT_FOUND, "not_found");
    };
    Json(json!({"id":id,"job_id":job_id,"index":index+1,"date":g.context.date().to_string(),"draft_markdown":candidate.draft_markdown,"claim_support":candidate.claim_support.iter().map(|c|json!({"claim":c.claim,"snippet_ids":c.snippet_ids})).collect::<Vec<_>>(),"warnings":candidate.warnings})).into_response()
}

#[cfg(test)]
mod tests;

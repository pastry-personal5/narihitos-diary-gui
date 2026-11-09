//! Job list/get/cancel routes and the server-sent event stream.
use super::*;

#[derive(Serialize)]
pub(crate) struct JobDto {
    id: String,
    sequence: u64,
    state: &'static str,
}
impl From<Job> for JobDto {
    fn from(job: Job) -> Self {
        Self {
            id: job.id,
            sequence: job.sequence,
            state: match job.state {
                JobState::Queued => "queued",
                JobState::Running => "running",
                JobState::Succeeded => "succeeded",
                JobState::Failed => "failed",
                JobState::Cancelled => "cancelled",
            },
        }
    }
}
pub(crate) fn job_value(s: &ServerState, session: &str, job: Job) -> Value {
    let progress =
        generations::progress(s, session, &job).or_else(|| chats::progress(s, session, &job));
    let mut value = serde_json::to_value(JobDto::from(job)).expect("fixed DTO");
    if let Some(Value::Object(p)) = progress {
        value["progress"] = Value::Object(p.clone());
        value.as_object_mut().expect("object").extend(p);
    }
    value
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct JobQuery {
    cursor: Option<String>,
}
pub(crate) async fn list_jobs(
    State(s): State<Arc<ServerState>>,
    headers: HeaderMap,
    Query(query): Query<JobQuery>,
) -> Response {
    let id = match session(&headers) {
        Ok(id) => id,
        Err(e) => return app_error(e),
    };
    let cursor = query.cursor.as_deref();
    let mut coordinator = s.coordinator.lock().expect("coordinator poisoned");
    match coordinator.list(id, s.now(), cursor, 200) {
        Ok(jobs) => {
            let next = if jobs.len() == 200 {
                jobs.last().map(|job| job.id.clone())
            } else {
                None
            };
            Json(json!({"jobs":jobs.into_iter().map(|j| job_value(&s,id,j)).collect::<Vec<_>>(),"next_cursor":next,"event_cursor":coordinator.event_cursor()})).into_response()
        }
        Err(e) => app_error(e),
    }
}
pub(crate) async fn get_job(
    State(s): State<Arc<ServerState>>,
    Path(job): Path<String>,
    headers: HeaderMap,
) -> Response {
    let id = match session(&headers) {
        Ok(id) => id,
        Err(e) => return app_error(e),
    };
    match s
        .coordinator
        .lock()
        .expect("coordinator poisoned")
        .get(id, s.now(), &job)
    {
        Ok(job) => {
            let errors = s.import_errors.lock().expect("import errors poisoned");
            let code = if errors.0 == id {
                errors.1.get(&job.id).copied()
            } else {
                None
            };
            let mut value = job_value(&s, id, job);
            if let Some(code) = code {
                value["error_code"] = json!(code);
            }
            Json(value).into_response()
        }
        Err(e) => app_error(e),
    }
}
pub(crate) async fn cancel_job(
    State(s): State<Arc<ServerState>>,
    Extension(trace): Extension<TraceId>,
    Path(job): Path<String>,
    headers: HeaderMap,
) -> Response {
    let id = match session(&headers) {
        Ok(id) => id,
        Err(e) => return app_error(e),
    };
    if exports::is_export_job(&s, &job) {
        return exports::cancel(&s, id, &job).await;
    }
    let mut c = s.coordinator.lock().expect("coordinator poisoned");
    let was_terminal = c
        .get(id, s.now(), &job)
        .is_ok_and(|j| j.state.is_terminal());
    let generation = s
        .generations
        .lock()
        .expect("generations poisoned")
        .get(id, &job);
    if let Some(g) = generation {
        let changed = {
            let g = g.lock().expect("generation poisoned");
            if !was_terminal && g.cancel() {
                let mut snap = g.shared.lock().expect("snapshot poisoned");
                snap.phase = Some(diary_application::agent::Phase::Stopping);
                snap.text = String::new();
                snap.revision += 1;
                true
            } else {
                false
            }
        };
        if changed && let Ok(j) = c.touch(id, s.now(), &job) {
            imports::publish(&s, &mut c, id, &j);
        }
        return match c.get(id, s.now(), &job) {
            Ok(j) => Json(job_value(&s, id, j)).into_response(),
            Err(e) => app_error(e),
        };
    }
    let mut chat_job = false;
    s.conversations.each(|chat| {
        chat_job |= chat.turn().is_some_and(|t| t.job_id == job);
    });
    if chat_job && !was_terminal {
        s.conversations.each(|chat| {
            chat.cancel_job(&job);
        });
        if let Ok(j) = c.touch(id, s.now(), &job) {
            imports::publish(&s, &mut c, id, &j);
        }
        return match c.get(id, s.now(), &job) {
            Ok(j) => Json(JobDto::from(j)).into_response(),
            Err(e) => app_error(e),
        };
    }
    match c.cancel(id, s.now(), &job) {
        Ok(job) => {
            s.conversations.each(|chat| {
                chat.cancel_job(&job.id);
            });
            // Compute the exact encoded envelope size before retaining notification.
            let dto = JobDto::from(job.clone());
            let bytes = serde_json::to_vec(&json!({"instance_id":s.instance_id,"session_id":id,"event_id":u64::MAX,"job_id":job.id,"job_sequence":job.sequence,"kind":"job_changed","payload":dto})).expect("fixed DTO").len();
            let _ = c.publish(id, s.now(), &job.id, bytes);
            if job.state.is_terminal() && !was_terminal {
                let mut record =
                    in_session(traced(EventCode::JobFinished, Level::Info, &trace), id);
                // Fixture job IDs are not opaque IDs; real (UUID) job IDs are recorded.
                record.job = OpaqueId::parse(&job.id);
                s.log(record);
            }
            Json(JobDto::from(job)).into_response()
        }
        Err(e) => app_error(e),
    }
}
pub(crate) async fn events(State(s): State<Arc<ServerState>>, headers: HeaderMap) -> Response {
    let id = match session(&headers) {
        Ok(id) => id.to_owned(),
        Err(e) => return app_error(e),
    };
    let after = match header(&headers, "last-event-id") {
        Some(v) => match v.parse::<u64>() {
            Ok(n) => n,
            Err(_) => return failure(StatusCode::BAD_REQUEST, "invalid_cursor"),
        },
        None => 0,
    };
    let instance = header(&headers, "x-diary-instance")
        .unwrap_or(&s.instance_id)
        .to_owned();
    if let Err(e) = s
        .coordinator
        .lock()
        .expect("coordinator poisoned")
        .list(&id, s.now(), None, 1)
    {
        return app_error(e);
    }
    let permit = match s.sse_slots.clone().try_acquire_owned() {
        Ok(permit) => permit,
        Err(_) => return failure(StatusCode::TOO_MANY_REQUESTS, "stream_limit"),
    };
    let reconcile = header(&headers, "x-diary-reconcile") == Some("1");
    let start = s.authentic_epoch.load(std::sync::atomic::Ordering::SeqCst);
    let stream = futures_util::stream::unfold(
        (s, id, instance, after, false, permit, start, !reconcile),
        |(s, id, instance, mut after, done, permit, mut seen, ready)| async move {
            if done {
                return None;
            }
            loop {
                let epoch = s.authentic_epoch.load(std::sync::atomic::Ordering::SeqCst);
                if epoch != seen {
                    seen = epoch;
                    // No id, no path, no content: the receiver always rescans.
                    return Some((
                        Ok::<_, Infallible>(
                            SseEvent::default().event("authentic_changed").data("{}"),
                        ),
                        (s, id, instance, after, false, permit, seen, ready),
                    ));
                }
                let replay = s
                    .coordinator
                    .lock()
                    .expect("coordinator poisoned")
                    .next_event(&id, s.now(), &instance, after);
                match replay {
                    Err(_) => return None,
                    Ok(Replay::ResyncRequired) => {
                        return Some((
                            Ok::<_, Infallible>(
                                SseEvent::default().event("resync_required").data("{}"),
                            ),
                            (s, id, instance, after, true, permit, seen, ready),
                        ));
                    }
                    Ok(Replay::Events(events)) => {
                        if let Some(event) = events.first() {
                            after = event.event_id;
                            let data = json!({"instance_id":event.instance_id,"session_id":event.session_id,"event_id":event.event_id,"job_id":event.job.id,"job_sequence":event.job.sequence,"kind":event.kind.as_str(),"payload":JobDto::from(event.job.clone())});
                            let item = SseEvent::default()
                                .id(after.to_string())
                                .event(event.kind.as_str())
                                .json_data(data)
                                .expect("fixed DTO");
                            return Some((
                                Ok(item),
                                (s, id, instance, after, false, permit, seen, ready),
                            ));
                        }
                        if !ready {
                            return Some((
                                Ok(SseEvent::default().event("replay_complete").data("{}")),
                                (s, id, instance, after, false, permit, seen, true),
                            ));
                        }
                    }
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        },
    );
    let mut response = Sse::new(stream)
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(10)))
        .into_response();
    if reconcile {
        response
            .headers_mut()
            .insert("x-diary-reconcile", "1".parse().expect("fixed header"));
    }
    response
}

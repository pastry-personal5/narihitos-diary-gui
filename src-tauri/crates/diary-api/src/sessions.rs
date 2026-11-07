//! Status probe and the client session lifecycle: open, heartbeat, close.
use super::*;

pub(crate) async fn status(State(s): State<Arc<ServerState>>) -> Json<Value> {
    Json(
        json!({"instance_id":s.instance_id,"workspace_key":s.workspace_key,"server_version":env!("CARGO_PKG_VERSION"),"api_version":1,"api_minor":0,"config_schema_version":1,"storage_ready":s.storage_ready,"capabilities":["status","sessions","jobs","events","chat","generation"],"limits":{"http_bytes":10485760,"event_bytes":65536,"replay_events":2048,"replay_bytes":4194304,"transient_bytes":67108864,"business_jobs":32,"reserved_save_slots":8,"sse_connections":4,"candidate_jobs":5,"candidate_bytes":8388608}}),
    )
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OpenSession {
    session_id: Option<String>,
}
pub(crate) async fn open_session(
    State(s): State<Arc<ServerState>>,
    Extension(trace): Extension<TraceId>,
    payload: Result<Json<OpenSession>, axum::extract::rejection::JsonRejection>,
) -> Response {
    s.expire_leases(s.now());
    let input = match payload {
        Ok(Json(input)) => input,
        Err(error) => {
            return failure(
                if error.status() == StatusCode::UNSUPPORTED_MEDIA_TYPE {
                    StatusCode::UNSUPPORTED_MEDIA_TYPE
                } else if error.status() == StatusCode::PAYLOAD_TOO_LARGE {
                    StatusCode::PAYLOAD_TOO_LARGE
                } else {
                    StatusCode::BAD_REQUEST
                },
                "invalid_input",
            );
        }
    };
    let id = input
        .session_id
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    if Uuid::parse_str(&id).is_err() {
        return failure(StatusCode::BAD_REQUEST, "invalid_input");
    }
    let mut coordinator = s.coordinator.lock().expect("coordinator poisoned");
    match coordinator.open(&id, s.now()) {
        Ok(()) => {
            #[cfg(feature = "scaffold-smoke")]
            if let Err(error) = smoke::admit(&mut coordinator, &id, s.now()) {
                return app_error(error);
            }
            s.log(in_session(
                traced(EventCode::SessionOpened, Level::Info, &trace),
                &id,
            ));
            Json(json!({"session_id":id,"heartbeat_interval_ms":5000,"lease_duration_ms":30000}))
                .into_response()
        }
        Err(e) => {
            if e == Error::ClientBusy {
                s.log(traced(EventCode::ClientBusy, Level::Warn, &trace).error(ErrorTag::Busy));
            }
            app_error(e)
        }
    }
}
pub(crate) fn session(headers: &HeaderMap) -> Result<&str, Error> {
    header(headers, "x-diary-session")
        .filter(|id| Uuid::parse_str(id).is_ok())
        .ok_or(Error::SessionExpired)
}
pub(crate) async fn heartbeat(
    State(s): State<Arc<ServerState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    if session(&headers).ok() != Some(id.as_str()) {
        return failure(StatusCode::UNAUTHORIZED, "session_expired");
    }
    match s
        .coordinator
        .lock()
        .expect("coordinator poisoned")
        .heartbeat(&id, s.now())
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => app_error(e),
    }
}
pub(crate) async fn close_session(
    State(s): State<Arc<ServerState>>,
    Extension(trace): Extension<TraceId>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    if session(&headers).ok() != Some(id.as_str()) {
        return failure(StatusCode::UNAUTHORIZED, "session_expired");
    }
    let active = {
        let mut c = s.coordinator.lock().expect("coordinator poisoned");
        let active = match c.begin_close(&id, s.now()) {
            Ok(job) => job,
            Err(e) => return app_error(e),
        };
        if let Some(job) = &active {
            s.generations
                .lock()
                .expect("generations poisoned")
                .cancel_job(&id, &job.id);
            if let Some(chat) = s.chat.lock().expect("chat poisoned").as_ref() {
                chat.cancel_job(&job.id);
            }
        }
        active
    };
    if let Some(job) = active {
        let acknowledgement = tokio::time::timeout(Duration::from_secs(30), async {
            loop {
                let state =
                    s.coordinator
                        .lock()
                        .expect("coordinator poisoned")
                        .get(&id, s.now(), &job.id);
                match state {
                    Ok(job) if job.state.is_terminal() => break,
                    Err(_) => break,
                    _ => {}
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        })
        .await;
        if acknowledgement.is_err() {
            return failure(StatusCode::SERVICE_UNAVAILABLE, "busy");
        }
    }
    match s
        .coordinator
        .lock()
        .expect("coordinator poisoned")
        .close(&id, s.now())
    {
        Ok(()) => {
            s.lease_ended();
            s.log(in_session(
                traced(EventCode::SessionClosed, Level::Info, &trace),
                &id,
            ));
            StatusCode::NO_CONTENT.into_response()
        }
        Err(e) => app_error(e),
    }
}

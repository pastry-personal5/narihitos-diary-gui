//! Explicit authenticated scaffold transport; no content mutation or fake admission route.
mod authentic;
mod calendar;
mod imports;
mod profiles;
#[cfg(feature = "scaffold-smoke")]
mod smoke;
mod source_save;
mod source_trash;
use axum::{
    Extension, Json, Router,
    extract::{ConnectInfo, DefaultBodyLimit, Path, Query, Request, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    middleware::{self, Next},
    response::{
        IntoResponse, Response,
        sse::{Event as SseEvent, KeepAlive, Sse},
    },
    routing::{delete, get, post, put},
};
use diary_application::diagnostics::{
    Diagnostics, ErrorTag, EventCode, Level, NullDiagnostics, OpaqueId, Record, TraceId,
};
use diary_application::{Coordinator, Error, Job, JobState, Limits, Replay};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
#[cfg(feature = "scaffold-smoke")]
pub use smoke::FolderCheck;
use std::{
    convert::Infallible,
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::Duration,
};
use subtle::ConstantTimeEq;
use uuid::Uuid;

pub struct ServerState {
    #[cfg(feature = "scaffold-smoke")]
    folder_checks: Vec<FolderCheck>,
    token: String,
    host: String,
    instance_id: String,
    workspace_key: String,
    development: bool,
    startup: Mutex<diary_application::StartupSummary>,
    reloader: Option<Arc<dyn diary_application::ConfigReloader>>,
    config_generation: std::sync::atomic::AtomicU64,
    last_reload: Mutex<Value>,
    storage_ready: bool,
    authentic: Option<Arc<dyn diary_application::authentic::AuthenticStore>>,
    imports: Option<Arc<dyn diary_application::imports::ImportStore>>,
    source_writer: Option<Arc<dyn diary_application::source::SourceWriter>>,
    source_trash: Option<Arc<dyn diary_application::source_trash::SourceTrash>>,
    profiles: Option<Arc<dyn diary_application::agent::ProfileDirectory>>,
    lease_ended: Option<Arc<dyn Fn() + Send + Sync>>,
    source_slots: Arc<tokio::sync::Semaphore>,
    import_errors: Mutex<(String, std::collections::BTreeMap<String, &'static str>)>,
    sse_slots: Arc<tokio::sync::Semaphore>,
    clock: Arc<dyn Fn() -> u64 + Send + Sync>,
    stopping: std::sync::atomic::AtomicBool,
    authentic_epoch: std::sync::atomic::AtomicU64,
    coordinator: Mutex<Coordinator>,
    diagnostics: Arc<dyn Diagnostics>,
}
impl ServerState {
    pub fn new(
        token: String,
        port: u16,
        instance_id: String,
        workspace_key: String,
        development: bool,
    ) -> Arc<Self> {
        Arc::new(Self {
            #[cfg(feature = "scaffold-smoke")]
            folder_checks: Vec::new(),
            token,
            host: format!("127.0.0.1:{port}"),
            coordinator: Mutex::new(Coordinator::new(instance_id.clone(), Limits::default())),
            instance_id,
            workspace_key,
            development,
            startup: Mutex::new(diary_application::StartupSummary::default()),
            reloader: None,
            config_generation: std::sync::atomic::AtomicU64::new(1),
            last_reload: Mutex::new(Value::Null),
            storage_ready: false,
            authentic: None,
            imports: None,
            source_writer: None,
            source_trash: None,
            profiles: None,
            lease_ended: None,
            source_slots: Arc::new(tokio::sync::Semaphore::new(2)),
            import_errors: Mutex::new((String::new(), std::collections::BTreeMap::new())),
            authentic_epoch: std::sync::atomic::AtomicU64::new(0),
            sse_slots: Arc::new(tokio::sync::Semaphore::new(4)),
            clock: Arc::new(|| {
                // macOS CLOCK_MONOTONIC_RAW includes time spent asleep.
                let mut value = std::mem::MaybeUninit::<libc::timespec>::uninit();
                // SAFETY: clock_gettime initializes the supplied timespec on success.
                if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC_RAW, value.as_mut_ptr()) }
                    != 0
                {
                    return u64::MAX;
                }
                let value = unsafe { value.assume_init() };
                (value.tv_sec as u64)
                    .saturating_mul(1000)
                    .saturating_add(value.tv_nsec as u64 / 1_000_000)
            }),
            stopping: std::sync::atomic::AtomicBool::new(false),
            diagnostics: Arc::new(NullDiagnostics),
        })
    }
    /// True only when the workspace opened `Ready`; recovery and inspection modes stay false.
    pub fn set_storage_ready(state: &mut Arc<Self>, ready: bool) {
        Arc::get_mut(state)
            .expect("startup precedes shared use")
            .storage_ready = ready;
    }
    /// Invalidation hint from the file watcher; open event streams announce it once each.
    pub fn authentic_changed(&self) {
        self.authentic_epoch
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
    /// The Authentic store: writable when the workspace is `Ready`, read-only in recovery.
    pub fn set_authentic(
        state: &mut Arc<Self>,
        store: Arc<dyn diary_application::authentic::AuthenticStore>,
    ) {
        Arc::get_mut(state)
            .expect("startup precedes shared use")
            .authentic = Some(store);
    }
    pub fn set_imports(
        state: &mut Arc<Self>,
        store: Arc<dyn diary_application::imports::ImportStore>,
    ) {
        Arc::get_mut(state)
            .expect("startup precedes shared use")
            .imports = Some(store);
    }
    pub fn set_source_writer(
        state: &mut Arc<Self>,
        writer: Arc<dyn diary_application::source::SourceWriter>,
    ) {
        Arc::get_mut(state)
            .expect("startup precedes shared use")
            .source_writer = Some(writer);
    }
    pub fn set_source_trash(
        state: &mut Arc<Self>,
        service: Arc<dyn diary_application::source_trash::SourceTrash>,
    ) {
        Arc::get_mut(state)
            .expect("startup precedes shared use")
            .source_trash = Some(service);
    }
    /// Called when the client lease ends (expiry, close or shutdown): agent runs belong to the lease.
    pub fn set_lease_ended(state: &mut Arc<Self>, hook: Arc<dyn Fn() + Send + Sync>) {
        Arc::get_mut(state)
            .expect("startup precedes shared use")
            .lease_ended = Some(hook);
    }
    fn lease_ended(&self) {
        if let Some(hook) = &self.lease_ended {
            hook();
        }
    }
    /// One sweep step; the hook runs after the coordinator lock is released.
    fn expire_leases(&self, now: u64) {
        let expired = self
            .coordinator
            .lock()
            .expect("coordinator poisoned")
            .expire(now);
        if expired {
            self.lease_ended();
        }
    }
    /// Enables `GET /profiles` and `POST /profiles/{alias}/check`; read-only, never starts a run.
    pub fn set_profiles(
        state: &mut Arc<Self>,
        profiles: Arc<dyn diary_application::agent::ProfileDirectory>,
    ) {
        Arc::get_mut(state)
            .expect("startup precedes shared use")
            .profiles = Some(profiles);
    }
    pub fn set_startup(state: &mut Arc<Self>, summary: diary_application::StartupSummary) {
        *Arc::get_mut(state)
            .expect("startup precedes shared use")
            .startup
            .get_mut()
            .expect("startup state poisoned") = summary;
    }
    /// Enables `POST /settings/reload`; without it the route answers `unavailable`.
    pub fn set_reloader(
        state: &mut Arc<Self>,
        reloader: Arc<dyn diary_application::ConfigReloader>,
    ) {
        Arc::get_mut(state)
            .expect("startup precedes shared use")
            .reloader = Some(reloader);
    }
    /// Diagnostics sinks must never block; the server wires the asynchronous log worker here.
    pub fn set_diagnostics(state: &mut Arc<Self>, diagnostics: Arc<dyn Diagnostics>) {
        Arc::get_mut(state)
            .expect("startup precedes shared use")
            .diagnostics = diagnostics;
    }
    #[cfg(feature = "scaffold-smoke")]
    pub fn set_smoke_folders(state: &mut Arc<Self>, checks: Vec<FolderCheck>) {
        Arc::get_mut(state).expect("startup unshared").folder_checks = checks;
    }
    fn now(&self) -> u64 {
        (self.clock)()
    }
    fn log(&self, record: Record) {
        self.diagnostics.emit(record);
    }
    pub fn shutdown(&self) {
        self.stopping
            .store(true, std::sync::atomic::Ordering::Release);
        self.expire_leases(u64::MAX);
    }
    pub async fn sweep(self: Arc<Self>) {
        while !self.stopping.load(std::sync::atomic::Ordering::Acquire) {
            self.expire_leases(self.now());
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
}

pub fn router(state: Arc<ServerState>) -> Router {
    Router::new()
        .route("/api/v1/status", get(status))
        .route("/api/v1/sessions", post(open_session))
        .route("/api/v1/sessions/{id}/heartbeat", post(heartbeat))
        .route("/api/v1/sessions/{id}", delete(close_session))
        .route("/api/v1/settings", get(settings))
        .route("/api/v1/settings/reload", post(reload_settings))
        .route("/api/v1/jobs", get(list_jobs))
        .route("/api/v1/jobs/{id}", get(get_job))
        .route("/api/v1/jobs/{id}/cancel", post(cancel_job))
        .route("/api/v1/events", get(events))
        .route(
            "/api/v1/imports",
            post(imports::create).layer(DefaultBodyLimit::max(4096)),
        )
        .route("/api/v1/profiles", get(profiles::list))
        .route("/api/v1/profiles/{alias}/check", post(profiles::check))
        .route("/api/v1/calendar", get(calendar::get_calendar))
        .route("/api/v1/sources/{id}", get(imports::read))
        .route("/api/v1/sources", get(imports::list))
        .route("/api/v1/sources/{id}/body", put(source_save::save))
        .route(
            "/api/v1/sources/{id}/trash-preview",
            post(source_trash::preview).layer(DefaultBodyLimit::max(4096)),
        )
        .route(
            "/api/v1/sources/{id}/trash",
            post(source_trash::confirm).layer(DefaultBodyLimit::max(4096)),
        )
        .route("/api/v1/operations", get(source_trash::unfinished))
        .route(
            "/api/v1/sources/{id}/time-resolution",
            post(imports::resolve).layer(DefaultBodyLimit::max(4096)),
        )
        .route("/api/v1/authentic", get(authentic::list))
        .route("/api/v1/authentic/refresh", post(authentic::refresh))
        .route(
            "/api/v1/authentic/{date}",
            get(authentic::load).put(authentic::save),
        )
        .route(
            "/api/v1/authentic/{date}/empty",
            post(authentic::create_empty),
        )
        .route("/api/v1/operations/{id}", get(authentic::operation))
        .fallback(|| async { failure(StatusCode::NOT_FOUND, "not_found") })
        .layer(DefaultBodyLimit::max(10 * 1024 * 1024))
        .layer(middleware::from_fn_with_state(state.clone(), authenticate))
        .layer(middleware::from_fn(trace))
        .with_state(state)
}

/// Correlates one request across client and server logs. A missing or invalid caller value
/// is replaced, never trusted; the ID is echoed so a caller can quote it when debugging.
async fn trace(mut request: Request, next: Next) -> Response {
    let id = header(request.headers(), "x-diary-trace-id")
        .and_then(TraceId::parse)
        .unwrap_or_else(|| {
            TraceId::parse(&Uuid::new_v4().simple().to_string()).expect("32 lowercase hex")
        });
    request.extensions_mut().insert(id.clone());
    let mut response = next.run(request).await;
    if let Ok(value) = HeaderValue::from_str(id.as_str()) {
        response.headers_mut().insert("x-diary-trace-id", value);
    }
    response
}
fn traced(code: EventCode, level: Level, trace: &TraceId) -> Record {
    Record::new(code, level).trace(trace.clone())
}
fn in_session(record: Record, session: &str) -> Record {
    match OpaqueId::parse(session) {
        Some(id) => record.session(id),
        None => record,
    }
}
/// Access-boundary rejection: logged by code only, never with the token, headers or body.
fn rejected(
    state: &ServerState,
    trace: &Option<TraceId>,
    status: StatusCode,
    code: &'static str,
) -> Response {
    let tag = match code {
        "server_stopping" | "request_timeout" => ErrorTag::Unavailable,
        "request_limit" => ErrorTag::Limit,
        "unauthorized" | "invalid_origin" | "invalid_destination" => ErrorTag::Denied,
        _ => ErrorTag::Invalid,
    };
    let mut record = Record::new(EventCode::AuthRejected, Level::Warn).error(tag);
    record.trace = trace.clone();
    state.log(record);
    failure(status, code)
}

fn failure(status: StatusCode, code: &'static str) -> Response {
    (
        status,
        Json(json!({"error":{"code":code,"message":code,"retryable":false}})),
    )
        .into_response()
}
fn app_error(error: Error) -> Response {
    let (status, code) = match error {
        Error::ClientBusy => (StatusCode::CONFLICT, "client_busy"),
        Error::SessionExpired => (StatusCode::UNAUTHORIZED, "session_expired"),
        Error::NotFound => (StatusCode::NOT_FOUND, "not_found"),
        Error::Limit => (StatusCode::TOO_MANY_REQUESTS, "limit"),
        Error::RequestMismatch => (StatusCode::CONFLICT, "request_mismatch"),
        Error::InvalidInput => (StatusCode::BAD_REQUEST, "invalid_input"),
        Error::InvalidTransition => (StatusCode::CONFLICT, "invalid_transition"),
    };
    failure(status, code)
}
fn header<'a>(headers: &'a HeaderMap, key: &str) -> Option<&'a str> {
    if headers.get_all(key).iter().count() != 1 {
        return None;
    }
    headers.get(key)?.to_str().ok()
}
async fn authenticate(
    State(state): State<Arc<ServerState>>,
    request: Request,
    next: Next,
) -> Response {
    let trace = request.extensions().get::<TraceId>().cloned();
    if state.stopping.load(std::sync::atomic::Ordering::Acquire) {
        return rejected(
            &state,
            &trace,
            StatusCode::SERVICE_UNAVAILABLE,
            "server_stopping",
        );
    }
    let headers = request.headers();
    let peer_ok = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .is_some_and(|p| p.0.ip() == std::net::Ipv4Addr::LOCALHOST);
    let token = header(headers, "authorization")
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");
    let token_ok: bool = token.as_bytes().ct_eq(state.token.as_bytes()).into();
    if !peer_ok || !token_ok {
        return rejected(&state, &trace, StatusCode::UNAUTHORIZED, "unauthorized");
    }
    if header(headers, "host") != Some(state.host.as_str())
        || request.uri().scheme().is_some()
        || request.uri().authority().is_some()
        || request.uri().query().is_some_and(|query| {
            if request.uri().path() == "/api/v1/calendar" {
                !calendar::valid_query(query)
            } else if request
                .uri()
                .path()
                .strip_prefix("/api/v1/sources/")
                .is_some_and(|id| !id.contains('/'))
            {
                !imports::valid_read_query(query)
            } else if request.uri().path() == "/api/v1/sources" {
                !imports::valid_query(query)
            } else if request.uri().path() == "/api/v1/operations" {
                !source_trash::valid_query(query)
            } else {
                !matches!(request.uri().path(), "/api/v1/jobs" | "/api/v1/authentic")
                    || !query.strip_prefix("cursor=").is_some_and(|cursor| {
                        !cursor.is_empty()
                            && cursor.len() <= 64
                            && cursor
                                .bytes()
                                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
                    })
            }
        })
        || headers.contains_key("forwarded")
        || headers
            .keys()
            .any(|k| k.as_str().starts_with("x-forwarded-"))
    {
        return rejected(&state, &trace, StatusCode::FORBIDDEN, "invalid_destination");
    }
    if headers.contains_key("origin") {
        let origin = header(headers, "origin");
        if origin != Some("tauri://localhost")
            && !(state.development && origin == Some("http://localhost:1420"))
        {
            return rejected(&state, &trace, StatusCode::FORBIDDEN, "invalid_origin");
        }
    }
    if headers.contains_key("content-type")
        && header(headers, "content-type") != Some("application/json")
    {
        return rejected(
            &state,
            &trace,
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "unsupported_media_type",
        );
    }
    let (parts, body) = request.into_parts();
    let bytes = match tokio::time::timeout(
        Duration::from_secs(5),
        axum::body::to_bytes(body, 10 * 1024 * 1024),
    )
    .await
    {
        Err(_) => {
            return rejected(
                &state,
                &trace,
                StatusCode::REQUEST_TIMEOUT,
                "request_timeout",
            );
        }
        Ok(Err(_)) => {
            return rejected(
                &state,
                &trace,
                StatusCode::PAYLOAD_TOO_LARGE,
                "request_limit",
            );
        }
        Ok(Ok(bytes)) => bytes,
    };
    next.run(Request::from_parts(parts, axum::body::Body::from(bytes)))
        .await
}
async fn status(State(s): State<Arc<ServerState>>) -> Json<Value> {
    Json(
        json!({"instance_id":s.instance_id,"workspace_key":s.workspace_key,"server_version":env!("CARGO_PKG_VERSION"),"api_version":1,"api_minor":0,"config_schema_version":1,"storage_ready":s.storage_ready,"capabilities":["status","sessions","jobs","events"],"limits":{"http_bytes":10485760,"event_bytes":65536,"replay_events":2048,"replay_bytes":4194304,"transient_bytes":67108864,"business_jobs":32,"reserved_save_slots":8,"sse_connections":4}}),
    )
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OpenSession {
    session_id: Option<String>,
}
async fn open_session(
    State(s): State<Arc<ServerState>>,
    Extension(trace): Extension<TraceId>,
    payload: Result<Json<OpenSession>, axum::extract::rejection::JsonRejection>,
) -> Response {
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
fn session(headers: &HeaderMap) -> Result<&str, Error> {
    header(headers, "x-diary-session")
        .filter(|id| Uuid::parse_str(id).is_ok())
        .ok_or(Error::SessionExpired)
}
async fn heartbeat(
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
async fn close_session(
    State(s): State<Arc<ServerState>>,
    Extension(trace): Extension<TraceId>,
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
fn availability_dto(items: &[diary_application::Availability]) -> Vec<Value> {
    use diary_application::UnavailableReason as Reason;
    items.iter().map(|item|json!({"alias":item.alias,"available":item.reason.is_none(),"reason":match item.reason {
        None=>"ready",Some(Reason::SourceRootMissing)=>"source_root_missing",Some(Reason::SourceRootDenied)=>"source_root_denied",Some(Reason::ProfileHomeMissing)=>"profile_home_missing",Some(Reason::ProfileHomeDenied)=>"profile_home_denied",Some(Reason::ExecutableMissing)=>"executable_missing",Some(Reason::AgentAdapterUnverified)=>"agent_adapter_unverified",Some(Reason::NativeAdapterUnverified)=>"native_adapter_unverified",
    }})).collect()
}
async fn settings(State(s): State<Arc<ServerState>>, headers: HeaderMap) -> Response {
    let id = match session(&headers) {
        Ok(id) => id,
        Err(e) => return app_error(e),
    };
    match s
        .coordinator
        .lock()
        .expect("coordinator poisoned")
        .list(id, s.now(), None, 1)
    {
        Ok(_) => {
            let startup = s.startup.lock().expect("startup state poisoned").clone();
            let value = json!({"config_schema_version":1,"config_generation":s.config_generation.load(std::sync::atomic::Ordering::SeqCst),"last_reload":s.last_reload.lock().expect("reload state poisoned").clone(),"storage_ready":s.storage_ready,"sources_available":startup.sources.iter().any(|s|s.reason.is_none()),"agents_available":false,"exports_available":false,"sources":availability_dto(&startup.sources),"profiles":availability_dto(&startup.profiles)});
            #[cfg(feature = "scaffold-smoke")]
            let value = {
                let mut value = value;
                value["folder_checks"] =
                    serde_json::to_value(&s.folder_checks).expect("fixture DTO");
                value
            };
            Json(value).into_response()
        }
        Err(e) => app_error(e),
    }
}
/// Validates a complete new config first; any failure leaves the last one in force.
async fn reload_settings(State(s): State<Arc<ServerState>>, headers: HeaderMap) -> Response {
    let id = match session(&headers) {
        Ok(id) => id,
        Err(e) => return app_error(e),
    };
    let busy =
        match s
            .coordinator
            .lock()
            .expect("coordinator poisoned")
            .list(id, s.now(), None, 200)
        {
            Ok(jobs) => jobs.iter().any(|job| !job.state.is_terminal()),
            Err(e) => return app_error(e),
        };
    let Some(reloader) = s.reloader.clone() else {
        return failure(StatusCode::SERVICE_UNAVAILABLE, "unavailable");
    };
    let issues = |list: &[diary_application::ReloadIssue]| -> Vec<Value> {
        list.iter()
            .map(|i| json!({"path":i.path,"code":i.code}))
            .collect()
    };
    let result = match tokio::task::spawn_blocking(move || reloader.reload(busy)).await {
        Ok(diary_application::ReloadOutcome::Applied(summary)) => {
            *s.startup.lock().expect("startup state poisoned") = summary;
            let generation = s
                .config_generation
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                + 1;
            json!({"outcome":"applied","config_generation":generation})
        }
        Ok(diary_application::ReloadOutcome::RestartRequired(list)) => {
            json!({"outcome":"restart_required","issues":issues(&list)})
        }
        Ok(diary_application::ReloadOutcome::Rejected(list)) => {
            json!({"outcome":"rejected","issues":issues(&list)})
        }
        Ok(diary_application::ReloadOutcome::Busy) => json!({"outcome":"busy"}),
        Err(_) => return failure(StatusCode::SERVICE_UNAVAILABLE, "unavailable"),
    };
    *s.last_reload.lock().expect("reload state poisoned") = result.clone();
    Json(result).into_response()
}
#[derive(Serialize)]
struct JobDto {
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
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct JobQuery {
    cursor: Option<String>,
}
async fn list_jobs(
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
            Json(json!({"jobs":jobs.into_iter().map(JobDto::from).collect::<Vec<_>>(),"next_cursor":next,"event_cursor":coordinator.event_cursor()})).into_response()
        }
        Err(e) => app_error(e),
    }
}
async fn get_job(
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
            let mut value = serde_json::to_value(JobDto::from(job)).expect("fixed DTO");
            if let Some(code) = code {
                value["error_code"] = json!(code);
            }
            Json(value).into_response()
        }
        Err(e) => app_error(e),
    }
}
async fn cancel_job(
    State(s): State<Arc<ServerState>>,
    Extension(trace): Extension<TraceId>,
    Path(job): Path<String>,
    headers: HeaderMap,
) -> Response {
    let id = match session(&headers) {
        Ok(id) => id,
        Err(e) => return app_error(e),
    };
    let mut c = s.coordinator.lock().expect("coordinator poisoned");
    let was_terminal = c
        .get(id, s.now(), &job)
        .is_ok_and(|j| j.state.is_terminal());
    match c.cancel(id, s.now(), &job) {
        Ok(job) => {
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
async fn events(State(s): State<Arc<ServerState>>, headers: HeaderMap) -> Response {
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
    let start = s.authentic_epoch.load(std::sync::atomic::Ordering::SeqCst);
    let stream = futures_util::stream::unfold(
        (s, id, instance, after, false, permit, start),
        |(s, id, instance, mut after, done, permit, mut seen)| async move {
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
                        (s, id, instance, after, false, permit, seen),
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
                            (s, id, instance, after, true, permit, seen),
                        ));
                    }
                    Ok(Replay::Events(events)) => {
                        if let Some(event) = events.first() {
                            after = event.event_id;
                            let data = json!({"instance_id":event.instance_id,"session_id":event.session_id,"event_id":event.event_id,"job_id":event.job.id,"job_sequence":event.job.sequence,"kind":"job_changed","payload":JobDto::from(event.job.clone())});
                            let item = SseEvent::default()
                                .id(after.to_string())
                                .event("job_changed")
                                .json_data(data)
                                .expect("fixed DTO");
                            return Some((Ok(item), (s, id, instance, after, false, permit, seen)));
                        }
                    }
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        },
    );
    Sse::new(stream)
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(10)))
        .into_response()
}

#[cfg(test)]
mod tests;

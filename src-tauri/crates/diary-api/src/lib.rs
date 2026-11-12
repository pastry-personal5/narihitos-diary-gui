//! Explicit authenticated scaffold transport; no content mutation or fake admission route.
// shortcut: every route helper returns `Result<_, axum::Response>` (about 128 bytes); boxing it
// touches ~100 call sites for no behavior gain, so the lint is allowed once here. Upgrade by
// introducing a boxed `Rejection` type if a helper's error path ever becomes hot.
#![allow(clippy::result_large_err)]
mod authentic;
mod calendar;
mod chats;
mod exports;
mod generations;
mod guard;
mod images;
mod imports;
mod jobs;
mod profiles;
mod reviews;
mod sessions;
mod settings;
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
use guard::*;
use jobs::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sessions::*;
use settings::*;
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
    configuration_folder: Option<std::path::PathBuf>,
    reloader: Option<Arc<dyn diary_application::ConfigReloader>>,
    config_generation: std::sync::atomic::AtomicU64,
    last_reload: Mutex<Value>,
    storage_ready: bool,
    authentic: Option<Arc<dyn diary_application::authentic::AuthenticStore>>,
    imports: Option<Arc<dyn diary_application::imports::ImportStore>>,
    source_writer: Option<Arc<dyn diary_application::source::SourceWriter>>,
    source_trash: Option<Arc<dyn diary_application::source_trash::SourceTrash>>,
    exports: Option<Arc<dyn diary_application::export::Exports>>,
    export_jobs: Mutex<std::collections::BTreeMap<String, (String, u64, &'static str)>>,
    images: Arc<diary_application::image::Registry>,
    profiles: Option<Arc<dyn diary_application::agent::ProfileDirectory>>,
    agent_runner: Option<Arc<dyn diary_application::agent::AgentRunner + Send + Sync>>,
    conversations: diary_application::chat::conversations::Conversations,
    generations: Mutex<diary_application::generation::Store>,
    reviews: diary_application::review::Reviews,
    review_hasher: Option<Arc<dyn diary_domain::review::Hasher>>,
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
            configuration_folder: None,
            reloader: None,
            config_generation: std::sync::atomic::AtomicU64::new(1),
            last_reload: Mutex::new(Value::Null),
            storage_ready: false,
            authentic: None,
            imports: None,
            source_writer: None,
            source_trash: None,
            exports: None,
            export_jobs: Mutex::new(std::collections::BTreeMap::new()),
            images: Arc::new(Default::default()),
            profiles: None,
            agent_runner: None,
            conversations: Default::default(),
            generations: Mutex::new(Default::default()),
            reviews: Default::default(),
            review_hasher: None,
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
    /// Enables paragraph review; digests come from infrastructure (SHA-256).
    pub fn set_review_hasher(state: &mut Arc<Self>, hasher: Arc<dyn diary_domain::review::Hasher>) {
        Arc::get_mut(state)
            .expect("startup precedes shared use")
            .review_hasher = Some(hasher);
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
    pub fn set_exports(
        state: &mut Arc<Self>,
        service: Arc<dyn diary_application::export::Exports>,
    ) {
        Arc::get_mut(state)
            .expect("startup precedes shared use")
            .exports = Some(service);
    }
    /// Call last: the notifier holds a weak reference, which forbids further `set_*` calls.
    pub fn wire_exports(state: &Arc<Self>) {
        let Some(service) = &state.exports else {
            return;
        };
        let weak = Arc::downgrade(state);
        service.set_notifier(Arc::new(move |r| {
            let Some(s) = weak.upgrade() else { return };
            let session = s
                .export_jobs
                .lock()
                .expect("export jobs poisoned")
                .get(r.operation_id.as_str())
                .map(|m| m.0.clone());
            if let Some(session) = session {
                exports::sync(&s, &session, r);
            }
        }));
    }
    /// Called when the client lease ends (expiry, close or shutdown): agent runs belong to the lease.
    pub fn set_lease_ended(state: &mut Arc<Self>, hook: Arc<dyn Fn() + Send + Sync>) {
        Arc::get_mut(state)
            .expect("startup precedes shared use")
            .lease_ended = Some(hook);
    }
    fn lease_ended(&self) {
        // The chat and its transcript belong to the lease; a running turn is cancelled first.
        self.conversations.clear();
        self.reviews.clear();
        self.generations
            .lock()
            .expect("generations poisoned")
            .clear();
        if let Some(hook) = &self.lease_ended {
            hook();
        }
    }
    /// Serialized with admission: expiry discovered by any coordinator read must also erase
    /// content, and a new lease cannot start while the old runs receive cancellation.
    fn expire_leases(&self, now: u64) {
        let mut c = self.coordinator.lock().expect("coordinator poisoned");
        let expired = c.expire(now);
        let orphaned = !c.has_session()
            && (!self.conversations.is_empty()
                || !self
                    .generations
                    .lock()
                    .expect("generations poisoned")
                    .is_empty());
        if expired || orphaned {
            self.lease_ended();
        }
    }
    /// Enables chat turns. Without it `POST /chats` answers `unavailable`.
    pub fn set_agent_runner(
        state: &mut Arc<Self>,
        runner: Arc<dyn diary_application::agent::AgentRunner + Send + Sync>,
    ) {
        Arc::get_mut(state)
            .expect("startup precedes shared use")
            .agent_runner = Some(runner);
    }
    pub fn set_images(state: &mut Arc<Self>, images: Arc<diary_application::image::Registry>) {
        Arc::get_mut(state)
            .expect("state shared before composition")
            .images = images;
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
    /// Records the active configuration folder for the native Finder action.
    pub fn set_configuration_folder(state: &mut Arc<Self>, folder: std::path::PathBuf) {
        Arc::get_mut(state)
            .expect("startup precedes shared use")
            .configuration_folder = Some(folder);
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
        .route(
            "/api/v1/settings/configuration-folder",
            get(configuration_folder),
        )
        .route("/api/v1/settings/reload", post(reload_settings))
        .route("/api/v1/jobs", get(list_jobs))
        .route("/api/v1/jobs/{id}", get(get_job))
        .route("/api/v1/jobs/{id}/cancel", post(cancel_job))
        .route("/api/v1/events", get(events))
        .route(
            "/api/v1/imports",
            post(imports::create).layer(DefaultBodyLimit::max(4096)),
        )
        .route(
            "/api/v1/chats",
            post(chats::open).layer(DefaultBodyLimit::max(4096)),
        )
        .route(
            "/api/v1/chats/{id}/turns",
            post(chats::turn).layer(DefaultBodyLimit::max(chats::BODY_LIMIT)),
        )
        .route("/api/v1/chats/{id}", get(chats::get).delete(chats::close))
        .route(
            "/api/v1/generations",
            post(generations::start).layer(DefaultBodyLimit::max(256 * 1024)),
        )
        .route(
            "/api/v1/generations/{id}",
            get(generations::get).delete(generations::discard),
        )
        .route(
            "/api/v1/jobs/{id}/result",
            get(generations::result).delete(generations::discard),
        )
        .route("/api/v1/candidates/{id}", get(generations::candidate))
        .route(
            "/api/v1/reviews",
            post(reviews::start).layer(DefaultBodyLimit::max(reviews::BODY_LIMIT)),
        )
        .route(
            "/api/v1/reviews/{id}/decisions",
            post(reviews::decide).layer(DefaultBodyLimit::max(reviews::BODY_LIMIT)),
        )
        .route("/api/v1/reviews/{id}", delete(reviews::end))
        .route("/api/v1/image-engines", get(images::list))
        .route("/api/v1/image-engines/{id}/check", post(images::check))
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
            "/api/v1/exports",
            post(exports::start).layer(DefaultBodyLimit::max(4096)),
        )
        .route(
            "/api/v1/exports/{id}/confirmation",
            post(exports::confirm).layer(DefaultBodyLimit::max(4096)),
        )
        .route(
            "/api/v1/exports/{id}/abandon",
            post(exports::abandon).layer(DefaultBodyLimit::max(4096)),
        )
        .route(
            "/api/v1/exports/{id}/abandon/confirmation",
            post(exports::abandon_confirm).layer(DefaultBodyLimit::max(4096)),
        )
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

#[cfg(test)]
mod tests;

#[cfg(test)]
mod test_prompts;

mod authentic;
mod calendar;
mod chat;
mod export;
mod generation;
mod imports;
mod profiles;
mod progress;
mod review;
mod source_trash;
use crate::delivery::{Delivery, Flow};
use crate::diagnostics::ClientLog;
use crate::discovery::{self, Discovery};
use crate::events::{BridgeEvent, Cursor, Decoder};
pub use authentic::{
    AuthenticError, AuthenticFile, AuthenticList, OperationState, RefreshReport, Revision, SaveAck,
};
pub use calendar::{Calendar, CalendarAction};
pub use chat::{ChatDocument, ChatOpened, ChatView, TurnAccepted};
use diary_application::diagnostics::{Component, ErrorTag, EventCode, Level, Record, TraceId};
use diary_logging::private_fs::PrivateDir;
pub use export::{
    ConfirmExportRequest, ExportAcknowledged, ExportPage, ExportResult, StartExportRequest,
    StartExportResult,
};
pub use generation::{CandidateView, GenerationInput, GenerationView};
pub use imports::{ImportAck, ImportPage, ResolutionAck, SourceBody, SourceSaved, TimeAuthority};
pub use profiles::{ProfileList, ProfileStatus};
use reqwest::{Client, Method};
pub use review::{DecideReviewInput, ReviewDecision, ReviewView, StartReviewInput};
use serde::{Deserialize, Serialize};
pub use source_trash::{
    TrashAcknowledged, TrashConfirmRequest, TrashPage, TrashPreview, TrashPreviewRequest,
};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Serialize, Deserialize, Clone)]
pub struct Status {
    pub instance_id: String,
    pub workspace_key: String,
    pub server_version: String,
    pub api_version: u32,
    pub api_minor: u32,
    pub storage_ready: bool,
    pub capabilities: Vec<String>,
}
#[derive(Serialize, Deserialize)]
pub struct Lease {
    pub session_id: String,
    pub heartbeat_interval_ms: u64,
    pub lease_duration_ms: u64,
}
#[derive(Serialize, Deserialize)]
pub struct Settings {
    pub config_schema_version: u32,
    #[serde(default)]
    pub config_generation: u64,
    #[serde(default)]
    pub last_reload: Option<ReloadResult>,
    pub storage_ready: bool,
    pub sources_available: bool,
    pub agents_available: bool,
    pub exports_available: bool,
    #[serde(default)]
    pub notion_exports_available: bool,
    pub sources: Vec<Availability>,
    pub profiles: Vec<Availability>,
    #[cfg(feature = "scaffold-smoke")]
    pub folder_checks: Vec<crate::native_smoke::FolderCheck>,
}
/// A redacted reload problem: a coarse config area and a stable code, never a value.
#[derive(Serialize, Deserialize)]
pub struct ReloadIssue {
    pub path: String,
    pub code: String,
}
#[derive(Serialize, Deserialize)]
pub struct ReloadResult {
    pub outcome: String,
    #[serde(default)]
    pub config_generation: Option<u64>,
    #[serde(default)]
    pub issues: Vec<ReloadIssue>,
}
#[derive(Serialize, Deserialize)]
pub struct Availability {
    pub alias: String,
    pub available: bool,
    pub reason: UnavailableReason,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnavailableReason {
    /// Available: the wire still carries a reason string, and `available` is true.
    Ready,
    SourceRootMissing,
    SourceRootDenied,
    ProfileHomeMissing,
    ProfileHomeDenied,
    ExecutableMissing,
    AgentAdapterUnverified,
    NativeAdapterUnverified,
}
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}
#[derive(Serialize, Deserialize, Clone)]
#[serde(try_from = "progress::WireJob")]
pub struct Job {
    pub id: String,
    pub sequence: u64,
    pub state: JobState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(deserialize_with = "job_error")]
    pub error_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<progress::AiProgress>,
}
fn job_error<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    let code = Option::<String>::deserialize(d)?;
    if let Some(c) = &code
        && diary_application::agent::AgentErrorCode::parse(c).is_none()
        && serde_json::from_value::<imports::ImportFailure>(serde_json::json!(c)).is_err()
    {
        return Err(serde::de::Error::custom("invalid job error"));
    }
    Ok(code)
}
#[derive(Serialize, Deserialize)]
pub struct JobList {
    pub jobs: Vec<Job>,
    pub next_cursor: Option<String>,
    pub event_cursor: u64,
}
impl JobList {
    fn validate(
        &self,
        cursor: Option<&str>,
        scope: Option<(&str, &str)>,
    ) -> Result<(), &'static str> {
        let unique = self
            .jobs
            .iter()
            .map(|j| &j.id)
            .collect::<std::collections::HashSet<_>>();
        if self.jobs.len() > 200
            || unique.len() != self.jobs.len()
            || self
                .next_cursor
                .as_deref()
                .is_some_and(|id| !valid_id(id) || cursor.is_some_and(|old| id <= old))
            || self.jobs.iter().any(|job| {
                !valid_id(&job.id)
                    || job.sequence == 0
                    || scope.is_some_and(|(instance, session)| !job.owned_by(instance, session))
            })
        {
            return Err("invalid_snapshot");
        }
        Ok(())
    }
}
impl Job {
    fn owned_by(&self, instance: &str, session: &str) -> bool {
        match &self.progress {
            None => true,
            Some(progress::AiProgress::Chat(p)) => {
                p.instance_id == instance && p.session_id == session
            }
            Some(progress::AiProgress::Generation(p)) => {
                p.instance_id == instance && p.session_id == session
            }
        }
    }
}
#[derive(Serialize)]
pub struct Metadata {
    pub version: String,
}

pub struct Connection {
    discovery: Option<Discovery>,
    session: Option<String>,
    cursor: Arc<std::sync::Mutex<Cursor>>,
}
pub struct Bridge {
    path: PathBuf,
    lifecycle: Mutex<()>,
    flow: std::sync::Mutex<Option<Arc<Flow>>>,
    client: Client,
    connection: Mutex<Connection>,
    subscription: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
    log: Arc<ClientLog>,
}
/// Unexpected statuses only: a 409 is an expected conflict (busy, stale) that the caller handles.
fn status_tag(status: reqwest::StatusCode) -> Option<ErrorTag> {
    use reqwest::StatusCode as S;
    match status {
        S::CONFLICT => None,
        S::UNAUTHORIZED | S::FORBIDDEN => Some(ErrorTag::Denied),
        S::TOO_MANY_REQUESTS => Some(ErrorTag::Limit),
        S::SERVICE_UNAVAILABLE => Some(ErrorTag::Unavailable),
        s if s.is_server_error() => Some(ErrorTag::Internal),
        _ => Some(ErrorTag::Invalid),
    }
}
/// One fresh ID per outgoing request; the server echoes it in its own records.
fn new_trace() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}
impl Bridge {
    pub fn log(&self) -> Arc<ClientLog> {
        self.log.clone()
    }
    fn note(&self, trace: &str, tag: ErrorTag) {
        let mut record = Record::new(EventCode::UiBridgeError, Level::Warn)
            .component(Component::Bridge)
            .error(tag);
        if let Some(trace) = TraceId::parse(trace) {
            record = record.trace(trace);
        }
        self.log.log(record);
    }
    /// The discovery file's parent is the validated per-workspace runtime directory.
    fn attach_log(&self) {
        if let Some(dir) = self.path.parent().and_then(|p| PrivateDir::open(p).ok()) {
            self.log.attach(&dir);
        }
    }
    pub fn new(path: PathBuf) -> Result<Self, &'static str> {
        let client = Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .map_err(|_| "transport_unavailable")?;
        Ok(Self {
            path,
            lifecycle: Mutex::new(()),
            flow: std::sync::Mutex::new(None),
            client,
            subscription: Mutex::new(None),
            log: ClientLog::new(),
            connection: Mutex::new(Connection {
                discovery: None,
                session: None,
                cursor: Arc::new(std::sync::Mutex::new(Cursor::default())),
            }),
        })
    }
    async fn request<T: serde::de::DeserializeOwned>(
        &self,
        c: &Connection,
        method: Method,
        path: &str,
        payload: Option<serde_json::Value>,
    ) -> Result<T, &'static str> {
        let d = c.discovery.as_ref().ok_or("disconnected")?;
        let trace = new_trace();
        let mut request = self
            .client
            .request(method, format!("{}/api/v1{path}", d.origin))
            .bearer_auth(&d.token)
            .header("x-diary-trace-id", &trace);
        if let Some(id) = &c.session {
            request = request.header("x-diary-session", id);
        }
        if let Some(payload) = payload {
            request = request.json(&payload);
        }
        if path.ends_with("/check") {
            // A profile check runs two probes, each bounded by the server at about 5 s plus grace.
            request = request.timeout(std::time::Duration::from_secs(20));
        }
        let mut response = request.send().await.map_err(|_| {
            self.note(&trace, ErrorTag::Unavailable);
            "server_unavailable"
        })?;
        if !response.status().is_success() {
            if let Some(tag) = status_tag(response.status()) {
                self.note(&trace, tag);
            }
            if response.status() == reqwest::StatusCode::CONFLICT {
                let mut bytes = Vec::new();
                while let Some(chunk) = response
                    .chunk()
                    .await
                    .map_err(|_| "server_rejected_request")?
                {
                    if bytes.len() + chunk.len() > 1024 {
                        return Err("server_rejected_request");
                    }
                    bytes.extend_from_slice(&chunk);
                }
                if serde_json::from_slice::<serde_json::Value>(&bytes)
                    .ok()
                    .is_some_and(|value| value["error"]["code"] == "client_busy")
                {
                    // The one expected conflict worth a client record: it ties a rejected second
                    // client to the server's `session.client_busy` record by trace ID.
                    self.note(&trace, ErrorTag::Busy);
                    return Err("client_busy");
                }
            }
            return Err("server_rejected_request");
        }
        if response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split(';').next())
            != Some("application/json")
        {
            return Err("invalid_response_type");
        }
        if response
            .content_length()
            .is_some_and(|bytes| bytes > 10 * 1024 * 1024)
        {
            return Err("response_limit");
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| "invalid_response")? {
            if bytes.len().saturating_add(chunk.len()) > 10 * 1024 * 1024 {
                return Err("response_limit");
            }
            bytes.extend_from_slice(&chunk);
        }
        serde_json::from_slice(&bytes).map_err(|_| "invalid_response")
    }
    pub async fn connect(&self) -> Result<Status, &'static str> {
        let _lifecycle = self.lifecycle.lock().await;
        self.stop_events().await;
        let record = match discovery::read(&self.path) {
            Ok(record) => record,
            Err(error) => return Err(error),
        };
        let mut c = self.connection.lock().await;
        let same = c.discovery.as_ref().is_some_and(|old| {
            old.instance_id == record.instance_id && old.workspace_key == record.workspace_key
        });
        if !same {
            c.cursor = Arc::new(std::sync::Mutex::new(Cursor::default()));
            c.session = None;
        }
        c.discovery = Some(record);
        let result = self
            .request::<Status>(&c, Method::GET, "/status", None)
            .await;
        let status = match result {
            Ok(status) => status,
            Err(e) => return Err(e),
        };
        let d = c.discovery.as_ref().ok_or("disconnected")?;
        if status.instance_id != d.instance_id
            || status.workspace_key != d.workspace_key
            || status.api_version != 1
            || status.server_version.len() > 32
            || status.capabilities.len() > 64
            || status
                .capabilities
                .iter()
                .any(|name| name.is_empty() || name.len() > 64)
            || !["status", "sessions", "jobs", "events"]
                .iter()
                .all(|required| status.capabilities.iter().any(|v| v == required))
        {
            c.discovery = None;
            c.session = None;
            return Err("incompatible_server");
        }
        self.attach_log();
        Ok(status)
    }
    pub async fn status(&self) -> Result<Status, &'static str> {
        self.request(&*self.connection.lock().await, Method::GET, "/status", None)
            .await
    }
    pub async fn open(&self) -> Result<Lease, &'static str> {
        let _lifecycle = self.lifecycle.lock().await;
        let resuming = self.connection.lock().await.session.is_some();
        if resuming {
            match self.heartbeat().await {
                Ok(()) | Err("session_expired") => {}
                Err(error) => return Err(error),
            }
        }
        let mut c = self.connection.lock().await;
        if c.session.is_none() {
            c.session = Some(uuid::Uuid::new_v4().to_string());
        }
        let lease: Lease = self
            .request(
                &c,
                Method::POST,
                "/sessions",
                Some(serde_json::json!({"session_id":c.session})),
            )
            .await?;
        if uuid::Uuid::parse_str(&lease.session_id).is_err()
            || lease.heartbeat_interval_ms != 5000
            || lease.lease_duration_ms != 30000
        {
            return Err("invalid_response");
        }
        c.session = Some(lease.session_id.clone());
        Ok(lease)
    }
    pub async fn heartbeat(&self) -> Result<(), &'static str> {
        self.session_action(false).await
    }
    pub async fn close(&self) -> Result<(), &'static str> {
        let _lifecycle = self.lifecycle.lock().await;
        // With no lease there is no client run to cancel (including closing before connection).
        if self.connection.lock().await.session.is_none() {
            return Ok(());
        }
        self.session_action(true).await?;
        self.stop_events().await;
        Ok(())
    }
    pub fn acknowledge(&self, id: &str, delivery: u64) -> Result<(), &'static str> {
        match self.flow.lock().expect("flow poisoned").as_ref() {
            Some(flow) => flow.acknowledge(id, delivery),
            None => Ok(()),
        }
    }
    pub async fn detach_events(&self) {
        let _lifecycle = self.lifecycle.lock().await;
        self.stop_events().await;
    }
    pub async fn stop_events(&self) {
        if let Some(flow) = self.flow.lock().expect("flow poisoned").take() {
            flow.stop();
        }
        if let Some(task) = self.subscription.lock().await.take() {
            task.abort();
        }
    }
    pub async fn subscribe(
        &self,
        channel: tauri::ipc::Channel<Delivery>,
    ) -> Result<(), &'static str> {
        let _lifecycle = self.lifecycle.lock().await;
        self.stop_events().await;
        let flow = Flow::new();
        *self.flow.lock().expect("flow poisoned") = Some(flow.clone());
        let c = self.connection.lock().await;
        let d = c.discovery.as_ref().ok_or("disconnected")?;
        let session = c.session.clone().ok_or("session_unavailable")?;
        let instance = d.instance_id.clone();
        let cursor = c.cursor.clone();
        // Establish a complete bounded paginated baseline before resuming hints.
        // Use the first page's watermark: events racing later pages are replayed.
        let mut page_cursor: Option<String> = None;
        let mut watermark = None;
        let mut pages = 0;
        let mut seen = std::collections::HashSet::new();
        loop {
            let path = page_cursor
                .as_ref()
                .map_or_else(|| "/jobs".into(), |id| format!("/jobs?cursor={id}"));
            let page: JobList = self.request(&c, Method::GET, &path, None).await?;
            page.validate(page_cursor.as_deref(), Some((&instance, &session)))?;
            if page.jobs.iter().any(|job| !seen.insert(job.id.clone())) {
                return Err("invalid_snapshot");
            }
            let first = watermark.is_none();
            watermark.get_or_insert(page.event_cursor);
            page_cursor = page.next_cursor;
            flow.send(
                &channel,
                BridgeEvent::JobSnapshot {
                    instance_id: instance.clone(),
                    session_id: session.clone(),
                    jobs: page.jobs,
                    first,
                    complete: false,
                },
            )
            .await?;
            pages += 1;
            if page_cursor.is_none() {
                break;
            }
            if pages >= 1024 {
                return Err("snapshot_limit");
            }
        }
        let last = watermark.unwrap_or(0);
        *cursor.lock().expect("cursor poisoned") = Cursor::at(last);
        // Streaming has an idle deadline, never the ordinary 5-second total deadline.
        let client = Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(std::time::Duration::from_secs(5))
            .read_timeout(std::time::Duration::from_secs(15))
            .build()
            .map_err(|_| "transport_unavailable")?;
        let response = client
            .get(format!("{}/api/v1/events", d.origin))
            .bearer_auth(&d.token)
            .header("x-diary-session", &session)
            .header("x-diary-trace-id", new_trace())
            .header("x-diary-instance", &instance)
            .header("last-event-id", last.to_string())
            .header("x-diary-reconcile", "1")
            .send()
            .await
            .map_err(|_| "server_unavailable")?;
        if !response.status().is_success()
            || response
                .headers()
                .get("x-diary-reconcile")
                .and_then(|v| v.to_str().ok())
                != Some("1")
            || response
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .is_none_or(|v| v.split(';').next() != Some("text/event-stream"))
        {
            return Err("invalid_event_stream");
        }
        let snapshot_client = self.client.clone();
        let origin = d.origin.clone();
        let token = d.token.clone();
        let task = tauri::async_runtime::spawn(async move {
            let mut ready = false;
            let mut dirty = std::collections::HashSet::<String>::new();
            let mut response = response;
            let mut decoder = Decoder::default();
            let outcome: Result<(), &'static str> = async {
                while let Some(chunk) = response.chunk().await.map_err(|_| "stream_disconnected")? {
                    for frame in decoder.feed(&chunk)? {
                        let event = cursor
                            .lock()
                            .expect("cursor poisoned")
                            .decode(&frame, &instance, &session)?;
                        if let Some(event) = event {
                            if matches!(event, BridgeEvent::ReplayComplete) {
                                if ready {
                                    return Err("invalid_event");
                                }
                                for id in dirty.drain() {
                                    let job = progress::reconcile_job(
                                        &snapshot_client,
                                        &origin,
                                        &token,
                                        &instance,
                                        &session,
                                        &id,
                                    )
                                    .await?;
                                    flow.send(
                                        &channel,
                                        BridgeEvent::JobSnapshot {
                                            instance_id: instance.clone(),
                                            session_id: session.clone(),
                                            jobs: vec![job],
                                            first: false,
                                            complete: false,
                                        },
                                    )
                                    .await?;
                                }
                                flow.send(
                                    &channel,
                                    BridgeEvent::JobSnapshot {
                                        instance_id: instance.clone(),
                                        session_id: session.clone(),
                                        jobs: vec![],
                                        first: false,
                                        complete: true,
                                    },
                                )
                                .await?;
                                ready = true;
                                continue;
                            }
                            if !ready
                                && let BridgeEvent::JobChanged { job_id, .. }
                                | BridgeEvent::CandidateReady { job_id, .. } = &event
                            {
                                if dirty.len() >= 4096 && !dirty.contains(job_id) {
                                    return Err("snapshot_limit");
                                }
                                dirty.insert(job_id.clone());
                            }
                            let resync = matches!(event, BridgeEvent::ResyncRequired { .. });
                            flow.send(&channel, event).await?;
                            if resync {
                                return Ok(());
                            }
                        }
                    }
                }
                Err("stream_disconnected")
            }
            .await;
            if outcome.is_err() {
                flow.disconnected(&channel);
            }
        });
        *self.subscription.lock().await = Some(task);
        Ok(())
    }
    async fn session_action(&self, close: bool) -> Result<(), &'static str> {
        let mut c = self.connection.lock().await;
        let id = c.session.as_ref().ok_or("session_unavailable")?;
        let d = c.discovery.as_ref().ok_or("disconnected")?;
        let path = if close {
            format!("/sessions/{id}")
        } else {
            format!("/sessions/{id}/heartbeat")
        };
        let response = self
            .client
            .request(
                if close { Method::DELETE } else { Method::POST },
                format!("{}/api/v1{path}", d.origin),
            )
            .bearer_auth(&d.token)
            .header("x-diary-session", id)
            .header("x-diary-trace-id", new_trace())
            .send()
            .await
            .map_err(|_| "server_unavailable")?;
        if !close && response.status() == reqwest::StatusCode::UNAUTHORIZED {
            c.session = None;
            return Err("session_expired");
        }
        if !response.status().is_success()
            && !(close && response.status() == reqwest::StatusCode::UNAUTHORIZED)
        {
            return Err("server_rejected_request");
        }
        if close {
            c.session = None;
        }
        Ok(())
    }
    pub async fn settings(&self) -> Result<Settings, &'static str> {
        self.request(
            &*self.connection.lock().await,
            Method::GET,
            "/settings",
            None,
        )
        .await
    }
    pub async fn reload_settings(&self) -> Result<ReloadResult, &'static str> {
        let result: ReloadResult = self
            .request(
                &*self.connection.lock().await,
                Method::POST,
                "/settings/reload",
                None,
            )
            .await?;
        let known = ["applied", "restart_required", "rejected", "busy"];
        let bounded = |t: &str| t.len() <= 64 && t.bytes().all(|b| b.is_ascii_graphic());
        if !known.contains(&result.outcome.as_str())
            || result.issues.len() > 16
            || result
                .issues
                .iter()
                .any(|i| !bounded(&i.path) || !bounded(&i.code))
        {
            return Err("unavailable");
        }
        Ok(result)
    }
    pub async fn jobs(&self, cursor: Option<String>) -> Result<JobList, &'static str> {
        if cursor.as_ref().is_some_and(|id| !valid_id(id)) {
            return Err("invalid_cursor");
        }
        let path = cursor
            .as_ref()
            .map_or_else(|| "/jobs".into(), |id| format!("/jobs?cursor={id}"));
        let c = self.connection.lock().await;
        let page: JobList = self.request(&c, Method::GET, &path, None).await?;
        let scope = c
            .discovery
            .as_ref()
            .zip(c.session.as_ref())
            .map(|(d, session)| (d.instance_id.as_str(), session.as_str()));
        page.validate(cursor.as_deref(), scope)?;
        Ok(page)
    }

    pub async fn job(&self, id: String, cancel: bool) -> Result<Job, &'static str> {
        if !valid_id(&id) {
            return Err("invalid_job_id");
        }
        let path = if cancel {
            format!("/jobs/{id}/cancel")
        } else {
            format!("/jobs/{id}")
        };
        let c = self.connection.lock().await;
        let job: Job = self
            .request(
                &c,
                if cancel { Method::POST } else { Method::GET },
                &path,
                None,
            )
            .await?;
        if job.id != id
            || c.discovery
                .as_ref()
                .zip(c.session.as_ref())
                .is_some_and(|(d, session)| !job.owned_by(&d.instance_id, session))
        {
            return Err("invalid_response");
        }
        Ok(job)
    }
}
fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

#[cfg(test)]
mod tests;

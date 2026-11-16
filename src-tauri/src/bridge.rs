mod authentic;
mod calendar;
mod chat;
mod export;
mod generation;
mod illustrated;
mod images;
pub use images::{ImageEngine, ImageEngines};
mod imports;
pub use illustrated::{IllustratedEntry, IllustratedInput, IllustratedList};
mod profiles;
mod progress;
mod review;
mod source_trash;
mod stream;
mod types;
pub use types::*;
mod validate;
use crate::delivery::{Delivery, Flow};
use crate::diagnostics::ClientLog;
use crate::discovery::{self, Discovery};
use crate::events::{BridgeEvent, Cursor, Decoder};
pub use authentic::{
    AuthenticError, AuthenticFile, AuthenticList, OperationState, RefreshReport, Revision, SaveAck,
};
pub use calendar::{Calendar, CalendarAction};
pub use chat::{ChatDocument, ChatOpened, ChatView, TurnAccepted};
use diary_application::diagnostics::{
    Component, ErrorTag, EventCode, Fault, Level, Record, TraceId,
};
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
    /// Content-free connection-stage record: stage code, outcome level and the typed fault.
    fn stage(&self, code: EventCode, outcome: Result<(), &'static str>) {
        let record = match outcome {
            Ok(()) => Record::new(code, Level::Debug),
            Err(e) => Record::new(code, Level::Warn).fault(Fault::from_code(e)),
        };
        self.log.log(record.component(Component::Bridge));
    }
    pub async fn connect(&self) -> Result<Status, &'static str> {
        let _lifecycle = self.lifecycle.lock().await;
        self.stage(EventCode::BridgeConnectStarted, Ok(()));
        self.stop_events().await;
        let record = match discovery::read(&self.path) {
            Ok(record) => record,
            Err(error) => {
                self.stage(EventCode::BridgeDiscoveryRead, Err(error));
                return Err(error);
            }
        };
        self.stage(EventCode::BridgeDiscoveryRead, Ok(()));
        // The discovery file and its parent are validated: attach now so later failures reach the file.
        self.attach_log();
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
            Err(e) => {
                self.stage(EventCode::BridgeStatusProbe, Err(e));
                return Err(e);
            }
        };
        self.stage(EventCode::BridgeStatusProbe, Ok(()));
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
            self.stage(EventCode::BridgeConnected, Err("incompatible_server"));
            return Err("incompatible_server");
        }
        self.stage(EventCode::BridgeConnected, Ok(()));
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
    pub async fn reveal_configuration_folder(&self) -> Result<(), &'static str> {
        #[derive(Deserialize)]
        struct Folder {
            folder: PathBuf,
        }
        let value: Folder = self
            .request(
                &*self.connection.lock().await,
                Method::GET,
                "/settings/configuration-folder",
                None,
            )
            .await?;
        if !value.folder.is_absolute() || !value.folder.is_dir() {
            return Err("unavailable");
        }
        let status = std::process::Command::new("/usr/bin/open")
            .arg("-a")
            .arg("Finder")
            .arg("--")
            .arg(value.folder)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map_err(|_| "unavailable")?;
        if status.success() {
            Ok(())
        } else {
            Err("unavailable")
        }
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

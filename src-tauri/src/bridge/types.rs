//! Wire types for status, lease, settings, jobs and app metadata.
use super::*;

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
    #[serde(default)]
    pub illustrated_notion_exports_available: bool,
    pub sources: Vec<Availability>,
    pub profiles: Vec<Availability>,
    #[cfg(feature = "scaffold-smoke")]
    #[serde(default)]
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
pub(super) fn job_error<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<String>, D::Error> {
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
    pub(super) fn validate(
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
    pub(super) fn owned_by(&self, instance: &str, session: &str) -> bool {
        match &self.progress {
            None => true,
            Some(progress::AiProgress::Illustrated(p)) => {
                p.instance_id == instance && p.session_id == session
            }
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

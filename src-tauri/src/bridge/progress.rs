//! Closed, body-less progress projection. Ordinary jobs have no progress.
use super::{Job, JobState, generation, valid_id};
use diary_application::agent::{AgentErrorCode, Locale};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AiProgress {
    Generation(generation::Progress),
    Chat(ChatProgress),
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatProgress {
    pub kind: String,
    pub job_id: String,
    pub request_id: String,
    pub instance_id: String,
    pub session_id: String,
    pub chat_id: String,
    pub origin_date: Option<String>,
    pub profile_id: String,
    pub locale: String,
    pub phase: Option<String>,
    pub elapsed_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub silence_ms: Option<u64>,
    pub last_agent_activity_at: Option<u64>,
    pub cancellation_requested: bool,
    pub error_code: Option<String>,
}

#[derive(Deserialize)]
pub struct WireJob {
    id: String,
    sequence: u64,
    state: JobState,
    #[serde(default, deserialize_with = "super::job_error")]
    error_code: Option<String>,
    progress: Option<AiProgress>,
}

impl TryFrom<WireJob> for Job {
    type Error = &'static str;
    fn try_from(w: WireJob) -> Result<Self, Self::Error> {
        if !valid_id(&w.id) || w.sequence == 0 {
            return Err("invalid job identity");
        }
        let valid = match &w.progress {
            None => true,
            Some(AiProgress::Chat(p)) => {
                p.kind == "chat"
                    && p.job_id == w.id
                    && valid_id(&p.chat_id)
                    && p.origin_date
                        .as_deref()
                        .is_none_or(super::authentic::date_shape)
                    && common(
                        &p.request_id,
                        &p.instance_id,
                        &p.session_id,
                        &p.profile_id,
                        &p.locale,
                        p.phase.as_deref(),
                        p.error_code.as_deref(),
                    )
            }
            Some(AiProgress::Generation(p)) => {
                let n = u16::from(p.requested_count);
                p.kind == "generation"
                    && p.job_id == w.id
                    && (1..=5).contains(&n)
                    && super::authentic::date_shape(&p.origin_date)
                    && common(
                        &p.request_id,
                        &p.instance_id,
                        &p.session_id,
                        &p.profile_id,
                        &p.locale,
                        p.phase.as_deref(),
                        p.error_code.as_deref(),
                    )
                    && u16::from(p.finished_count)
                        == u16::from(p.ready_count)
                            + u16::from(p.failed_count)
                            + u16::from(p.cancelled_count)
                    && u16::from(p.finished_count) + u16::from(p.not_started_count) <= n
                    && p.candidate_index.is_none_or(|i| i > 0 && u16::from(i) <= n)
                    && p.candidate_ids.len() == usize::from(p.ready_count)
                    && p.candidate_ids
                        .iter()
                        .all(|id| (1..=n).any(|i| *id == format!("{}-{i}", w.id)))
                    && p.candidate_ids
                        .iter()
                        .collect::<std::collections::HashSet<_>>()
                        .len()
                        == p.candidate_ids.len()
                    && p.baseline_revision.as_ref().is_none_or(|r| {
                        r.sequence > 0
                            && r.sha256.len() == 64
                            && r.sha256.bytes().all(|b| b.is_ascii_hexdigit())
                    })
            }
        };
        if !valid {
            return Err("invalid job progress");
        }
        Ok(Job {
            id: w.id,
            sequence: w.sequence,
            state: w.state,
            error_code: w.error_code,
            progress: w.progress,
        })
    }
}

fn common(
    request: &str,
    instance: &str,
    session: &str,
    profile: &str,
    locale: &str,
    phase: Option<&str>,
    code: Option<&str>,
) -> bool {
    valid_id(request)
        && valid_id(instance)
        && valid_id(session)
        && super::profiles::valid_alias(profile)
        && Locale::parse(locale).is_ok()
        && phase.is_none_or(|p| super::chat::STAGES.contains(&p))
        && code.is_none_or(|c| AgentErrorCode::parse(c).is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn chat() -> serde_json::Value {
        json!({"id":"j","sequence":2,"state":"running","progress":{
            "kind":"chat","job_id":"j","request_id":"r","instance_id":"i","session_id":"s",
            "chat_id":"c","origin_date":null,"profile_id":"main","locale":"en-US",
            "phase":"waiting_for_output","elapsed_ms":100,"last_agent_activity_at":null,
            "cancellation_requested":false,"error_code":null}})
    }
    #[test]
    fn chat_progress_round_trips_without_text() {
        let value = chat();
        let job: Job = serde_json::from_value(value.clone()).unwrap();
        assert!(job.owned_by("i", "s"));
        assert!(!job.owned_by("i", "foreign"));
        assert_eq!(serde_json::to_value(job).unwrap(), value);
    }
    #[test]
    fn ordinary_jobs_remain_compatible() {
        let job: Job =
            serde_json::from_value(json!({"id":"j","sequence":1,"state":"queued"})).unwrap();
        assert!(job.progress.is_none());
    }
    #[test]
    fn invalid_progress_is_refused() {
        for (field, value) in [
            ("kind", json!("unknown")),
            ("phase", json!("thinking")),
            ("job_id", json!("other")),
            ("session_id", json!("../private")),
            ("locale", json!("")),
            ("provisional", json!("secret")),
        ] {
            let mut wire = chat();
            wire["progress"][field] = value;
            assert!(serde_json::from_value::<Job>(wire).is_err(), "{field}");
        }
    }
}

#[cfg(test)]
mod page_tests {
    use crate::bridge::JobList;
    use serde_json::json;
    #[test]
    fn page_bounds_duplicates_and_nonadvancing_cursor_refused() {
        let j = json!({"id":"j","sequence":1,"state":"queued"});
        for value in [
            json!({"jobs":[j.clone(),j.clone()],"next_cursor":null,"event_cursor":0}),
            json!({"jobs":vec![j.clone();201],"next_cursor":null,"event_cursor":0}),
            json!({"jobs":[j],"next_cursor":"j","event_cursor":0}),
        ] {
            let page: JobList = serde_json::from_value(value).unwrap();
            assert!(page.validate(Some("j"), None).is_err());
        }
    }
    #[test]
    fn bounded_ordinary_pages_remain_compatible() {
        let page: JobList = serde_json::from_value(json!({"jobs":[{"id":"k","sequence":1,"state":"queued"}],"next_cursor":"k","event_cursor":12})).unwrap();
        assert!(
            page.validate(Some("j"), Some(("instance", "session")))
                .is_ok()
        );
    }
}

/// Fixed authenticated route used only to reconcile body-less jobs changed during pagination.
pub(super) async fn reconcile_job(
    client: &reqwest::Client,
    origin: &str,
    token: &str,
    instance: &str,
    session: &str,
    id: &str,
) -> Result<Job, &'static str> {
    if !valid_id(id) {
        return Err("invalid_job_id");
    }
    let mut response = client
        .get(format!("{origin}/api/v1/jobs/{id}"))
        .bearer_auth(token)
        .header("x-diary-session", session)
        .send()
        .await
        .map_err(|_| "server_unavailable")?;
    if !response.status().is_success()
        || response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .is_none_or(|v| v.split(';').next() != Some("application/json"))
    {
        return Err("invalid_response");
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| "invalid_response")? {
        if bytes.len().saturating_add(chunk.len()) > 65536 {
            return Err("response_limit");
        }
        bytes.extend_from_slice(&chunk);
    }
    let job: Job = serde_json::from_slice(&bytes).map_err(|_| "invalid_response")?;
    if job.id != id || !job.owned_by(instance, session) {
        return Err("invalid_response");
    }
    Ok(job)
}

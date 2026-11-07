//! Explicit candidate-generation commands: fixed routes, closed statuses and bounded content.
use super::{AuthenticError, Bridge, Job, Revision, TurnAccepted};
use diary_application::agent::{AgentErrorCode, Locale, MAX_RESULT_BYTES, MAX_TEXT_BYTES};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::json;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    pub import_id: String,
    pub source_id: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GenerationInput {
    pub request_id: String,
    pub date: String,
    pub selections: Vec<Selection>,
    pub profile: String,
    pub locale: String,
    pub count: u8,
}
#[derive(Serialize, Deserialize, Clone)]
pub struct Progress {
    pub kind: String,
    pub job_id: String,
    pub request_id: String,
    pub instance_id: String,
    pub session_id: String,
    pub origin_date: String,
    pub profile_id: String,
    pub locale: String,
    pub baseline_revision: Option<Revision>,
    pub candidate_index: Option<u8>,
    pub requested_count: u8,
    pub finished_count: u8,
    pub ready_count: u8,
    pub failed_count: u8,
    pub cancelled_count: u8,
    pub not_started_count: u8,
    pub phase: Option<String>,
    pub elapsed_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub silence_ms: Option<u64>,
    pub last_agent_activity_at: Option<u64>,
    pub cancellation_requested: bool,
    pub candidate_ids: Vec<String>,
    pub error_code: Option<String>,
}
#[derive(Serialize, Deserialize)]
pub struct CandidateSlot {
    pub index: u8,
    pub id: Option<String>,
    pub status: String,
    pub error_code: Option<String>,
}
#[derive(Serialize, Deserialize)]
pub struct GenerationView {
    pub job: Job,
    pub date: String,
    pub profile: String,
    pub locale: String,
    pub count: u8,
    pub stale_baseline: bool,
    pub candidates: Vec<CandidateSlot>,
    pub progress: Progress,
    pub provisional: Option<String>,
}
#[derive(Serialize, Deserialize)]
pub struct Claim {
    pub claim: String,
    pub snippet_ids: Vec<String>,
}
#[derive(Serialize, Deserialize)]
pub struct CandidateView {
    pub id: String,
    pub job_id: String,
    pub index: u8,
    pub date: String,
    pub draft_markdown: String,
    pub claim_support: Vec<Claim>,
    pub warnings: Vec<String>,
}
fn id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
}
fn code(c: Option<&str>) -> bool {
    c.is_none_or(|c| AgentErrorCode::parse(c).is_some())
}
fn bad() -> AuthenticError {
    AuthenticError::new("invalid_input")
}
fn sound(v: &GenerationView) -> bool {
    let p = &v.progress;
    let slots = v.candidates.len() == v.count as usize
        && v.candidates.iter().enumerate().all(|(i, s)| {
            s.index as usize == i + 1
                && matches!(
                    s.status.as_str(),
                    "not_started" | "running" | "ready" | "failed" | "cancelled"
                )
                && code(s.error_code.as_deref())
                && match s.status.as_str() {
                    "ready" => s.id.as_deref() == Some(format!("{}-{}", v.job.id, i + 1).as_str()),
                    _ => s.id.is_none(),
                }
        });
    let baseline = p.baseline_revision.as_ref().is_none_or(|r| {
        r.sequence > 0 && r.sha256.len() == 64 && r.sha256.bytes().all(|b| b.is_ascii_hexdigit())
    });
    id(&v.job.id)
        && super::authentic::date_shape(&v.date)
        && super::profiles::valid_alias(&v.profile)
        && Locale::parse(&v.locale).is_ok()
        && (1..=5).contains(&v.count)
        && slots
        && baseline
        && p.kind == "generation"
        && p.job_id == v.job.id
        && id(&p.request_id)
        && id(&p.instance_id)
        && id(&p.session_id)
        && p.origin_date == v.date
        && p.profile_id == v.profile
        && p.locale == v.locale
        && p.requested_count == v.count
        && u16::from(p.finished_count)
            == u16::from(p.ready_count) + u16::from(p.failed_count) + u16::from(p.cancelled_count)
        && u16::from(p.finished_count) + u16::from(p.not_started_count) <= u16::from(v.count)
        && p.ready_count as usize == v.candidates.iter().filter(|s| s.status == "ready").count()
        && p.failed_count as usize == v.candidates.iter().filter(|s| s.status == "failed").count()
        && p.cancelled_count as usize
            == v.candidates
                .iter()
                .filter(|s| s.status == "cancelled")
                .count()
        && p.not_started_count as usize
            == v.candidates
                .iter()
                .filter(|s| s.status == "not_started")
                .count()
        && p.candidate_index.is_none_or(|i| (1..=v.count).contains(&i))
        && p.candidate_ids
            == v.candidates
                .iter()
                .filter_map(|s| s.id.clone())
                .collect::<Vec<_>>()
        && code(p.error_code.as_deref())
        && p.phase
            .as_deref()
            .is_none_or(|s| super::chat::STAGES.contains(&s))
        && v.provisional
            .as_ref()
            .is_none_or(|s| s.len() <= MAX_TEXT_BYTES)
}
impl Bridge {
    pub async fn start_generation(
        &self,
        input: GenerationInput,
    ) -> Result<TurnAccepted, AuthenticError> {
        if !id(&input.request_id)
            || !super::authentic::date_shape(&input.date)
            || !super::profiles::valid_alias(&input.profile)
            || Locale::parse(&input.locale).is_err()
            || !(1..=5).contains(&input.count)
            || input.selections.is_empty()
            || input
                .selections
                .iter()
                .any(|s| !id(&s.import_id) || !id(&s.source_id))
        {
            return Err(bad());
        }
        let accepted: TurnAccepted = self
            .authentic(Method::POST, "/generations", Some(json!(input)))
            .await?;
        if !id(&accepted.job.id) {
            return Err(AuthenticError::new("unavailable"));
        };
        Ok(accepted)
    }
    pub async fn get_generation(&self, job_id: String) -> Result<GenerationView, AuthenticError> {
        if !id(&job_id) {
            return Err(bad());
        }
        let view: GenerationView = self
            .authentic(Method::GET, &format!("/jobs/{job_id}/result"), None)
            .await?;
        if !sound(&view) || view.job.id != job_id {
            return Err(AuthenticError::new("unavailable"));
        };
        Ok(view)
    }
    pub async fn read_candidate(
        &self,
        candidate_id: String,
    ) -> Result<CandidateView, AuthenticError> {
        if !id(&candidate_id) {
            return Err(bad());
        }
        let c: CandidateView = self
            .authentic(Method::GET, &format!("/candidates/{candidate_id}"), None)
            .await?;
        let valid = c.id == candidate_id
            && id(&c.job_id)
            && (1..=5).contains(&c.index)
            && c.id == format!("{}-{}", c.job_id, c.index)
            && super::authentic::date_shape(&c.date)
            && c.draft_markdown.len() <= MAX_TEXT_BYTES
            && c.claim_support.len() <= 2000
            && c.warnings.len() <= 100
            && c.warnings.iter().all(|w| w.len() <= 2048)
            && c.claim_support.iter().all(|s| {
                s.claim.len() <= 2048
                    && !s.snippet_ids.is_empty()
                    && s.snippet_ids.iter().all(|s| {
                        s.strip_prefix("snippet-")
                            .is_some_and(|n| n.parse::<usize>().is_ok_and(|n| n > 0))
                    })
            });
        if !valid || serde_json::to_vec(&c).map_or(true, |s| s.len() > MAX_RESULT_BYTES) {
            return Err(AuthenticError::new("unavailable"));
        };
        Ok(c)
    }
    pub async fn discard_generation(&self, job_id: String) -> Result<(), AuthenticError> {
        if !id(&job_id) {
            return Err(bad());
        }
        let _: serde_json::Value = self
            .authentic(Method::DELETE, &format!("/generations/{job_id}"), None)
            .await?;
        Ok(())
    }
}
#[cfg(test)]
mod tests;

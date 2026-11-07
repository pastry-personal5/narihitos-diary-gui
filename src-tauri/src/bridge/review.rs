//! Paragraph review commands: fixed routes, closed inputs and a validated view. The view carries
//! the saved text and located pending hunks; the client never computes offsets itself.
use super::validate::{invalid, sha256, uuid};
use super::{AuthenticError, Bridge, Revision};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::json;

const MAX_BODY: usize = 2 * 1024 * 1024;
const MAX_HUNKS: usize = 4_000;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StartReviewInput {
    pub review_id: String,
    pub date: String,
    pub candidate_id: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecideReviewInput {
    pub review_id: String,
    pub operation_id: String,
    pub hunk_id: String,
    pub action: String,
    pub expected_revision: Option<Revision>,
    pub expected_set_digest: String,
}
#[derive(Serialize, Deserialize)]
pub struct ReviewHunk {
    pub id: String,
    pub kind: String,
    pub label: Option<String>,
    /// UTF-16 offsets into `body` of the text this hunk would remove or replace.
    pub from: usize,
    pub to: usize,
    /// Text this hunk would add.
    pub text: String,
}
#[derive(Serialize, Deserialize)]
pub struct ReviewView {
    pub review_id: String,
    pub date: String,
    pub candidate_id: String,
    pub revision: Option<Revision>,
    pub set_digest: String,
    pub body: String,
    pub hunks: Vec<ReviewHunk>,
}
#[derive(Serialize, Deserialize)]
pub struct ReviewReceipt {
    pub hunk: String,
    pub revision: Revision,
    pub before: String,
    pub after: String,
}
#[derive(Serialize, Deserialize)]
pub struct ReviewDecision {
    pub receipt: Option<ReviewReceipt>,
    /// Absent when the review no longer exists (a replay after restart).
    pub view: Option<ReviewView>,
}

fn opaque(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
}
fn revision(r: &Option<Revision>) -> bool {
    r.as_ref()
        .is_none_or(|r| r.sequence > 0 && sha256(&r.sha256))
}
fn sound(v: &ReviewView) -> bool {
    let units = v.body.encode_utf16().count();
    let mut last = 0;
    opaque(&v.review_id)
        && opaque(&v.candidate_id)
        && super::validate::date_shape(&v.date)
        && revision(&v.revision)
        && sha256(&v.set_digest)
        && v.body.len() <= MAX_BODY
        && v.hunks.len() <= MAX_HUNKS
        && v.hunks.iter().all(|h| {
            let ordered = h.from >= last && h.from <= h.to && h.to <= units;
            last = h.from;
            ordered
                && opaque(&h.id)
                && matches!(h.kind.as_str(), "replace" | "insert" | "delete")
                && h.text.len() <= MAX_BODY
                && h.label.as_deref().is_none_or(|l| l.len() <= 32)
                && (h.kind != "insert" || h.from == h.to)
                && (h.kind != "delete" || h.text.is_empty())
        })
}

impl Bridge {
    pub async fn start_review(
        &self,
        input: StartReviewInput,
    ) -> Result<ReviewView, AuthenticError> {
        if !uuid(&input.review_id)
            || !opaque(&input.candidate_id)
            || !super::validate::date_shape(&input.date)
        {
            return Err(invalid());
        }
        let view: ReviewView = self
            .authentic(Method::POST, "/reviews", Some(json!(input)))
            .await?;
        if !sound(&view) || view.review_id != input.review_id {
            return Err(AuthenticError::new("unavailable"));
        }
        Ok(view)
    }

    pub async fn decide_review(
        &self,
        input: DecideReviewInput,
    ) -> Result<ReviewDecision, AuthenticError> {
        if !uuid(&input.review_id)
            || !uuid(&input.operation_id)
            || !opaque(&input.hunk_id)
            || !matches!(input.action.as_str(), "accept" | "reject")
            || !sha256(&input.expected_set_digest)
            || !revision(&input.expected_revision)
        {
            return Err(invalid());
        }
        let path = format!("/reviews/{}/decisions", input.review_id);
        let body = json!({
            "operation_id": input.operation_id,
            "hunk_id": input.hunk_id,
            "action": input.action,
            "expected_revision": input.expected_revision,
            "expected_set_digest": input.expected_set_digest,
        });
        let decision: ReviewDecision = self.authentic(Method::POST, &path, Some(body)).await?;
        let receipt_ok = decision.receipt.as_ref().is_none_or(|r| {
            opaque(&r.hunk) && sha256(&r.revision.sha256) && sha256(&r.before) && sha256(&r.after)
        });
        let view_ok = decision
            .view
            .as_ref()
            .is_none_or(|v| sound(v) && v.review_id == input.review_id);
        if !receipt_ok || !view_ok || (decision.receipt.is_none() && decision.view.is_none()) {
            return Err(AuthenticError::new("unavailable"));
        }
        Ok(decision)
    }

    pub async fn end_review(&self, review_id: String) -> Result<(), AuthenticError> {
        if !uuid(&review_id) {
            return Err(invalid());
        }
        let _: serde_json::Value = self
            .authentic(Method::DELETE, &format!("/reviews/{review_id}"), None)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;

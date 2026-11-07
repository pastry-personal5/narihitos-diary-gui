//! Paragraph review routes: start against a captured candidate, serialized per-hunk decisions
//! and end. The view carries the saved text and located pending hunks (UTF-16 offsets), never
//! paths. Store work runs off the async executor.
use super::*;
use axum::extract::rejection::JsonRejection;
use diary_application::{
    operations::OperationId,
    review::{Decide, Decision, Error as ReviewFail, Start, View},
};
use diary_domain::authentic::{AuthenticRevision, DiaryDate};

pub(super) const BODY_LIMIT: usize = 4096;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StartBody {
    review_id: String,
    date: String,
    candidate_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Rev {
    sequence: u64,
    sha256: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DecideBody {
    operation_id: String,
    hunk_id: String,
    action: String,
    expected_revision: Option<Rev>,
    expected_set_digest: String,
}

fn rev_json(r: &Option<AuthenticRevision>) -> Value {
    json!(
        r.as_ref()
            .map(|r| json!({"sequence":r.sequence,"sha256":r.sha256}))
    )
}

fn utf16(text: &str, byte: usize) -> usize {
    text[..byte].encode_utf16().count()
}

fn view_json(v: &View) -> Value {
    json!({
        "review_id": v.review_id,
        "date": v.date.to_string(),
        "candidate_id": v.candidate_id,
        "revision": rev_json(&v.revision),
        "set_digest": v.set_digest,
        "body": v.body,
        "hunks": v.pending.iter().map(|p| json!({
            "id": p.id,
            "kind": p.kind.as_str(),
            "label": p.unit.label(),
            "from": utf16(&v.body, p.body.start),
            "to": utf16(&v.body, p.body.end),
            "text": p.text,
        })).collect::<Vec<_>>(),
    })
}

fn decision_json(d: &Decision) -> Value {
    json!({
        "receipt": d.receipt.as_ref().map(|r| json!({
            "hunk": r.hunk,
            "revision": {"sequence": r.revision.sequence, "sha256": r.revision.sha256},
            "before": r.before,
            "after": r.after,
        })),
        "view": d.view.as_ref().map(view_json),
    })
}

fn error(e: ReviewFail) -> Response {
    use ReviewFail::*;
    match e {
        NotFound => failure(StatusCode::NOT_FOUND, "not_found"),
        Busy => failure(StatusCode::CONFLICT, "busy"),
        Uncertain => failure(StatusCode::SERVICE_UNAVAILABLE, "outcome_uncertain"),
        Invalidated => failure(StatusCode::CONFLICT, "review_invalidated"),
        StaleBaseline => failure(StatusCode::CONFLICT, "stale_baseline"),
        StaleRevision => failure(StatusCode::CONFLICT, "revision_conflict"),
        SetMismatch => failure(StatusCode::CONFLICT, "set_mismatch"),
        Decided => failure(StatusCode::CONFLICT, "decided"),
        Unsupported => failure(StatusCode::UNPROCESSABLE_ENTITY, "review_unsupported"),
        Limit => failure(StatusCode::UNPROCESSABLE_ENTITY, "review_limit"),
        Failed(_) => failure(StatusCode::CONFLICT, "operation_failed"),
        Store(e) => authentic::store_error(e),
    }
}

type Ports = (
    Arc<dyn diary_application::authentic::AuthenticStore>,
    Arc<dyn diary_domain::review::Hasher>,
);
fn ports(s: &ServerState) -> Result<Ports, Response> {
    match (s.authentic.clone(), s.review_hasher.clone()) {
        (Some(store), Some(hasher)) => Ok((store, hasher)),
        _ => Err(failure(StatusCode::SERVICE_UNAVAILABLE, "unavailable")),
    }
}

pub(super) async fn start(
    State(s): State<Arc<ServerState>>,
    headers: HeaderMap,
    b: Result<Json<StartBody>, JsonRejection>,
) -> Response {
    let session = match imports::authorize(&s, &headers) {
        Ok(id) => id,
        Err(r) => return r,
    };
    let b = match imports::body(b) {
        Ok(b) => b,
        Err(r) => return r,
    };
    let (store, hasher) = match ports(&s) {
        Ok(p) => p,
        Err(r) => return r,
    };
    let Some(date) = DiaryDate::parse(&b.date) else {
        return failure(StatusCode::BAD_REQUEST, "invalid_input");
    };
    let Some((job_id, index)) = b.candidate_id.rsplit_once('-').and_then(|(job, i)| {
        i.parse::<usize>()
            .ok()
            .filter(|i| (1..=5).contains(i))
            .map(|i| (job.to_owned(), i - 1))
    }) else {
        return failure(StatusCode::BAD_REQUEST, "invalid_input");
    };
    let Some(g) = s
        .generations
        .lock()
        .expect("generations poisoned")
        .get(&session, &job_id)
    else {
        return failure(StatusCode::NOT_FOUND, "result_unavailable");
    };
    let (body, baseline) = {
        let g = g.lock().expect("generation poisoned");
        let Some(candidate) = g.candidates.get(index).and_then(Option::as_ref) else {
            return failure(StatusCode::NOT_FOUND, "not_found");
        };
        if g.context.date() != date {
            return failure(StatusCode::BAD_REQUEST, "invalid_input");
        }
        (
            candidate.draft_markdown.clone(),
            g.context.baseline().cloned(),
        )
    };
    let s2 = s.clone();
    let result = tokio::task::spawn_blocking(move || {
        s2.reviews.start(
            &*store,
            &*hasher,
            Start {
                session: &session,
                review_id: &b.review_id,
                date,
                candidate_id: &b.candidate_id,
                candidate_body: &body,
                baseline,
            },
        )
    })
    .await;
    match result {
        Ok(Ok(view)) => (StatusCode::CREATED, Json(view_json(&view))).into_response(),
        Ok(Err(e)) => error(e),
        Err(_) => failure(StatusCode::INTERNAL_SERVER_ERROR, "internal"),
    }
}

pub(super) async fn decide(
    State(s): State<Arc<ServerState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    b: Result<Json<DecideBody>, JsonRejection>,
) -> Response {
    let session = match imports::authorize(&s, &headers) {
        Ok(id) => id,
        Err(r) => return r,
    };
    let b = match imports::body(b) {
        Ok(b) => b,
        Err(r) => return r,
    };
    let (store, hasher) = match ports(&s) {
        Ok(p) => p,
        Err(r) => return r,
    };
    let accept = match b.action.as_str() {
        "accept" => true,
        "reject" => false,
        _ => return failure(StatusCode::BAD_REQUEST, "invalid_input"),
    };
    let Some(operation_id) = OperationId::parse(&b.operation_id) else {
        return failure(StatusCode::BAD_REQUEST, "invalid_input");
    };
    if !imports::opaque(&b.hunk_id) || !imports::opaque(&b.expected_set_digest) {
        return failure(StatusCode::BAD_REQUEST, "invalid_input");
    }
    let request = Decide {
        operation_id,
        hunk: b.hunk_id,
        accept,
        expected: b.expected_revision.map(|r| AuthenticRevision {
            sequence: r.sequence,
            sha256: r.sha256,
        }),
        set_digest: b.expected_set_digest,
    };
    let s2 = s.clone();
    let result = tokio::task::spawn_blocking(move || {
        s2.reviews.decide(&*store, &*hasher, &session, &id, request)
    })
    .await;
    match result {
        Ok(Ok(d)) => Json(decision_json(&d)).into_response(),
        Ok(Err(e)) => error(e),
        Err(_) => failure(StatusCode::INTERNAL_SERVER_ERROR, "internal"),
    }
}

pub(super) async fn end(
    State(s): State<Arc<ServerState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let session = match imports::authorize(&s, &headers) {
        Ok(id) => id,
        Err(r) => return r,
    };
    match s.reviews.end(&session, &id) {
        Ok(()) => Json(json!({})).into_response(),
        Err(e) => error(e),
    }
}

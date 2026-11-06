//! Owning-session import jobs and bounded body-less source pages. Errors are fixed codes.
#![allow(clippy::result_large_err)]
use super::*;
use axum::extract::rejection::JsonRejection;
use diary_application::imports::{Authority, ImportError, ImportListing, ImportStore};
use diary_application::source::SourceListing;
use diary_domain::{
    authentic::DiaryDate,
    source::stamp::{Dating, Instant},
};
use std::sync::atomic::Ordering;

fn opaque(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
}
pub(super) fn valid_query(query: &str) -> bool {
    let mut keys = std::collections::BTreeSet::new();
    query.split('&').all(|part| {
        let Some((key, value)) = part.split_once('=') else {
            return false;
        };
        keys.insert(key)
            && match key {
                "import_id" => opaque(value),
                "cursor" => value.len() <= 8 && value.parse::<usize>().is_ok_and(|n| n <= 30_000),
                _ => false,
            }
    }) && keys.contains("import_id")
}
pub(super) fn authorize(s: &ServerState, headers: &HeaderMap) -> Result<String, Response> {
    let id = session(headers).map_err(app_error)?;
    s.coordinator
        .lock()
        .expect("coordinator poisoned")
        .list(id, s.now(), None, 1)
        .map_err(app_error)?;
    Ok(id.to_owned())
}
pub(super) fn store(s: &ServerState) -> Result<Arc<dyn ImportStore>, Response> {
    s.imports
        .clone()
        .ok_or_else(|| failure(StatusCode::SERVICE_UNAVAILABLE, "storage_unavailable"))
}
pub(super) fn cancelled(s: &ServerState, session: &str) -> bool {
    s.stopping.load(Ordering::Acquire)
        || s.coordinator
            .lock()
            .expect("coordinator poisoned")
            .list(session, s.now(), None, 1)
            .is_err()
}
/// Polls `f` on the first call and every 64th after; a `true` is latched.
/// Scans call the predicate per directory entry, and `f` takes the coordinator lock.
pub(super) fn throttled(f: impl Fn() -> bool) -> impl Fn() -> bool {
    let calls = std::cell::Cell::new(0u32);
    let hit = std::cell::Cell::new(false);
    move || {
        let n = calls.get();
        calls.set(n.wrapping_add(1));
        if !hit.get() && n.is_multiple_of(64) {
            hit.set(f());
        }
        hit.get()
    }
}
fn code(e: ImportError) -> &'static str {
    match e {
        ImportError::UnnamedZone => "unnamed_timezone",
        ImportError::TimezoneDatabase => "timezone_database_unavailable",
        ImportError::InvalidDiaryDate => "invalid_diary_date",
        ImportError::Storage => "storage_error",
        ImportError::NotFound => "not_found",
        ImportError::Stale => "source_stale",
        ImportError::InvalidAuthority => "invalid_time_authority",
        ImportError::FilenameDirectoryMismatch => "fix_filename_directory_externally",
        ImportError::Cancelled => "cancelled",
        ImportError::Limit => "limit",
    }
}
pub(super) fn error(e: ImportError) -> Response {
    failure(
        match e {
            ImportError::NotFound => StatusCode::NOT_FOUND,
            ImportError::Storage | ImportError::UnnamedZone | ImportError::TimezoneDatabase => {
                StatusCode::SERVICE_UNAVAILABLE
            }
            ImportError::Limit => StatusCode::TOO_MANY_REQUESTS,
            _ => StatusCode::CONFLICT,
        },
        code(e),
    )
}
fn remember_error(s: &ServerState, session: &str, job: &str, code: &'static str) {
    let mut errors = s.import_errors.lock().expect("import errors poisoned");
    if errors.0 != session {
        *errors = (session.to_owned(), Default::default());
    }
    // Per-job coordinator admission accounts for this bounded metadata.
    errors.1.insert(job.to_owned(), code);
}
fn body<T>(b: Result<Json<T>, JsonRejection>) -> Result<T, Response> {
    b.map(|Json(v)| v).map_err(|e| {
        failure(
            if e.status() == StatusCode::PAYLOAD_TOO_LARGE {
                StatusCode::PAYLOAD_TOO_LARGE
            } else {
                StatusCode::BAD_REQUEST
            },
            "invalid_input",
        )
    })
}
fn publish(s: &ServerState, c: &mut Coordinator, session: &str, job: &Job) {
    let bytes = serde_json::to_vec(&json!({"instance_id":s.instance_id,"session_id":session,"event_id":u64::MAX,"job_id":job.id,"job_sequence":job.sequence,"kind":"job_changed","payload":JobDto::from(job.clone())})).expect("fixed DTO").len();
    let _ = c.publish(session, s.now(), &job.id, bytes);
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Create {
    date: String,
    request_id: String,
}

pub(super) async fn create(
    State(s): State<Arc<ServerState>>,
    headers: HeaderMap,
    b: Result<Json<Create>, JsonRejection>,
) -> Response {
    let session = match authorize(&s, &headers) {
        Ok(id) => id,
        Err(r) => return r,
    };
    let b = match body(b) {
        Ok(b) => b,
        Err(r) => return r,
    };
    let date = match DiaryDate::parse(&b.date) {
        Some(d) => d,
        None => return failure(StatusCode::BAD_REQUEST, "invalid_input"),
    };
    if !opaque(&b.request_id) {
        return failure(StatusCode::BAD_REQUEST, "invalid_input");
    }
    let store = match store(&s) {
        Ok(st) => st,
        Err(r) => return r,
    };
    let generation = s.config_generation.load(Ordering::SeqCst);
    // Fixed-width injective encoding of the only inputs, in the coordinator's 64-hex
    // request-key field. No lossy hash or untrusted arbitrary request digest.
    let key = format!(
        "{:04x}{:02x}{:02x}{generation:016x}{:040x}",
        date.year(),
        date.month(),
        date.day(),
        10
    );
    let mut c = s.coordinator.lock().expect("coordinator poisoned");
    let job = match c.admit(
        &session,
        s.now(),
        &b.request_id,
        &key,
        &Uuid::new_v4().to_string(),
        4096,
    ) {
        Ok(j) => j,
        Err(e) => return app_error(e),
    };
    let response = json!({"import_id":job.id,"job":JobDto::from(job.clone())});
    if job.state != JobState::Queued {
        return (StatusCode::ACCEPTED, Json(response)).into_response();
    }
    let permit = match s.source_slots.clone().try_acquire_owned() {
        Ok(p) => p,
        Err(_) => {
            remember_error(&s, &session, &job.id, "limit");
            if let Ok(failed) = c.transition(&session, s.now(), &job.id, JobState::Failed) {
                publish(&s, &mut c, &session, &failed);
            }
            return failure(StatusCode::TOO_MANY_REQUESTS, "limit");
        }
    };
    let running = c
        .transition(&session, s.now(), &job.id, JobState::Running)
        .expect("new admission");
    publish(&s, &mut c, &session, &running);
    drop(c);
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let check = throttled(|| {
            cancelled(&s, &session)
                || s.coordinator
                    .lock()
                    .expect("coordinator poisoned")
                    .get(&session, s.now(), &job.id)
                    .map_or(true, |j| j.state == JobState::Cancelled)
        });
        let result = store.import_date(&job.id, date, generation, &check);
        let mut c = s.coordinator.lock().expect("coordinator poisoned");
        if c.get(&session, s.now(), &job.id).is_err() {
            return;
        }
        let terminal = match result {
            Ok(_) => JobState::Succeeded,
            Err(ImportError::Cancelled) => JobState::Cancelled,
            Err(e) => {
                remember_error(&s, &session, &job.id, code(e));
                JobState::Failed
            }
        };
        if let Ok(job) = c.transition(&session, s.now(), &job.id, terminal) {
            publish(&s, &mut c, &session, &job);
        }
    });
    (StatusCode::ACCEPTED, Json(response)).into_response()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ListQuery {
    import_id: String,
    cursor: Option<usize>,
}
fn instant(t: Instant) -> Value {
    json!({"unix_ms":t.unix_ms,"precision":match t.precision { diary_domain::source::stamp::Precision::Second => "second", _ => "milli" }})
}
fn source(s: &SourceListing) -> Value {
    let dating = match s.dating {
        Dating::Agreed(t) => json!({"state":"agreed","header":instant(t),"filename":instant(t)}),
        Dating::Disagree { header, filename } => {
            json!({"state":"timestamp_conflict","header":instant(header),"filename":instant(filename)})
        }
        Dating::HeaderOnly { header, .. } => {
            json!({"state":"header_only","header":instant(header)})
        }
        Dating::FilenameOnly { filename, .. } => {
            json!({"state":"filename_only","filename":instant(filename)})
        }
        Dating::Undated { .. } => json!({"state":"undated"}),
    };
    let digest: String = s.token.sha256.iter().map(|b| format!("{b:02x}")).collect();
    // Token stays server-side except the opaque identity, digest and context needed
    // to request a resolution. Native identity and root namespace are never exposed.
    json!({"id":s.id,"relative_key":s.token.key,"digest":digest,"import_id":s.token.import_id,"dating":dating,"duplicate_producer_id":!s.warnings.is_empty()})
}
pub(super) fn failure_code(outcome: &diary_application::source::SourceOutcome) -> &'static str {
    use diary_application::source::{SourceOutcome, Unavailable};
    match outcome {
        SourceOutcome::Unavailable(e) => match e {
            Unavailable::Missing => "missing",
            Unavailable::Denied => "denied",
            Unavailable::NotDownloaded => "not_downloaded",
            Unavailable::NotRegular => "not_regular",
            Unavailable::TooLarge => "too_large",
            Unavailable::SourceChanging => "source_changing",
            Unavailable::InvalidPath => "invalid_path",
            Unavailable::Io => "unavailable",
        },
        SourceOutcome::Malformed(_) => "malformed",
        SourceOutcome::Unsupported { .. } => "unsupported",
        _ => "conflict",
    }
}
fn page(r: ImportListing, cursor: usize, trash_available: bool) -> Result<Value, ImportError> {
    let total =
        r.included.len() + r.warnings.len() + r.scan.failures.len() + r.scan.collisions.len();
    if cursor > total {
        return Err(ImportError::NotFound);
    }
    let misplaced: std::collections::BTreeSet<_> = r.misplaced.iter().collect();
    // Select references first: skipped records must never allocate JSON or digests.
    enum Item<'a> {
        Included(&'a diary_application::imports::IncludedSource),
        Warning(&'a SourceListing),
        Failure(usize, &'a str, &'a diary_application::source::SourceOutcome),
        Collision(usize, &'a Vec<String>),
    }
    let items: Vec<_> = r.included.iter().map(Item::Included)
        .chain(r.warnings.iter().map(Item::Warning))
        .chain(r.scan.failures.iter().enumerate().map(|(i, (key, outcome))| Item::Failure(i, key, outcome)))
        .chain(r.scan.collisions.iter().enumerate().map(|(i,keys)| Item::Collision(i,keys)))
        .skip(cursor).take(100).map(|item| match item {
            Item::Included(s) => json!({"kind":"included","source":enhanced_source(&s.source,&r,true,trash_available),"instant":instant(s.instant),"misplaced":misplaced.contains(&s.source.id)}),
            Item::Warning(s) => json!({"kind":"warning","source":enhanced_source(s,&r,false,trash_available),"misplaced":misplaced.contains(&s.id)}),
            Item::Failure(i, key, outcome) => {
                let group = r.navigation.as_ref().and_then(|n|n.failure_groups.get(i));
                let readable = r.scan.readable.iter().find(|s| s.rel == key && r.navigation.as_ref().and_then(|n|n.sources.get(&s.id)).is_some_and(|d| Some(&d.group_id)==group));
                json!({"kind":"failure","relative_key":key,"code":failure_code(outcome),"group_id":group,"source":readable.map(|s|enhanced_source(s,&r,false,trash_available))})
            },
            Item::Collision(i, keys) => json!({"kind":"path_collision","relative_keys":keys,"group_id":r.navigation.as_ref().and_then(|n|n.collision_groups.get(i))}),
        }).collect();
    let next = cursor + items.len();
    Ok(
        json!({"groups":r.navigation.as_ref().map(|n|n.groups.iter().map(|g|json!({"id":g.id,"alias":g.alias,"type":g.kind})).collect::<Vec<_>>()),"reimport_required":r.navigation.is_none(),"import_id":r.context.id,"date":r.context.date.to_string(),"zone":r.context.zone.name,"timezone_database":r.context.zone.database_version,"config_generation":r.context.config_generation,"completed_at":r.context.completed_at,"items":items,"next_cursor":if next < total { Some(next) } else { None },"incomplete":r.scan.incomplete}),
    )
}
pub(super) async fn list(
    State(s): State<Arc<ServerState>>,
    headers: HeaderMap,
    Query(q): Query<ListQuery>,
) -> Response {
    let session = match authorize(&s, &headers) {
        Ok(id) => id,
        Err(r) => return r,
    };
    let st = match store(&s) {
        Ok(st) => st,
        Err(r) => return r,
    };
    let permit = match s.source_slots.clone().try_acquire_owned() {
        Ok(p) => p,
        Err(_) => return error(ImportError::Limit),
    };
    let result = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let r = st.list(&q.import_id, &throttled(|| cancelled(&s, &session)))?;
        if r.context.config_generation != s.config_generation.load(Ordering::SeqCst) {
            return Err(ImportError::Stale);
        }
        page(
            r,
            q.cursor.unwrap_or(0),
            s.source_trash.as_ref().is_some_and(|t| t.available()),
        )
    })
    .await;
    match result {
        Ok(Ok(value)) => Json(value).into_response(),
        Ok(Err(e)) => error(e),
        Err(_) => failure(StatusCode::INTERNAL_SERVER_ERROR, "internal"),
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Resolve {
    import_id: String,
    digest: String,
    authority: String,
}
pub(super) async fn resolve(
    State(s): State<Arc<ServerState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    b: Result<Json<Resolve>, JsonRejection>,
) -> Response {
    let session = match authorize(&s, &headers) {
        Ok(id) => id,
        Err(r) => return r,
    };
    let b = match body(b) {
        Ok(b) => b,
        Err(r) => return r,
    };
    if !opaque(&id)
        || !opaque(&b.import_id)
        || b.digest.len() != 64
        || !b.digest.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return failure(StatusCode::BAD_REQUEST, "invalid_input");
    }
    let authority = match b.authority.as_str() {
        "header" => Authority::Header,
        "filename" => Authority::Filename,
        _ => return failure(StatusCode::BAD_REQUEST, "invalid_input"),
    };
    let mut digest = [0; 32];
    for (i, byte) in digest.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&b.digest[i * 2..i * 2 + 2], 16).expect("validated hex");
    }
    let st = match store(&s) {
        Ok(st) => st,
        Err(r) => return r,
    };
    let permit = match s.source_slots.clone().try_acquire_owned() {
        Ok(p) => p,
        Err(_) => return error(ImportError::Limit),
    };
    let generation = s.config_generation.load(Ordering::SeqCst);
    match tokio::task::spawn_blocking(move || {
        let _permit = permit;
        st.resolve(
            &b.import_id,
            &id,
            &digest,
            authority,
            generation,
            &throttled(|| cancelled(&s, &session)),
        )
    })
    .await
    {
        Ok(Ok(())) => Json(json!({"resolved":true})).into_response(),
        Ok(Err(e)) => error(e),
        Err(_) => failure(StatusCode::INTERNAL_SERVER_ERROR, "internal"),
    }
}

#[cfg(test)]
mod tests;

pub(super) fn valid_read_query(query: &str) -> bool {
    query.strip_prefix("snapshot_id=").is_some_and(opaque)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReadQuery {
    snapshot_id: String,
}
pub(super) async fn read(
    State(s): State<Arc<ServerState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(q): Query<ReadQuery>,
) -> Response {
    use diary_application::source::ViewError;
    let session = match authorize(&s, &headers) {
        Ok(id) => id,
        Err(r) => return r,
    };
    if !opaque(&id) || !opaque(&q.snapshot_id) {
        return failure(StatusCode::BAD_REQUEST, "invalid_input");
    }
    let st = match store(&s) {
        Ok(st) => st,
        Err(r) => return r,
    };
    let permit = match s.source_slots.clone().try_acquire_owned() {
        Ok(p) => p,
        Err(_) => return error(ImportError::Limit),
    };
    let generation = s.config_generation.load(Ordering::SeqCst);
    match tokio::task::spawn_blocking(move || {
        let _permit = permit;
        // Legacy contexts need a new import; never invent a descriptor from a path.
        let snapshot = st.list(&q.snapshot_id, &|| cancelled(&s, &session)).map_err(|_| "source_stale")?;
        if snapshot.navigation.is_none() { return Err("reimport_required"); }
        let view = st.read(&q.snapshot_id, &id, generation, &|| cancelled(&s, &session)).map_err(|e| match e { ViewError::Stale => "source_stale", ViewError::Outcome(o) => failure_code(&o) })?;
        if generation != s.config_generation.load(Ordering::SeqCst) || cancelled(&s, &session) { return Err("source_stale"); }
        if view.body.len() > diary_domain::authentic::MAX_BODY_BYTES { return Err("too_large"); }
        let value = json!({"source":enhanced_source(&view.listing,&snapshot,false,s.source_trash.as_ref().is_some_and(|t|t.available())),"snapshot_id":q.snapshot_id,"body":view.body,"capabilities":source_capabilities(view.write, view.trash, s.source_trash.as_ref().is_some_and(|t|t.available()))});
        let bytes = serde_json::to_vec(&value).map_err(|_|"internal")?;
        if bytes.len() > 10 * 1024 * 1024 { return Err("too_large"); }
        Ok(bytes)
    }).await {
        Ok(Ok(bytes)) => ([(axum::http::header::CONTENT_TYPE,"application/json")],bytes).into_response(),
        Ok(Err(code))=>failure(StatusCode::CONFLICT,code),Err(_)=>failure(StatusCode::INTERNAL_SERVER_ERROR,"internal"),
    }
}

fn source_capabilities(
    write: Result<(), diary_application::source::WriteReason>,
    trash: Result<(), diary_application::source::TrashReason>,
    trash_available: bool,
) -> Value {
    use diary_application::source::{TrashReason, WriteReason};
    let reason = match write {
        Ok(()) => "ready",
        Err(WriteReason::ProducerUnverified) => "producer_unverified",
        Err(WriteReason::WriterUnverified) => "writer_unverified",
        Err(WriteReason::UnsupportedFormat) => "unsupported_format",
    };
    // Adapter policy alone is not readiness: an unrecovered or absent service cannot preview.
    let trash = trash.and_then(|()| {
        if trash_available {
            Ok(())
        } else {
            Err(TrashReason::TrashUnverified)
        }
    });
    let trash_reason = match trash {
        Ok(()) => "ready",
        Err(TrashReason::ProducerUnverified) => "producer_unverified",
        Err(TrashReason::TrashUnverified) => "trash_unverified",
        Err(TrashReason::UnsupportedFormat) => "unsupported_format",
    };
    json!({"read":true,"save":write.is_ok(),"trash":trash.is_ok(),"reason":reason,"trash_reason":trash_reason})
}

fn local(t: &diary_application::imports::LocalTime) -> Value {
    json!({"date":t.date.to_string(),"datetime":t.datetime,"offset_seconds":t.offset_seconds})
}
fn stamp_error(e: diary_domain::source::stamp::StampError) -> &'static str {
    use diary_domain::source::stamp::StampError::*;
    match e {
        Format => "format",
        Offset => "offset",
        Calendar => "calendar",
        DirMismatch => "directory_mismatch",
    }
}
fn enhanced_source(
    s: &SourceListing,
    r: &ImportListing,
    eligible: bool,
    trash_available: bool,
) -> Value {
    let mut v = source(s);
    let (header_error, filename_error) = match s.dating {
        Dating::HeaderOnly { filename, .. } => (None, Some(stamp_error(filename))),
        Dating::FilenameOnly { header, .. } => (Some(stamp_error(header)), None),
        Dating::Undated { header, filename } => {
            (Some(stamp_error(header)), Some(stamp_error(filename)))
        }
        _ => (None, None),
    };
    v["header_invalid"] = json!(header_error);
    v["filename_invalid"] = json!(filename_error);
    let display = r.navigation.as_ref().and_then(|n| n.sources.get(&s.id));
    v["navigation"] = display.map(|d|json!({"group_id":d.group_id,"header":d.header.as_ref().map(local),"filename":d.filename.as_ref().map(local),"included":d.included.as_ref().map(local),"authority":d.authority.map(|a|match a {Authority::Header=>"header",Authority::Filename=>"filename"})})).unwrap_or(Value::Null);
    v["eligible"] = json!(eligible && display.is_some());
    let producer = r
        .navigation
        .as_ref()
        .and_then(|n| display.and_then(|d| n.groups.iter().find(|g| g.id == d.group_id)))
        .map(|g| g.kind.as_str());
    let trash_reason = if display.is_none() {
        "reimport_required"
    } else if producer != Some("narihito") {
        "producer_unverified"
    } else if trash_available {
        "checked_on_preview"
    } else {
        "trash_unverified"
    };
    v["capabilities"] = json!({"read":display.is_some(),"save":false,"trash":false,"trash_reason":trash_reason,"reason":if display.is_some(){"checked_on_open"}else{"reimport_required"}});
    v
}

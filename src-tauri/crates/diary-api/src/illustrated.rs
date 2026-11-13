//! Authenticated illustrated resources and asynchronous creation jobs.
use super::*;
use axum::extract::rejection::JsonRejection;
use diary_application::{
    agent::*,
    illustrated::{self, Error},
};
use diary_domain::authentic::DiaryDate;
fn error(e: Error) -> Response {
    let (s, c) = match e {
        Error::ProfileUnavailable => (StatusCode::CONFLICT, "profile_unavailable"),
        Error::EngineUnavailable => (StatusCode::CONFLICT, "engine_unavailable"),
        Error::SourceBusy => (StatusCode::CONFLICT, "source_busy"),
        Error::Invalid => (StatusCode::UNPROCESSABLE_ENTITY, "invalid_illustrated"),
        Error::NotFound => (StatusCode::NOT_FOUND, "not_found"),
        Error::Stale => (StatusCode::CONFLICT, "stale_version"),
        Error::Busy => (StatusCode::CONFLICT, "busy"),
        Error::ConfirmationRequired => (StatusCode::CONFLICT, "confirmation_required"),
        Error::Cancelled => (StatusCode::CONFLICT, "cancelled"),
        Error::Agent => (StatusCode::BAD_GATEWAY, "agent_failed"),
        Error::Render => (StatusCode::BAD_GATEWAY, "render_failed"),
        Error::Storage | Error::Unavailable => (StatusCode::SERVICE_UNAVAILABLE, "unavailable"),
    };
    failure(s, c)
}
fn date(s: &str) -> Result<DiaryDate, Response> {
    DiaryDate::parse(s).ok_or_else(|| failure(StatusCode::BAD_REQUEST, "invalid_input"))
}
fn store(s: &ServerState) -> Result<illustrated::SharedStore, Response> {
    s.illustrated
        .as_ref()
        .map(|v| v.0.clone())
        .ok_or_else(|| error(Error::Unavailable))
}
async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, Error> + Send + 'static,
) -> Result<T, Response> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|_| error(Error::Storage))?
        .map_err(error)
}
pub async fn list(State(s): State<Arc<ServerState>>, headers: HeaderMap) -> Response {
    if let Err(r) = imports::authorize(&s, &headers) {
        return r;
    }
    let store = match store(&s) {
        Ok(v) => v,
        Err(r) => return r,
    };
    match blocking(move || store.list()).await {
        Ok(d) => Json(json!({"dates":d.iter().map(ToString::to_string).collect::<Vec<_>>()}))
            .into_response(),
        Err(r) => r,
    }
}
pub async fn read(
    State(s): State<Arc<ServerState>>,
    Path(d): Path<String>,
    headers: HeaderMap,
) -> Response {
    if let Err(r) = imports::authorize(&s, &headers) {
        return r;
    }
    let (store, d) = match store(&s).and_then(|v| Ok((v, date(&d)?))) {
        Ok(v) => v,
        Err(r) => return r,
    };
    match blocking(move||store.read(d)).await {Ok(Some(e))=>Json(json!({"date":d.to_string(),"version":e.id,"markdown":e.body,"source_authentic_id":e.source_id,"source_revision":e.source_revision,"source_sha256":e.source_sha256,"agent_profile":e.agent_profile,"image_engine":e.image_engine,"theme_count":e.theme_count,"photos":e.photos.iter().enumerate().map(|(i,p)|json!({"file":format!("{:02}.png",i+1),"seed":p.seed,"prompt":p.prompt})).collect::<Vec<_>>()})).into_response(),Ok(None)=>error(Error::NotFound),Err(r)=>r}
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhotoQuery {
    version: String,
}
pub async fn photo(
    State(s): State<Arc<ServerState>>,
    Path((d, n)): Path<(String, String)>,
    Query(q): Query<PhotoQuery>,
    headers: HeaderMap,
) -> Response {
    if let Err(r) = imports::authorize(&s, &headers) {
        return r;
    }
    let (store, d) = match store(&s).and_then(|v| Ok((v, date(&d)?))) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Some(n) = n
        .strip_suffix(".png")
        .filter(|v| v.len() == 2 && v.bytes().all(|c| c.is_ascii_digit()))
        .and_then(|v| v.parse::<usize>().ok())
    else {
        return error(Error::Invalid);
    };
    match blocking(move || store.photo(d, &q.version, n)).await {
        Ok(b) => (
            [
                ("content-type", "image/png"),
                ("cache-control", "no-store"),
                ("x-content-type-options", "nosniff"),
            ],
            b,
        )
            .into_response(),
        Err(r) => r,
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Delete {
    version: String,
    confirmed: bool,
}
pub async fn delete(
    State(s): State<Arc<ServerState>>,
    Path(d): Path<String>,
    headers: HeaderMap,
    b: Result<Json<Delete>, JsonRejection>,
) -> Response {
    if let Err(r) = imports::authorize(&s, &headers) {
        return r;
    }
    let b = match imports::body(b) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let (store, d) = match store(&s).and_then(|v| Ok((v, date(&d)?))) {
        Ok(v) => v,
        Err(r) => return r,
    };
    // Serialize deletion admission with create admission. The worker's publication is also locked by the store.
    let guard = s.coordinator.lock().unwrap();
    if s.illustrated_jobs
        .lock()
        .unwrap()
        .values()
        .any(|j| j.date == d && !j.terminal)
    {
        return error(Error::Busy);
    }
    // Reserve a date until the blocking deletion completes.
    let key = Uuid::new_v4().to_string();
    s.illustrated_jobs
        .lock()
        .unwrap()
        .insert(key.clone(), illustrated::JobProgress::new(String::new(), d));
    drop(guard);
    let cleanup = s.clone();
    let result = blocking(move || {
        let result = store.delete(d, &b.version, b.confirmed);
        cleanup.illustrated_jobs.lock().unwrap().remove(&key);
        result
    })
    .await;
    match result {
        Ok(()) => Json(json!({"deleted":true})).into_response(),
        Err(r) => r,
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Start {
    request_id: String,
    profile: String,
    locale: String,
    engine: Option<String>,
}
fn fingerprint(b: &Start, d: DiaryDate) -> String {
    use std::hash::{Hash, Hasher};
    (0u8..4)
        .map(|n| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (
                n,
                "illustrated",
                d.to_string(),
                &b.profile,
                &b.locale,
                &b.engine,
            )
                .hash(&mut h);
            format!("{:016x}", h.finish())
        })
        .collect()
}
fn instant() -> String {
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as libc::time_t;
    let mut tm = std::mem::MaybeUninit::<libc::tm>::uninit();
    // SAFETY: both pointers are valid; gmtime_r initializes tm on success.
    let tm = unsafe {
        if libc::gmtime_r(&t, tm.as_mut_ptr()).is_null() {
            return String::new();
        }
        tm.assume_init()
    };
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        tm.tm_year + 1900,
        tm.tm_mon + 1,
        tm.tm_mday,
        tm.tm_hour,
        tm.tm_min,
        tm.tm_sec
    )
}
pub fn progress(s: &ServerState, session: &str, id: &str) -> Option<Value> {
    let jobs = s.illustrated_jobs.lock().unwrap();
    let j = jobs.get(id)?;
    if j.session != session {
        return None;
    }
    Some(
        json!({"kind":"illustrated","job_id":id,"instance_id":s.instance_id,"session_id":session,"origin_date":j.date.to_string(),"phase":if j.terminal{"finished"}else if j.run.token().is_cancelled(){"stopping"}else if j.photo==0{"text"}else{"photo"},"photo_index":j.photo,"photo_count":j.total,"error_code":j.error.map(|e|match e{Error::Cancelled=>"cancelled",Error::Agent=>"agent_failed",Error::Render=>"render_failed",Error::NotFound=>"no_authentic",Error::ProfileUnavailable=>"profile_unavailable",Error::EngineUnavailable=>"engine_unavailable",Error::SourceBusy=>"source_busy",Error::Invalid=>"invalid_illustrated",_=>"unavailable"})}),
    )
}
pub async fn create(
    State(s): State<Arc<ServerState>>,
    Path(d): Path<String>,
    headers: HeaderMap,
    b: Result<Json<Start>, JsonRejection>,
) -> Response {
    let session = match imports::authorize(&s, &headers) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let b = match imports::body(b) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let d = match date(&d) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let locale = match Locale::parse(&b.locale) {
        Ok(v) => v,
        Err(_) => return error(Error::Invalid),
    };
    if !imports::opaque(&b.request_id) || !imports::opaque(&b.profile) {
        return error(Error::Invalid);
    }
    let (Some((store, settings)), Some(authentic), Some(runner), Some(profiles)) = (
        s.illustrated.clone(),
        s.authentic.clone(),
        s.agent_runner.clone(),
        s.profiles.clone(),
    ) else {
        return error(Error::Unavailable);
    };
    let digest = fingerprint(&b, d);
    let mut c = s.coordinator.lock().unwrap();
    match c.lookup_request(&session, s.now(), &b.request_id, &digest) {
        Ok(Some(j)) => {
            return (StatusCode::ACCEPTED, Json(json!({"job":JobDto::from(j)}))).into_response();
        }
        Err(e) => return app_error(e),
        _ => {}
    }
    if s.illustrated_jobs
        .lock()
        .unwrap()
        .values()
        .any(|j| j.date == d && !j.terminal)
    {
        return error(Error::Busy);
    }
    let Some(profile) = profiles
        .list()
        .into_iter()
        .find(|p| p.alias == b.profile)
        .map(|p| ProfileHandle {
            alias: p.alias,
            kind: p.kind,
        })
    else {
        return error(Error::Unavailable);
    };
    let engines = s.images.list();
    let engine = match b.engine.or(settings.engine) {
        Some(id) if engines.iter().any(|e| e.id == id) => id,
        None if engines.len() == 1 => engines[0].id.clone(),
        _ => return error(Error::Unavailable),
    };
    let prompt_settings = match s.images.prompt_settings(&engine) {
        Ok(v) => v,
        Err(_) => return error(Error::Unavailable),
    };
    let id = Uuid::new_v4().to_string();
    let job = match c.admit_agent(&session, s.now(), &b.request_id, &digest, &id, 4096) {
        Ok(v) => v,
        Err(e) => return app_error(e),
    };
    let _ = job;
    let job = c
        .transition(&session, s.now(), &id, JobState::Running)
        .expect("new job");
    let p = illustrated::JobProgress::new(session.clone(), d);
    let cancel = p.run.token();
    s.illustrated_jobs.lock().unwrap().insert(id.clone(), p);
    imports::publish(&s, &mut c, &session, &job);
    drop(c);
    let response = (StatusCode::ACCEPTED, Json(json!({"job":JobDto::from(job)}))).into_response();
    tokio::task::spawn_blocking(move || {
        let creation = illustrated::Creation {
            authentic: &*authentic,
            store: &*store,
            runner: &*runner,
            images: &s.images,
            profile,
            locale,
            engine,
            settings: prompt_settings,
            max_photos: settings.max_photos,
            date: d,
            id: Uuid::new_v4().to_string(),
            created_at: instant(),
            cancel: cancel.clone(),
        };
        let result = creation.execute(
            &mut || {
                let u = Uuid::new_v4();
                u32::from_le_bytes(u.as_bytes()[..4].try_into().unwrap())
            },
            &mut |photo, total| {
                let mut c = s.coordinator.lock().unwrap();
                if let Some(p) = s.illustrated_jobs.lock().unwrap().get_mut(&id) {
                    p.photo = photo;
                    p.total = total;
                }
                if let Ok(j) = c.touch(&session, s.now(), &id) {
                    imports::publish(&s, &mut c, &session, &j);
                }
            },
        );
        let mut c = s.coordinator.lock().unwrap();
        if let Some(p) = s.illustrated_jobs.lock().unwrap().get_mut(&id) {
            p.terminal = true;
            p.error = result.err();
        }
        let state = match result {
            Ok(()) => JobState::Succeeded,
            Err(Error::Cancelled) => JobState::Cancelled,
            Err(_) => JobState::Failed,
        };
        if let Ok(j) = c.transition(&session, s.now(), &id, state) {
            imports::publish(&s, &mut c, &session, &j);
        } else {
            // The lease ended while the worker was finishing; its reservation
            // is no longer needed and its progress belongs to the old lease.
            s.illustrated_jobs.lock().unwrap().remove(&id);
        }
    });
    response
}

pub fn valid_photo_query(path: &str, query: &str) -> bool {
    let Some(rest) = path.strip_prefix("/api/v1/illustrated/") else {
        return false;
    };
    let parts: Vec<_> = rest.split('/').collect();
    parts.len() == 3
        && DiaryDate::parse(parts[0]).is_some()
        && parts[1] == "photos"
        && parts[2].len() == 6
        && parts[2].ends_with(".png")
        && parts[2].as_bytes()[..2].iter().all(u8::is_ascii_digit)
        && query
            .strip_prefix("version=")
            .is_some_and(diary_domain::authentic::valid_uuid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{request, state};
    use axum::body::to_bytes;
    use tower::ServiceExt;
    const SESSION: &str = "07249746-e280-4d51-a86a-cbcb198ee101";
    const JOB: &str = "07249746-e280-4d51-a86a-cbcb198ee102";

    fn running(s: &ServerState) {
        let mut c = s.coordinator.lock().unwrap();
        c.open(SESSION, s.now()).unwrap();
        c.admit_agent(SESSION, s.now(), "create", &"a".repeat(64), JOB, 4096)
            .unwrap();
        c.transition(SESSION, s.now(), JOB, JobState::Running)
            .unwrap();
        s.illustrated_jobs.lock().unwrap().insert(
            JOB.into(),
            illustrated::JobProgress::new(SESSION.into(), DiaryDate::parse("2026-11-13").unwrap()),
        );
    }

    #[test]
    fn lease_cleanup_keeps_workers_reserved_and_cancels_them() {
        let s = state();
        running(&s);
        let mut finished =
            illustrated::JobProgress::new(SESSION.into(), DiaryDate::parse("2026-11-12").unwrap());
        finished.terminal = true;
        s.illustrated_jobs
            .lock()
            .unwrap()
            .insert("finished".into(), finished);
        s.lease_ended();
        let jobs = s.illustrated_jobs.lock().unwrap();
        assert_eq!(jobs.len(), 1);
        assert!(!jobs[JOB].terminal);
        assert!(jobs[JOB].run.token().is_cancelled());
    }

    #[tokio::test]
    async fn server_shutdown_waits_for_illustrated_worker_cleanup() {
        let s = state();
        running(&s);
        s.shutdown();
        assert!(
            s.images
                .shutdown_signal()
                .load(std::sync::atomic::Ordering::SeqCst)
        );
        assert!(
            s.illustrated_jobs.lock().unwrap()[JOB]
                .run
                .token()
                .is_cancelled()
        );
        assert!(
            tokio::time::timeout(Duration::from_millis(50), s.drain_illustrated())
                .await
                .is_err()
        );
        s.illustrated_jobs.lock().unwrap().remove(JOB);
        tokio::time::timeout(Duration::from_secs(1), s.drain_illustrated())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn cancellation_advances_sequence_and_publishes_once() {
        let s = state();
        running(&s);
        let before = s
            .coordinator
            .lock()
            .unwrap()
            .get(SESSION, s.now(), JOB)
            .unwrap();
        let cursor = s.coordinator.lock().unwrap().event_cursor();
        let app = router(s.clone());
        for _ in 0..2 {
            let mut req = request("POST", &format!("/api/v1/jobs/{JOB}/cancel"));
            req.headers_mut()
                .insert("x-diary-session", SESSION.parse().unwrap());
            let r = app.clone().oneshot(req).await.unwrap();
            assert_eq!(r.status(), StatusCode::OK);
            let v: Value =
                serde_json::from_slice(&to_bytes(r.into_body(), 16384).await.unwrap()).unwrap();
            assert_eq!(v["sequence"], before.sequence + 1);
            assert_eq!(v["phase"], "stopping");
        }
        assert!(s.coordinator.lock().unwrap().event_cursor() > cursor);
    }

    #[tokio::test]
    async fn close_signals_illustrated_worker_before_waiting_for_acknowledgement() {
        let s = state();
        running(&s);
        let worker = s.clone();
        let acknowledge = tokio::spawn(async move {
            tokio::time::timeout(Duration::from_secs(2), async {
                loop {
                    if worker.illustrated_jobs.lock().unwrap()[JOB]
                        .run
                        .token()
                        .is_cancelled()
                    {
                        worker
                            .illustrated_jobs
                            .lock()
                            .unwrap()
                            .get_mut(JOB)
                            .unwrap()
                            .terminal = true;
                        worker
                            .coordinator
                            .lock()
                            .unwrap()
                            .transition(SESSION, worker.now(), JOB, JobState::Cancelled)
                            .unwrap();
                        return;
                    }
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
            })
            .await
            .unwrap();
        });
        let mut req = request("DELETE", &format!("/api/v1/sessions/{SESSION}"));
        req.headers_mut()
            .insert("x-diary-session", SESSION.parse().unwrap());
        let response = tokio::time::timeout(Duration::from_secs(3), router(s).oneshot(req))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        acknowledge.await.unwrap();
    }
    #[test]
    fn photo_queries_are_closed() {
        let p = "/api/v1/illustrated/2026-11-13/photos/01.png";
        let q = "version=07249746-e280-4d51-a86a-cbcb198ee101";
        assert!(valid_photo_query(p, q));
        for bad in [
            format!("{q}&extra=x"),
            "version=../x".into(),
            "token=secret".into(),
        ] {
            assert!(!valid_photo_query(p, &bad));
        }
        assert!(!valid_photo_query("/api/v1/illustrated/2026-11-13", q));
    }
    #[tokio::test]
    async fn every_resource_requires_an_authorized_session() {
        for (method, path) in [
            ("GET", "/api/v1/illustrated"),
            ("GET", "/api/v1/illustrated/2026-11-13"),
            ("POST", "/api/v1/illustrated/2026-11-13"),
            ("DELETE", "/api/v1/illustrated/2026-11-13"),
            (
                "GET",
                "/api/v1/illustrated/2026-11-13/photos/01.png?version=07249746-e280-4d51-a86a-cbcb198ee101",
            ),
        ] {
            let response = router(state())
                .oneshot(request(method, path))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        }
    }
}

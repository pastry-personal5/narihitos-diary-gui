//! Authenticated, content-free image engine discovery; probes never render.
use super::{ServerState, failure, imports};
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use diary_application::image::{Availability, EngineView};
use serde_json::json;
use std::sync::Arc;
fn view(v: EngineView) -> serde_json::Value {
    let status = match v.status {
        Availability::Unchecked => "unchecked",
        Availability::Ready => "ready",
        Availability::Unavailable => "unavailable",
        Availability::Busy => "busy",
    };
    json!({"id":v.id,"label":v.label,"status":status,"reason":if v.status==Availability::Unavailable {Some("prerequisites_unavailable")} else {None}})
}
pub async fn list(State(s): State<Arc<ServerState>>, headers: HeaderMap) -> Response {
    if let Err(r) = imports::authorize(&s, &headers) {
        return r;
    }
    let images = s.images.clone();
    match tokio::task::spawn_blocking(move || images.list()).await {
        Ok(v) => Json(json!({"image_engines":v.into_iter().map(view).collect::<Vec<_>>()}))
            .into_response(),
        Err(_) => failure(StatusCode::INTERNAL_SERVER_ERROR, "internal"),
    }
}
pub async fn check(
    State(s): State<Arc<ServerState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    if let Err(r) = imports::authorize(&s, &headers) {
        return r;
    }
    let images = s.images.clone();
    match tokio::task::spawn_blocking(move || images.check(&id)).await {
        Ok(Ok(Some(v))) => Json(view(v)).into_response(),
        Ok(Ok(None)) => failure(StatusCode::NOT_FOUND, "not_found"),
        Ok(Err(_)) => failure(StatusCode::CONFLICT, "busy"),
        Err(_) => failure(StatusCode::INTERNAL_SERVER_ERROR, "internal"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        router,
        tests::{request, state},
    };
    use axum::body::to_bytes;
    use diary_application::image::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tower::ServiceExt;
    struct Fake(AtomicUsize);
    impl ImageEngine for Fake {
        fn check(&self) -> Availability {
            self.0.fetch_add(1, Ordering::SeqCst);
            Availability::Ready
        }
        fn render(&self, _: &RenderRequest) -> Result<Rendered, EngineError> {
            panic!("status routes must never render")
        }
    }
    #[tokio::test]
    async fn discovery_is_authenticated_cached_and_probe_only() {
        let mut s = state();
        let registry = Arc::new(Registry::default());
        let fake = Arc::new(Fake(AtomicUsize::new(0)));
        registry
            .replace(vec![("local".into(), "Local".into(), fake.clone())])
            .unwrap();
        ServerState::set_images(&mut s, registry);
        let session = "07249746-e280-4d51-a86a-cbcb198ee101";
        s.coordinator
            .lock()
            .unwrap()
            .open(session, s.now())
            .unwrap();
        for (method, path, auth, expected) in [
            (
                "GET",
                "/api/v1/image-engines",
                false,
                StatusCode::UNAUTHORIZED,
            ),
            ("GET", "/api/v1/image-engines", true, StatusCode::OK),
            (
                "POST",
                "/api/v1/image-engines/local/check",
                false,
                StatusCode::UNAUTHORIZED,
            ),
            (
                "POST",
                "/api/v1/image-engines/missing/check",
                true,
                StatusCode::NOT_FOUND,
            ),
            (
                "POST",
                "/api/v1/image-engines/local/check",
                true,
                StatusCode::OK,
            ),
        ] {
            let mut r = request(method, path);
            if auth {
                r.headers_mut()
                    .insert("x-diary-session", session.parse().unwrap());
            }
            let response = router(s.clone()).oneshot(r).await.unwrap();
            assert_eq!(response.status(), expected);
            let body = to_bytes(response.into_body(), 65536).await.unwrap();
            assert!(!String::from_utf8_lossy(&body).contains("/bin/"));
            if method == "GET" {
                assert_eq!(fake.0.load(Ordering::SeqCst), 0);
            }
        }
        assert_eq!(fake.0.load(Ordering::SeqCst), 1);
    }
}

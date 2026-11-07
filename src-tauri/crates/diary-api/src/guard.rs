//! Request tracing, bearer/loopback admission and the shared error responses.
use super::*;

/// Correlates one request across client and server logs. A missing or invalid caller value
/// is replaced, never trusted; the ID is echoed so a caller can quote it when debugging.
pub(crate) async fn trace(mut request: Request, next: Next) -> Response {
    let id = header(request.headers(), "x-diary-trace-id")
        .and_then(TraceId::parse)
        .unwrap_or_else(|| {
            TraceId::parse(&Uuid::new_v4().simple().to_string()).expect("32 lowercase hex")
        });
    request.extensions_mut().insert(id.clone());
    let mut response = next.run(request).await;
    if let Ok(value) = HeaderValue::from_str(id.as_str()) {
        response.headers_mut().insert("x-diary-trace-id", value);
    }
    response
}
pub(crate) fn traced(code: EventCode, level: Level, trace: &TraceId) -> Record {
    Record::new(code, level).trace(trace.clone())
}
pub(crate) fn in_session(record: Record, session: &str) -> Record {
    match OpaqueId::parse(session) {
        Some(id) => record.session(id),
        None => record,
    }
}
/// Access-boundary rejection: logged by code only, never with the token, headers or body.
pub(crate) fn rejected(
    state: &ServerState,
    trace: &Option<TraceId>,
    status: StatusCode,
    code: &'static str,
) -> Response {
    let tag = match code {
        "server_stopping" | "request_timeout" => ErrorTag::Unavailable,
        "request_limit" => ErrorTag::Limit,
        "unauthorized" | "invalid_origin" | "invalid_destination" => ErrorTag::Denied,
        _ => ErrorTag::Invalid,
    };
    let mut record = Record::new(EventCode::AuthRejected, Level::Warn).error(tag);
    record.trace = trace.clone();
    state.log(record);
    failure(status, code)
}

pub(crate) fn failure(status: StatusCode, code: &'static str) -> Response {
    (
        status,
        Json(json!({"error":{"code":code,"message":code,"retryable":false}})),
    )
        .into_response()
}
pub(crate) fn app_error(error: Error) -> Response {
    let (status, code) = match error {
        Error::ClientBusy => (StatusCode::CONFLICT, "client_busy"),
        Error::Busy => (StatusCode::CONFLICT, "busy"),
        Error::SessionExpired => (StatusCode::UNAUTHORIZED, "session_expired"),
        Error::NotFound => (StatusCode::NOT_FOUND, "not_found"),
        Error::Limit => (StatusCode::TOO_MANY_REQUESTS, "limit"),
        Error::RequestMismatch => (StatusCode::CONFLICT, "request_mismatch"),
        Error::InvalidInput => (StatusCode::BAD_REQUEST, "invalid_input"),
        Error::InvalidTransition => (StatusCode::CONFLICT, "invalid_transition"),
    };
    failure(status, code)
}
pub(crate) fn header<'a>(headers: &'a HeaderMap, key: &str) -> Option<&'a str> {
    if headers.get_all(key).iter().count() != 1 {
        return None;
    }
    headers.get(key)?.to_str().ok()
}
pub(crate) async fn authenticate(
    State(state): State<Arc<ServerState>>,
    request: Request,
    next: Next,
) -> Response {
    let trace = request.extensions().get::<TraceId>().cloned();
    if state.stopping.load(std::sync::atomic::Ordering::Acquire) {
        return rejected(
            &state,
            &trace,
            StatusCode::SERVICE_UNAVAILABLE,
            "server_stopping",
        );
    }
    let headers = request.headers();
    let peer_ok = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .is_some_and(|p| p.0.ip() == std::net::Ipv4Addr::LOCALHOST);
    let token = header(headers, "authorization")
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");
    let token_ok: bool = token.as_bytes().ct_eq(state.token.as_bytes()).into();
    if !peer_ok || !token_ok {
        return rejected(&state, &trace, StatusCode::UNAUTHORIZED, "unauthorized");
    }
    if header(headers, "host") != Some(state.host.as_str())
        || request.uri().scheme().is_some()
        || request.uri().authority().is_some()
        || request.uri().query().is_some_and(|query| {
            if request.uri().path() == "/api/v1/calendar" {
                !calendar::valid_query(query)
            } else if request
                .uri()
                .path()
                .strip_prefix("/api/v1/sources/")
                .is_some_and(|id| !id.contains('/'))
            {
                !imports::valid_read_query(query)
            } else if request.uri().path() == "/api/v1/sources" {
                !imports::valid_query(query)
            } else if request.uri().path() == "/api/v1/operations" {
                !source_trash::valid_query(query)
            } else {
                !matches!(request.uri().path(), "/api/v1/jobs" | "/api/v1/authentic")
                    || !query.strip_prefix("cursor=").is_some_and(|cursor| {
                        !cursor.is_empty()
                            && cursor.len() <= 64
                            && cursor
                                .bytes()
                                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
                    })
            }
        })
        || headers.contains_key("forwarded")
        || headers
            .keys()
            .any(|k| k.as_str().starts_with("x-forwarded-"))
    {
        return rejected(&state, &trace, StatusCode::FORBIDDEN, "invalid_destination");
    }
    if headers.contains_key("origin") {
        let origin = header(headers, "origin");
        if origin != Some("tauri://localhost")
            && !(state.development && origin == Some("http://localhost:1420"))
        {
            return rejected(&state, &trace, StatusCode::FORBIDDEN, "invalid_origin");
        }
    }
    if headers.contains_key("content-type")
        && header(headers, "content-type") != Some("application/json")
    {
        return rejected(
            &state,
            &trace,
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "unsupported_media_type",
        );
    }
    let (parts, body) = request.into_parts();
    let bytes = match tokio::time::timeout(
        Duration::from_secs(5),
        axum::body::to_bytes(body, 10 * 1024 * 1024),
    )
    .await
    {
        Err(_) => {
            return rejected(
                &state,
                &trace,
                StatusCode::REQUEST_TIMEOUT,
                "request_timeout",
            );
        }
        Ok(Err(_)) => {
            return rejected(
                &state,
                &trace,
                StatusCode::PAYLOAD_TOO_LARGE,
                "request_limit",
            );
        }
        Ok(Ok(bytes)) => bytes,
    };
    next.run(Request::from_parts(parts, axum::body::Body::from(bytes)))
        .await
}

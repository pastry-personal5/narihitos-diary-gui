use super::*;
use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    os::unix::fs::PermissionsExt,
    thread,
};
fn fixture(
    response: impl FnOnce(u16) -> String + Send + 'static,
) -> (tempfile::TempDir, Bridge, thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let task = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(3)))
            .unwrap();
        let mut request = Vec::new();
        let mut buffer = [0u8; 1024];
        while !request.windows(4).any(|w| w == b"\r\n\r\n") {
            let size = stream.read(&mut buffer).unwrap();
            if size == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..size]);
        }
        let _ = stream.write_all(response(port).as_bytes());
        String::from_utf8(request).unwrap()
    });
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let path = root.join("discovery.json");
    let record = serde_json::json!({"schema_version":1,"instance_id":"07249746-e280-4d51-a86a-cbcb198ee101","pid":1,"workspace_key":"b".repeat(64),"api_version":1,"origin":format!("http://127.0.0.1:{port}"),"token":"a".repeat(64)});
    fs::write(&path, serde_json::to_vec(&record).unwrap()).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    (temp, Bridge::new(path).unwrap(), task)
}
fn status() -> serde_json::Value {
    serde_json::json!({"instance_id":"07249746-e280-4d51-a86a-cbcb198ee101","workspace_key":"b".repeat(64),"server_version":"1.0.0","api_version":1,"api_minor":0,"storage_ready":false,"capabilities":["status","sessions","jobs","events"]})
}
fn json_response(value: serde_json::Value) -> String {
    let body = value.to_string();
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}
#[tokio::test]
async fn real_handshake_is_authenticated_and_bound_to_discovery() {
    let (_temp, bridge, task) = fixture(|_| json_response(status()));
    assert!(bridge.connect().await.is_ok());
    let request = task.join().unwrap().to_lowercase();
    assert!(request.starts_with("get /api/v1/status http/1.1\r\n"));
    assert!(request.contains(&format!("authorization: bearer {}", "a".repeat(64))));
    assert!(request.contains("host: 127.0.0.1:"));
    assert!(!request.contains("?token="));
}
#[tokio::test]
async fn incompatible_handshakes_clear_credentials_before_session_admission() {
    for key in [
        "instance_id",
        "workspace_key",
        "api_version",
        "capabilities",
    ] {
        let key = key.to_owned();
        let (_temp, bridge, task) = fixture(move |_| {
            let mut value = status();
            value[&key] = match key.as_str() {
                "api_version" => serde_json::json!(2),
                "capabilities" => serde_json::json!(["status"]),
                _ => serde_json::json!("other"),
            };
            json_response(value)
        });
        assert!(bridge.connect().await.is_err());
        task.join().unwrap();
        let c = bridge.connection.lock().await;
        assert!(c.discovery.is_none() && c.session.is_none());
    }
}
#[tokio::test]
async fn redirects_wrong_media_and_oversize_are_rejected() {
    for response in ["HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/credential-trap\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_owned(),"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}".to_owned(),"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 10485761\r\nConnection: close\r\n\r\n".to_owned()]{
        let (_temp,bridge,task)=fixture(move|_|response);assert!(bridge.connect().await.is_err());task.join().unwrap();
    }
}
#[test]
fn hostile_proxy_environment_cannot_redirect_loopback_handshake() {
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "bridge::tests::real_handshake_is_authenticated_and_bound_to_discovery",
            "--nocapture",
        ])
        .env("HTTP_PROXY", "http://127.0.0.1:1")
        .env("http_proxy", "http://127.0.0.1:1")
        .env("ALL_PROXY", "http://127.0.0.1:1")
        .env("all_proxy", "http://127.0.0.1:1")
        .env("NO_PROXY", "")
        .env("no_proxy", "")
        .status()
        .unwrap();
    assert!(status.success());
}

#[tokio::test]
async fn streamed_response_limit_applies_without_content_length() {
    let (_temp, bridge, task) = fixture(|_| {
        let body = "x".repeat(10 * 1024 * 1024 + 1);
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n{:x}\r\n{}\r\n0\r\n\r\n",
            body.len(),
            body
        )
    });
    assert_eq!(bridge.connect().await.err(), Some("response_limit"));
    task.join().unwrap();
}
#[tokio::test]
async fn replacing_discovery_with_invalid_bytes_clears_a_previous_connection() {
    let (_temp, bridge, task) = fixture(|_| json_response(status()));
    assert!(bridge.connect().await.is_ok());
    task.join().unwrap();
    fs::write(&bridge.path, b"invalid discovery").unwrap();
    assert_eq!(bridge.connect().await.err(), Some("invalid_discovery"));
    let c = bridge.connection.lock().await;
    assert!(c.discovery.is_none() && c.session.is_none());
}
#[tokio::test]
async fn redirect_is_rejected_before_any_followup_destination_request() {
    let (_temp, bridge, task) = fixture(|_| {
        "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/credential-trap\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into()
    });
    assert_eq!(
        bridge.connect().await.err(),
        Some("server_rejected_request")
    );
    task.join().unwrap();
}

#[tokio::test]
async fn busy_error_is_allowlisted_without_forwarding_private_error_details() {
    let (_temp, bridge, task) = fixture(|_| {
        let body = r#"{"error":{"code":"client_busy","message":"invented private path/token","details":{"path":"/private/invented"}}}"#;
        format!(
            "HTTP/1.1 409 Conflict\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    });
    assert_eq!(bridge.connect().await.err(), Some("client_busy"));
    assert_eq!(
        bridge.log().buffered(),
        1,
        "a busy rejection leaves one traced record"
    );
    task.join().unwrap();
}

#[tokio::test]
async fn every_request_carries_a_fresh_lowercase_hex_trace_id() {
    let (_temp, bridge, task) = fixture(|_| json_response(status()));
    assert!(bridge.connect().await.is_ok());
    let request = task.join().unwrap().to_lowercase();
    let at = request.find("x-diary-trace-id: ").expect("trace header") + "x-diary-trace-id: ".len();
    let id = &request[at..at + 32];
    assert!(
        id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')),
        "{id}"
    );
    assert_eq!(request.as_bytes()[at + 32], b'\r');
}

#[test]
fn only_unexpected_statuses_are_logged_and_tagged_by_kind() {
    use reqwest::StatusCode as S;
    assert_eq!(status_tag(S::CONFLICT), None);
    assert_eq!(status_tag(S::TOO_MANY_REQUESTS), Some(ErrorTag::Limit));
    assert_eq!(
        status_tag(S::SERVICE_UNAVAILABLE),
        Some(ErrorTag::Unavailable)
    );
    assert_eq!(
        status_tag(S::INTERNAL_SERVER_ERROR),
        Some(ErrorTag::Internal)
    );
    assert_eq!(status_tag(S::FORBIDDEN), Some(ErrorTag::Denied));
    assert_eq!(status_tag(S::UNAUTHORIZED), Some(ErrorTag::Denied));
    assert_eq!(status_tag(S::BAD_REQUEST), Some(ErrorTag::Invalid));
}

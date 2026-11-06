use super::*;
use crate::bridge::Bridge;
use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    os::unix::fs::PermissionsExt,
    thread,
};

const OP: &str = "07249746-e280-4d51-a86a-cbcb198ee101";

pub(crate) fn http(status: &str, value: serde_json::Value) -> String {
    let body = value.to_string();
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}
/// Serves the handshake, then one scripted response per later request; returns the raw requests.
pub(crate) fn fixture(
    replies: Vec<String>,
) -> (tempfile::TempDir, Bridge, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let status = serde_json::json!({"instance_id":OP,"workspace_key":"b".repeat(64),"server_version":"1.0.0","api_version":1,"api_minor":0,"storage_ready":true,"capabilities":["status","sessions","jobs","events"]});
    let mut all = vec![http("200 OK", status)];
    all.extend(replies);
    let task = thread::spawn(move || {
        let mut requests = Vec::new();
        for reply in all {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(3)))
                .unwrap();
            let mut request = Vec::new();
            let mut buffer = [0u8; 4096];
            loop {
                let size = stream.read(&mut buffer).unwrap_or(0);
                request.extend_from_slice(&buffer[..size]);
                let text = String::from_utf8_lossy(&request).to_string();
                let Some((head, rest)) = text.split_once("\r\n\r\n") else {
                    if size == 0 {
                        break;
                    }
                    continue;
                };
                let length = head
                    .to_lowercase()
                    .lines()
                    .find_map(|l| {
                        l.strip_prefix("content-length: ")
                            .map(|v| v.parse().unwrap())
                    })
                    .unwrap_or(0usize);
                if rest.len() >= length || size == 0 {
                    break;
                }
            }
            let _ = stream.write_all(reply.as_bytes());
            requests.push(String::from_utf8(request).unwrap());
        }
        requests
    });
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let path = root.join("discovery.json");
    let record = serde_json::json!({"schema_version":1,"instance_id":OP,"pid":1,"workspace_key":"b".repeat(64),"api_version":1,"origin":format!("http://127.0.0.1:{port}"),"token":"a".repeat(64)});
    fs::write(&path, serde_json::to_vec(&record).unwrap()).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    (temp, Bridge::new(path).unwrap(), task)
}
fn rev() -> Revision {
    Revision {
        sequence: 2,
        sha256: "c".repeat(64),
    }
}

#[tokio::test]
async fn invalid_input_is_rejected_before_any_connection() {
    let (_t, bridge, _task) = fixture(vec![]);
    let bad =
        |code: Result<(), AuthenticError>| assert_eq!(code.unwrap_err().code, "invalid_input");
    bad(bridge.load_authentic("2026-1-05".into()).await.map(|_| ()));
    bad(bridge.list_authentic(Some("x".into())).await.map(|_| ()));
    bad(bridge.get_operation("a/b".into()).await.map(|_| ()));
    bad(bridge
        .create_empty_authentic("2026-11-05".into(), "".into())
        .await
        .map(|_| ()));
    let save = |op: &str, expected, body: String| {
        bridge.save_authentic("2026-11-05".into(), op.into(), expected, body, 1)
    };
    bad(save("a/b", None, String::new()).await.map(|_| ()));
    bad(save(OP, None, "x".repeat(1024 * 1024 + 1))
        .await
        .map(|_| ()));
    bad(save(
        OP,
        Some(Revision {
            sequence: 0,
            sha256: "c".repeat(64),
        }),
        String::new(),
    )
    .await
    .map(|_| ()));
    bad(save(
        OP,
        Some(Revision {
            sequence: 1,
            sha256: "zz".into(),
        }),
        String::new(),
    )
    .await
    .map(|_| ()));
    // A well-formed request reaches the connection check instead.
    assert_eq!(
        save(OP, None, String::new()).await.unwrap_err().code,
        "disconnected"
    );
}

#[tokio::test]
async fn save_sends_the_fixed_route_and_original_operation_id() {
    let ack = serde_json::json!({"operation_id":OP,"revision":{"sequence":3,"sha256":"d".repeat(64)},"buffer_generation":7,"private":"dropped"});
    let (_t, bridge, task) = fixture(vec![http("200 OK", ack)]);
    bridge.connect().await.unwrap();
    let out = bridge
        .save_authentic(
            "2026-11-05".into(),
            OP.into(),
            Some(rev()),
            "hello".into(),
            7,
        )
        .await
        .unwrap();
    assert_eq!((out.revision.sequence, out.buffer_generation), (3, 7));
    assert!(!serde_json::to_string(&out).unwrap().contains("private"));
    let requests = task.join().unwrap();
    let request = &requests[1];
    assert!(request.starts_with("PUT /api/v1/authentic/2026-11-05 HTTP/1.1"));
    let body: serde_json::Value =
        serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
    assert_eq!(body["operation_id"], OP);
    assert_eq!(body["expected_revision"]["sequence"], 2);
    assert_eq!(body["body"], "hello");
    assert_eq!(body["buffer_generation"], 7);
}

#[tokio::test]
async fn server_errors_become_allowlisted_codes_without_server_text() {
    let leak = "/Users/me/diary/secret body text";
    let (_t, bridge, _task) = fixture(vec![
        http(
            "409 Conflict",
            serde_json::json!({"error":{"code":"revision_conflict","message":leak}}),
        ),
        http(
            "500 Internal Server Error",
            serde_json::json!({"error":{"code":"made_up","message":leak}}),
        ),
        http(
            "503 Service Unavailable",
            serde_json::json!({"error":{"code":"outcome_uncertain","message":leak}}),
        ),
    ]);
    bridge.connect().await.unwrap();
    let a = bridge
        .load_authentic("2026-11-05".into())
        .await
        .unwrap_err();
    let b = bridge
        .load_authentic("2026-11-05".into())
        .await
        .unwrap_err();
    let c = bridge.get_operation(OP.into()).await.unwrap_err();
    assert_eq!((a.code, a.retryable), ("revision_conflict", false));
    assert_eq!(b.code, "unavailable");
    assert_eq!((c.code, c.retryable), ("outcome_uncertain", true));
    for e in [a, b, c] {
        assert!(!serde_json::to_string(&e).unwrap().contains("secret"));
    }
}

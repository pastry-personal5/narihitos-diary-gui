//! E12 isolation: a full queue and a blocked log writer must not slow any request.
use diary_application::diagnostics::{Diagnostics, EventCode, Level, Record, Role};
use diary_logging::{clock::SystemClock, worker::LogWorker};
use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

/// A writer that blocks until the test clears `blocked`.
struct Gated(Arc<AtomicBool>);
impl Write for Gated {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        while self.0.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(5));
        }
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn complete(response: &str) -> bool {
    let Some((head, body)) = response.split_once("\r\n\r\n") else {
        return false;
    };
    head.to_ascii_lowercase()
        .lines()
        .find_map(|l| {
            l.strip_prefix("content-length: ")?
                .trim()
                .parse::<usize>()
                .ok()
        })
        .is_some_and(|n| body.len() >= n)
}

fn http(port: u16, method: &str, path: &str, token: &str, body: &str) -> String {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    write!(
        stream,
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nAuthorization: Bearer {token}\r\n\
         Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .unwrap();
    let mut out = String::new();
    // The server may reject a request without reading its body and close the socket; that can
    // reset the connection after a complete response was sent, so judge the bytes received.
    if let Err(e) = stream.read_to_string(&mut out) {
        assert!(complete(&out), "{method} {path}: {e}; partial {out:?}");
    }
    out
}

#[tokio::test(flavor = "multi_thread")]
async fn http_status_sessions_and_rejections_are_unaffected_by_stalled_diagnostics() {
    let blocked = Arc::new(AtomicBool::new(true));
    let worker = LogWorker::start(
        None,
        Role::Server,
        None,
        Arc::new(SystemClock),
        Box::new(Gated(blocked.clone())),
    );
    for _ in 0..1_000 {
        worker.emit(Record::new(EventCode::ServerStarted, Level::Info));
    }
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port();
    let token = "a".repeat(64);
    let mut state = diary_api::ServerState::new(
        token.clone(),
        port,
        "instance".into(),
        "workspace".into(),
        false,
    );
    diary_api::ServerState::set_diagnostics(&mut state, worker.clone());
    let app = diary_api::router(state);
    let server = tokio::spawn(std::future::IntoFuture::into_future(axum::serve(
        tokio::net::TcpListener::from_std(listener).unwrap(),
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )));
    let t = token.clone();
    let started = Instant::now();
    let replies = tokio::task::spawn_blocking(move || {
        [
            http(port, "GET", "/api/v1/status", &t, ""),
            http(port, "POST", "/api/v1/sessions", &t, "{}"),
            http(port, "POST", "/api/v1/sessions", "wrong", "{}"),
        ]
    })
    .await
    .unwrap();
    assert!(started.elapsed() < Duration::from_secs(3));
    assert!(replies[0].starts_with("HTTP/1.1 200"), "{}", replies[0]);
    assert!(replies[1].starts_with("HTTP/1.1 200"), "{}", replies[1]);
    assert!(replies[2].starts_with("HTTP/1.1 401"), "{}", replies[2]);
    server.abort();
    blocked.store(false, Ordering::Relaxed);
    worker.shutdown(Duration::from_secs(1));
}

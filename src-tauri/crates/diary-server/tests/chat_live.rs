//! Live end-to-end chat proof (P1-M21 C15): the real `diary-api` router, job layer and chat service
//! over a real loopback socket, driving the real adapter for one kind. Ignored by default, spends
//! account usage (Apple is on-device) and is run by the agent, never a human gate:
//!
//! `DIARY_CHAT_LIVE=<codex|claude|qwen|apple> [DIARY_CHAT_LIVE_HOME=<profile_home>] \
//!    cargo test -p diary-server --test chat_live -- --ignored --nocapture --test-threads=1`
//!
//! Two tests: one real chat turn with an unsaved-document snapshot (`live_chat_turn`) and one cancel
//! while the model is still working (`live_chat_cancel`). Inputs are synthetic. The client is plain
//! `std::net` HTTP so the test adds no dependency.
use diary_application::agent::AgentKind;
use diary_infrastructure::agent::{
    host::Host,
    process::Limits,
    profiles::{Entry, ProfileService, ProfileTable},
    registry::Agents,
};
use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

const TOKEN: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

/// A private scratch directory removed on drop (no dev-dependency for this).
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        use std::os::unix::fs::DirBuilderExt;
        let dir = std::env::temp_dir().join(format!("diary-chat-live-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
        Self(dir)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct Live {
    port: u16,
    session: String,
    _root: Scratch,
}

fn kind() -> Option<AgentKind> {
    match std::env::var("DIARY_CHAT_LIVE").ok()?.as_str() {
        "codex" => Some(AgentKind::Codex),
        "claude" => Some(AgentKind::Claude),
        "qwen" => Some(AgentKind::Qwen),
        "apple" => Some(AgentKind::AppleFoundationModels),
        _ => None,
    }
}

fn start() -> Option<Live> {
    let kind = kind().or_else(|| {
        eprintln!("DIARY_CHAT_LIVE not set to codex|claude|qwen|apple; skipping");
        None
    })?;
    let home = std::env::var_os("DIARY_CHAT_LIVE_HOME").map(PathBuf::from);
    let root = Scratch::new();
    let table = Arc::new(ProfileTable::from_entries(vec![Entry {
        alias: "live".into(),
        kind,
        home: if kind == AgentKind::AppleFoundationModels {
            None
        } else {
            Some(home.expect("DIARY_CHAT_LIVE_HOME"))
        },
        executable: None,
    }]));
    let host = Arc::new(Host::new(
        root.0.clone(),
        std::env::var_os("PATH").unwrap_or_default(),
        std::env::current_dir().unwrap(),
        Arc::new(diary_application::diagnostics::NullDiagnostics),
        Limits::production(),
        Limits::probe(),
    ));
    let agents = Arc::new(Agents::standard(table.clone(), host));
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port();
    let mut state = diary_api::ServerState::new(
        TOKEN.into(),
        port,
        "live-instance".into(),
        "live-workspace".into(),
        false,
    );
    diary_api::ServerState::set_agent_runner(&mut state, agents.clone());
    diary_api::ServerState::set_profiles(&mut state, Arc::new(ProfileService::new(table, agents)));
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            let listener = tokio::net::TcpListener::from_std(listener).unwrap();
            let app = diary_api::router(state);
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await
            .unwrap();
        });
    });
    let mut live = Live {
        port,
        session: String::new(),
        _root: root,
    };
    let (status, body) = live.http("POST", "/sessions", Some("{}"));
    assert_eq!(status, 200, "{body}");
    live.session = field(&body, "session_id").expect("session_id");
    Some(live)
}

impl Live {
    fn http(&self, method: &str, path: &str, body: Option<&str>) -> (u16, String) {
        let mut s = TcpStream::connect(("127.0.0.1", self.port)).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(30))).unwrap();
        let body = body.unwrap_or("");
        let session = if self.session.is_empty() {
            String::new()
        } else {
            format!("x-diary-session: {}\r\n", self.session)
        };
        write!(
            s,
            "{method} /api/v1{path} HTTP/1.1\r\nhost: 127.0.0.1:{}\r\nauthorization: Bearer {TOKEN}\r\n{session}content-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            self.port,
            body.len()
        )
        .unwrap();
        let mut raw = String::new();
        s.read_to_string(&mut raw).unwrap();
        let status = raw[9..12].parse().unwrap();
        let body = raw.split_once("\r\n\r\n").map_or("", |(_, b)| b);
        // The server may chunk small bodies; the JSON itself is what the fields are read from.
        (status, body.to_owned())
    }
    fn heartbeat(&self) {
        let _ = self.http(
            "POST",
            &format!("/sessions/{}/heartbeat", self.session),
            Some("{}"),
        );
    }
    /// Polls the chat until `done` says stop, keeping the lease alive. Returns the last body.
    fn wait(&self, chat: &str, limit: Duration, mut done: impl FnMut(&str) -> bool) -> String {
        let start = Instant::now();
        let mut last_beat = Instant::now();
        loop {
            let (_, body) = self.http("GET", &format!("/chats/{chat}"), None);
            if done(&body) {
                return body;
            }
            assert!(start.elapsed() < limit, "timed out; last body: {body}");
            if last_beat.elapsed() > Duration::from_secs(5) {
                self.heartbeat();
                last_beat = Instant::now();
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    fn open_chat(&self) -> String {
        let (status, body) = self.http(
            "POST",
            "/chats",
            Some(r#"{"profile":"live","locale":"en-US"}"#),
        );
        assert_eq!(
            status, 201,
            "open_chat refused (profile not ready?): {body}"
        );
        field(&body, "chat_id").unwrap()
    }
}

/// Reads `"key":"value"` out of a compact JSON body (the server emits no spaces).
fn field(body: &str, key: &str) -> Option<String> {
    let marker = format!("\"{key}\":\"");
    let start = body.find(&marker)? + marker.len();
    Some(body[start..start + body[start..].find('"')?].to_owned())
}

#[test]
#[ignore = "spends usage; set DIARY_CHAT_LIVE=<kind>"]
fn live_chat_turn() {
    let Some(live) = start() else { return };
    let chat = live.open_chat();
    let t = Instant::now();
    let (status, body) = live.http(
        "POST",
        &format!("/chats/{chat}/turns"),
        Some(
            r#"{"request_id":"live-1","message":"In one short sentence, say hello.","document":{"kind":"authentic","date":"2026-11-06","unsaved":true,"body":"I drank tea."}}"#,
        ),
    );
    assert_eq!(status, 202, "{body}");
    let mut saw_stage = false;
    let done = live.wait(&chat, Duration::from_secs(240), |b| {
        saw_stage |= !b.contains("\"stage\":null") && b.contains("\"stage\":");
        b.contains("\"state\":\"done\"") || b.contains("\"state\":\"failed\"")
    });
    eprintln!("chat turn: {:?}; stage observed: {saw_stage}", t.elapsed());
    assert!(done.contains("\"state\":\"done\""), "turn failed: {done}");
    assert!(done.contains("\"role\":\"assistant\""), "{done}");
    // A reload re-reads the same transcript within the lease.
    let (_, again) = live.http("GET", &format!("/chats/{chat}"), None);
    assert!(again.contains("\"role\":\"assistant\""));
}

#[test]
#[ignore = "spends usage; set DIARY_CHAT_LIVE=<kind>"]
fn live_chat_cancel() {
    let Some(live) = start() else { return };
    let chat = live.open_chat();
    let (status, body) = live.http(
        "POST",
        &format!("/chats/{chat}/turns"),
        Some(
            r#"{"request_id":"live-2","message":"Write a very long story about a dragon, 800 words.","document":null}"#,
        ),
    );
    assert_eq!(status, 202, "{body}");
    let job = field(&body, "id").expect("job id");
    std::thread::sleep(Duration::from_millis(1500));
    let t = Instant::now();
    let (status, cancelled) = live.http("POST", &format!("/jobs/{job}/cancel"), Some("{}"));
    assert_eq!(status, 200, "{cancelled}");
    let end = live.wait(&chat, Duration::from_secs(30), |b| {
        !b.contains("\"state\":\"running\"")
    });
    eprintln!("cancel settled in {:?}: {end}", t.elapsed());
    // A model that finished inside the 1.5 s window legitimately succeeded; after a committed
    // cancel a success must never reach the transcript.
    if end.contains("\"state\":\"cancelled\"") {
        assert!(end.contains("\"transcript\":[]"), "{end}");
    }
}

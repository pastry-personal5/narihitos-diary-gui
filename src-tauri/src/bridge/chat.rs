//! Transient AI chat. The server owns the transcript for the lease; the WebView sends the active
//! document snapshot with each turn and re-reads the chat after a reload. Requests are bounded
//! before they leave and responses are re-validated against closed vocabularies.
use super::validate::invalid;
use super::{AuthenticError, Bridge, Job};
use diary_application::agent::{
    AgentErrorCode, Locale, MAX_TEXT_BYTES, MAX_TRANSCRIPT_BYTES, MAX_TURNS, MAX_USER_MESSAGE_BYTES,
};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::json;

const STATES: [&str; 4] = ["running", "done", "failed", "cancelled"];
pub(super) const STAGES: [&str; 8] = [
    "preparing_context",
    "starting_agent",
    "waiting_for_output",
    "receiving_output",
    "validating_output",
    "between_candidates",
    "stopping",
    "finished",
];
const DOCUMENT_LIMIT: usize = 1024 * 1024;

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatOpened {
    pub chat_id: String,
    pub profile: String,
    pub locale: String,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub text: String,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct TurnView {
    pub job_id: String,
    pub state: String,
    pub error_code: Option<String>,
    pub stage: Option<String>,
    pub revision: u64,
    pub provisional: Option<String>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct ChatView {
    pub chat_id: String,
    pub profile: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub locale: String,
    pub transcript: Vec<ChatMessage>,
    pub turn: Option<TurnView>,
}
/// The active editor snapshot sent with one message.
#[derive(Debug, Serialize, Deserialize)]
pub struct ChatDocument {
    pub kind: String,
    pub date: String,
    pub unsaved: bool,
    pub body: String,
}
#[derive(Serialize, Deserialize)]
pub struct TurnAccepted {
    pub job: Job,
}

fn id_ok(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}
fn sound(v: &ChatView) -> bool {
    let turn = v.turn.as_ref().is_none_or(|t| {
        id_ok(&t.job_id)
            && STATES.contains(&t.state.as_str())
            && t.error_code
                .as_deref()
                .is_none_or(|c| AgentErrorCode::parse(c).is_some())
            && t.stage.as_deref().is_none_or(|s| STAGES.contains(&s))
            && t.provisional
                .as_ref()
                .is_none_or(|p| p.len() <= MAX_TEXT_BYTES)
    });
    let bytes: usize = v.transcript.iter().map(|m| m.text.len()).sum();
    id_ok(&v.chat_id)
        && super::profiles::valid_alias(&v.profile)
        && Locale::parse(&v.locale).is_ok()
        && v.transcript.len() <= MAX_TURNS + 2
        // A reply can exceed the held-transcript bound once; the next message is then refused.
        && bytes <= MAX_TRANSCRIPT_BYTES + MAX_TEXT_BYTES
        && v.transcript
            .iter()
            .all(|m| m.role == "user" || m.role == "assistant")
        && turn
}

impl Bridge {
    pub async fn open_chat(
        &self,
        profile: String,
        locale: String,
    ) -> Result<ChatOpened, AuthenticError> {
        self.open_chat_kind(profile, locale, "chat").await
    }
    pub async fn open_chat_kind(
        &self,
        profile: String,
        locale: String,
        purpose: &str,
    ) -> Result<ChatOpened, AuthenticError> {
        if !super::profiles::valid_alias(&profile)
            || Locale::parse(&locale).is_err()
            || !matches!(purpose, "chat" | "paragraph_review")
        {
            return Err(invalid());
        }
        let opened: ChatOpened = self
            .authentic(
                Method::POST,
                "/chats",
                Some(json!({"profile":profile,"locale":locale,"purpose":purpose})),
            )
            .await?;
        if !id_ok(&opened.chat_id) || opened.profile != profile {
            return Err(AuthenticError::new("unavailable"));
        }
        Ok(opened)
    }
    pub async fn send_chat(
        &self,
        chat_id: String,
        request_id: String,
        message: String,
        document: Option<ChatDocument>,
    ) -> Result<TurnAccepted, AuthenticError> {
        let doc_ok = document.as_ref().is_none_or(|d| {
            matches!(d.kind.as_str(), "authentic" | "source")
                && super::validate::date_shape(&d.date)
                && d.body.len() <= DOCUMENT_LIMIT
        });
        if !id_ok(&chat_id)
            || !id_ok(&request_id)
            || message.trim().is_empty()
            || message.len() > MAX_USER_MESSAGE_BYTES
            || !doc_ok
        {
            return Err(invalid());
        }
        self.authentic(
            Method::POST,
            &format!("/chats/{chat_id}/turns"),
            Some(json!({"request_id":request_id,"message":message,"document":document})),
        )
        .await
    }
    pub async fn get_chat(&self, chat_id: String) -> Result<ChatView, AuthenticError> {
        if !id_ok(&chat_id) {
            return Err(invalid());
        }
        let view: ChatView = self
            .authentic(Method::GET, &format!("/chats/{chat_id}"), None)
            .await?;
        if !sound(&view) || view.chat_id != chat_id {
            return Err(AuthenticError::new("unavailable"));
        }
        Ok(view)
    }
    pub async fn close_chat(&self, chat_id: String) -> Result<(), AuthenticError> {
        if !id_ok(&chat_id) {
            return Err(invalid());
        }
        let _: serde_json::Value = self
            .authentic(Method::DELETE, &format!("/chats/{chat_id}"), None)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::bridge::authentic::tests::{fixture, http};
    use serde_json::{Value, json};

    fn view(state: &str) -> Value {
        json!({"chat_id":"c1","profile":"main","type":"codex","locale":"en-US",
            "transcript":[{"role":"user","text":"hi"},{"role":"assistant","text":"yo"}],
            "turn":{"job_id":"j1","state":state,"error_code":null,"stage":"receiving_output",
                    "revision":3,"provisional":null}})
    }

    #[tokio::test]
    async fn chat_uses_the_fixed_routes_and_sends_the_snapshot() {
        let (_t, bridge, task) = fixture(vec![
            http(
                "201 Created",
                json!({"chat_id":"c1","profile":"main","locale":"en-US"}),
            ),
            http(
                "202 Accepted",
                json!({"job":{"id":"j1","sequence":2,"state":"running"}}),
            ),
            http("200 OK", view("running")),
            http("200 OK", json!({})),
        ]);
        bridge.connect().await.unwrap();
        bridge
            .open_chat("main".into(), "en-US".into())
            .await
            .unwrap();
        let doc = super::ChatDocument {
            kind: "authentic".into(),
            date: "2026-11-06".into(),
            unsaved: true,
            body: "dear diary".into(),
        };
        bridge
            .send_chat("c1".into(), "r1".into(), "hello".into(), Some(doc))
            .await
            .unwrap();
        assert_eq!(
            bridge.get_chat("c1".into()).await.unwrap().transcript.len(),
            2
        );
        bridge.close_chat("c1".into()).await.unwrap();
        let r = task.join().unwrap();
        assert!(r[1].starts_with("POST /api/v1/chats HTTP/1.1"));
        assert!(r[2].starts_with("POST /api/v1/chats/c1/turns HTTP/1.1"));
        assert!(r[2].contains("\"unsaved\":true"));
        assert!(r[3].starts_with("GET /api/v1/chats/c1 HTTP/1.1"));
        assert!(r[4].starts_with("DELETE /api/v1/chats/c1 HTTP/1.1"));
    }

    #[tokio::test]
    async fn bad_input_never_reaches_the_network() {
        let (_t, bridge, _task) = fixture(vec![]);
        assert_eq!(
            bridge
                .open_chat("a/b".into(), "en-US".into())
                .await
                .err()
                .unwrap()
                .code,
            "invalid_input"
        );
        assert_eq!(
            bridge
                .open_chat("main".into(), "not a locale".into())
                .await
                .err()
                .unwrap()
                .code,
            "invalid_input"
        );
        let big = "a".repeat(64 * 1024 + 1);
        assert_eq!(
            bridge
                .send_chat("c1".into(), "r1".into(), big, None)
                .await
                .err()
                .unwrap()
                .code,
            "invalid_input"
        );
        assert_eq!(
            bridge
                .send_chat("c1".into(), "r1".into(), "  ".into(), None)
                .await
                .err()
                .unwrap()
                .code,
            "invalid_input"
        );
        assert_eq!(
            bridge.get_chat("../x".into()).await.err().unwrap().code,
            "invalid_input"
        );
    }

    #[tokio::test]
    async fn out_of_vocabulary_responses_are_rejected() {
        let mut bad_state = view("running");
        bad_state["turn"]["state"] = json!("/Users/x/secret");
        let mut bad_code = view("failed");
        bad_code["turn"]["error_code"] = json!("native text");
        let mut bad_role = view("done");
        bad_role["transcript"][0]["role"] = json!("system");
        let (_t, bridge, _task) = fixture(vec![
            http("200 OK", bad_state),
            http("200 OK", bad_code),
            http("200 OK", bad_role),
        ]);
        bridge.connect().await.unwrap();
        for _ in 0..3 {
            assert_eq!(
                bridge.get_chat("c1".into()).await.err().unwrap().code,
                "unavailable"
            );
        }
    }

    #[tokio::test]
    async fn server_codes_for_chat_reach_the_webview() {
        let (_t, bridge, _task) = fixture(vec![
            http("409 Conflict", json!({"error":{"code":"busy"}})),
            http(
                "409 Conflict",
                json!({"error":{"code":"profile_not_ready"}}),
            ),
            http(
                "422 Unprocessable Entity",
                json!({"error":{"code":"context"}}),
            ),
        ]);
        bridge.connect().await.unwrap();
        let e = bridge
            .send_chat("c1".into(), "r1".into(), "hi".into(), None)
            .await
            .err()
            .unwrap();
        assert_eq!(e.code, "busy");
        assert_eq!(
            bridge
                .open_chat("main".into(), "en-US".into())
                .await
                .err()
                .unwrap()
                .code,
            "profile_not_ready"
        );
        let e = bridge
            .send_chat("c1".into(), "r2".into(), "hi".into(), None)
            .await
            .err()
            .unwrap();
        assert_eq!(e.code, "context");
    }
}

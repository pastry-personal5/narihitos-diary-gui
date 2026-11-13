//! Read-only agent profile status. Nothing here can start a run; responses are re-validated
//! against closed vocabularies before they reach the WebView.
use super::{AuthenticError, Bridge};
use reqwest::Method;
use serde::{Deserialize, Serialize};

const STATUSES: [&str; 9] = [
    "unchecked",
    "missing",
    "unsupported_version",
    "unavailable",
    "available_auth_unknown",
    "unauthenticated",
    "ready",
    "busy",
    "failed",
];
const REASONS: [&str; 3] = ["assets_missing", "assets_invalid", "adapter_unimplemented"];
const TYPES: [&str; 4] = ["codex", "claude", "qwen", "apple-foundation-models"];

#[derive(Debug, Serialize, Deserialize)]
pub struct ActionCapability {
    pub available: bool,
    pub reason: Option<String>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Capabilities {
    pub chat: ActionCapability,
    pub generate: ActionCapability,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub illustrated: Option<ActionCapability>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct ProfileStatus {
    pub alias: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub status: String,
    /// Closed reason behind the status; `null` when ready, unchecked or not attributed.
    #[serde(default)]
    pub reason: Option<String>,
    pub capabilities: Capabilities,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct ProfileList {
    pub profiles: Vec<ProfileStatus>,
}

pub(super) fn valid_alias(a: &str) -> bool {
    !a.is_empty()
        && a.len() <= 64
        && a.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}
fn sound(p: &ProfileStatus) -> bool {
    let cap = |c: &ActionCapability| {
        c.reason.as_deref().is_none_or(|r| REASONS.contains(&r))
            && (c.available == c.reason.is_none())
    };
    valid_alias(&p.alias)
        && TYPES.contains(&p.kind.as_str())
        && STATUSES.contains(&p.status.as_str())
        && p.reason.as_deref().is_none_or(|r| {
            diary_application::agent::Reason::parse(r).is_some_and(|r| r.is_api_visible())
        })
        && cap(&p.capabilities.chat)
        && cap(&p.capabilities.generate)
        && p.capabilities.illustrated.as_ref().is_none_or(cap)
}

impl Bridge {
    pub async fn list_profiles(&self) -> Result<ProfileList, AuthenticError> {
        let list: ProfileList = self.authentic(Method::GET, "/profiles", None).await?;
        if list.profiles.len() > 64 || !list.profiles.iter().all(sound) {
            return Err(AuthenticError::new("unavailable"));
        }
        Ok(list)
    }
    pub async fn check_profile(&self, alias: String) -> Result<ProfileStatus, AuthenticError> {
        if !valid_alias(&alias) {
            return Err(AuthenticError::new("invalid_input"));
        }
        let status: ProfileStatus = self
            .authentic(Method::POST, &format!("/profiles/{alias}/check"), None)
            .await?;
        if !sound(&status) || status.alias != alias {
            return Err(AuthenticError::new("unavailable"));
        }
        Ok(status)
    }
}

#[cfg(test)]
mod tests {
    use crate::bridge::authentic::tests::{fixture, http};
    use serde_json::json;

    fn profile(alias: &str, status: &str) -> serde_json::Value {
        json!({"alias":alias,"type":"codex","status":status,"reason":null,
            "capabilities":{"chat":{"available":true,"reason":null},
                            "generate":{"available":false,"reason":"assets_missing"}}})
    }

    #[tokio::test]
    async fn list_and_check_use_the_fixed_read_only_routes() {
        let (_t, bridge, task) = fixture(vec![
            http("200 OK", json!({"profiles":[profile("main","unchecked")]})),
            http("200 OK", profile("main", "ready")),
        ]);
        bridge.connect().await.unwrap();
        let list = bridge.list_profiles().await.unwrap();
        assert_eq!(list.profiles[0].status, "unchecked");
        assert_eq!(
            bridge.check_profile("main".into()).await.unwrap().status,
            "ready"
        );
        let requests = task.join().unwrap();
        assert!(requests[1].starts_with("GET /api/v1/profiles HTTP/1.1"));
        assert!(requests[2].starts_with("POST /api/v1/profiles/main/check HTTP/1.1"));
    }

    #[tokio::test]
    async fn reasons_are_checked_against_the_closed_api_set() {
        let mut ok = profile("main", "unavailable");
        ok["reason"] = json!("apple_intelligence_disabled");
        let mut run_only = profile("main", "failed");
        run_only["reason"] = json!("escaped_descendants");
        let mut text = profile("main", "failed");
        text["reason"] = json!("/Users/x/secret");
        let (_t, bridge, _task) = fixture(vec![
            http("200 OK", json!({"profiles":[ok]})),
            http("200 OK", json!({"profiles":[run_only]})),
            http("200 OK", json!({"profiles":[text]})),
        ]);
        bridge.connect().await.unwrap();
        let list = bridge.list_profiles().await.unwrap();
        assert_eq!(
            list.profiles[0].reason.as_deref(),
            Some("apple_intelligence_disabled")
        );
        for _ in 0..2 {
            assert_eq!(
                bridge.list_profiles().await.unwrap_err().code,
                "unavailable"
            );
        }
    }

    #[tokio::test]
    async fn invalid_alias_never_reaches_the_network() {
        let (_t, bridge, _task) = fixture(vec![]);
        for bad in ["", "a/b", "a b", "..", &"x".repeat(65)] {
            let e = bridge.check_profile(bad.into()).await.unwrap_err();
            assert_eq!(e.code, "invalid_input", "{bad:?}");
        }
    }

    #[tokio::test]
    async fn out_of_vocabulary_responses_are_rejected() {
        let (_t, bridge, _task) = fixture(vec![
            http(
                "200 OK",
                json!({"profiles":[profile("main","/Users/x/secret")]}),
            ),
            http("200 OK", profile("other", "ready")),
        ]);
        bridge.connect().await.unwrap();
        assert_eq!(
            bridge.list_profiles().await.unwrap_err().code,
            "unavailable"
        );
        // The echoed alias must match the one asked for.
        assert_eq!(
            bridge.check_profile("main".into()).await.unwrap_err().code,
            "unavailable"
        );
    }
}

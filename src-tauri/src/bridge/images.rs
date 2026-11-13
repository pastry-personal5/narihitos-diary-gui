//! Content-free image-engine discovery and explicit probes; these commands never render.
use super::{AuthenticError, Bridge, valid_id};
use reqwest::Method;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageEngine {
    pub id: String,
    pub label: String,
    pub status: String,
    pub reason: Option<String>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageEngines {
    pub image_engines: Vec<ImageEngine>,
    pub illustrated_engine: Option<String>,
    pub illustrated_available: bool,
}
fn sound(e: &ImageEngine) -> bool {
    valid_id(&e.id)
        && !e.label.trim().is_empty()
        && e.label.len() <= 256
        && !e.label.chars().any(char::is_control)
        && ["unchecked", "ready", "unavailable", "busy"].contains(&e.status.as_str())
        && e.reason.as_deref()
            == if e.status == "unavailable" {
                Some("prerequisites_unavailable")
            } else {
                None
            }
}
impl Bridge {
    pub async fn list_image_engines(&self) -> Result<ImageEngines, AuthenticError> {
        let v: ImageEngines = self.authentic(Method::GET, "/image-engines", None).await?;
        let ids: std::collections::HashSet<_> =
            v.image_engines.iter().map(|e| e.id.as_str()).collect();
        if v.image_engines.len() > 64
            || !v.image_engines.iter().all(sound)
            || ids.len() != v.image_engines.len()
            || v.illustrated_engine
                .as_deref()
                .is_some_and(|id| !ids.contains(id))
            || (v.illustrated_available && v.illustrated_engine.is_none())
        {
            return Err(AuthenticError::new("invalid_response"));
        }
        Ok(v)
    }
    pub async fn check_image_engine(&self, id: String) -> Result<ImageEngine, AuthenticError> {
        if !valid_id(&id) {
            return Err(AuthenticError::new("invalid_input"));
        }
        let v: ImageEngine = self
            .authentic(Method::POST, &format!("/image-engines/{id}/check"), None)
            .await?;
        if !sound(&v) || v.id != id {
            return Err(AuthenticError::new("invalid_response"));
        }
        Ok(v)
    }
}
#[cfg(test)]
mod tests {
    use crate::bridge::authentic::tests::{fixture, http};
    use serde_json::json;
    #[tokio::test]
    async fn probes_use_fixed_routes_and_validate_selection_and_identity() {
        let engine = json!({"id":"local","label":"Local","status":"ready","reason":null});
        let (_t, b, task) = fixture(vec![
            http(
                "200 OK",
                json!({"image_engines":[engine.clone()],"illustrated_engine":"local","illustrated_available":true}),
            ),
            http("200 OK", engine.clone()),
            http(
                "200 OK",
                json!({"image_engines":[engine.clone()],"illustrated_engine":"other","illustrated_available":true}),
            ),
            http("200 OK", engine),
        ]);
        b.connect().await.unwrap();
        assert!(b.list_image_engines().await.unwrap().illustrated_available);
        assert_eq!(
            b.check_image_engine("local".into()).await.unwrap().status,
            "ready"
        );
        assert!(b.list_image_engines().await.is_err());
        assert!(b.check_image_engine("other".into()).await.is_err());
        assert!(b.check_image_engine("../local".into()).await.is_err());
        let requests = task.join().unwrap();
        assert!(requests[1].starts_with("GET /api/v1/image-engines HTTP/1.1"));
        assert!(requests[2].starts_with("POST /api/v1/image-engines/local/check HTTP/1.1"));
    }
}

//! Fixed illustrated commands; only validated dates, versions and photo names enter routes.
use super::validate::{date_shape, invalid};
use super::{AuthenticError, Bridge, TurnAccepted};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::json;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IllustratedList {
    pub dates: Vec<String>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Photo {
    pub file: String,
    pub seed: u32,
    pub prompt: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IllustratedEntry {
    pub date: String,
    pub version: String,
    pub markdown: String,
    pub source_authentic_id: String,
    pub source_revision: u64,
    pub source_sha256: String,
    pub agent_profile: String,
    pub image_engine: String,
    pub theme_count: usize,
    pub photos: Vec<Photo>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IllustratedInput {
    pub request_id: String,
    pub profile: String,
    pub locale: String,
    pub engine: Option<String>,
}
impl Bridge {
    pub async fn list_illustrated(&self) -> Result<IllustratedList, AuthenticError> {
        let v: IllustratedList = self.authentic(Method::GET, "/illustrated", None).await?;
        if v.dates.iter().any(|d| !date_shape(d)) || v.dates.windows(2).any(|v| v[0] >= v[1]) {
            return Err(invalid());
        }
        Ok(v)
    }
    pub async fn read_illustrated(&self, date: String) -> Result<IllustratedEntry, AuthenticError> {
        if !date_shape(&date) {
            return Err(invalid());
        }
        let v: IllustratedEntry = self
            .authentic(Method::GET, &format!("/illustrated/{date}"), None)
            .await?;
        if v.date != date
            || !diary_application::diagnostics::valid_uuid(&v.version)
            || !diary_application::diagnostics::valid_uuid(&v.source_authentic_id)
            || v.source_revision == 0
            || v.source_sha256.len() != 64
            || !v.source_sha256.bytes().all(|b| b.is_ascii_hexdigit())
            || v.markdown.len() > 1024 * 1024
            || v.photos.is_empty()
            || v.photos.len() > 99
            || v.theme_count < v.photos.len()
            || v.theme_count > 1024
            || !super::valid_id(&v.agent_profile)
            || !super::valid_id(&v.image_engine)
            || v.photos.iter().enumerate().any(|(i, p)| {
                p.file != format!("{:02}.png", i + 1)
                    || p.prompt.trim().is_empty()
                    || p.prompt.len() > 32768
            })
        {
            return Err(invalid());
        }
        Ok(v)
    }
    pub async fn create_illustrated(
        &self,
        date: String,
        input: IllustratedInput,
    ) -> Result<TurnAccepted, AuthenticError> {
        if !date_shape(&date)
            || !super::valid_id(&input.request_id)
            || !super::valid_id(&input.profile)
            || diary_application::agent::Locale::parse(&input.locale).is_err()
            || input.engine.as_ref().is_some_and(|e| !super::valid_id(e))
        {
            return Err(invalid());
        }
        let v: TurnAccepted = self
            .authentic(
                Method::POST,
                &format!("/illustrated/{date}"),
                Some(json!(input)),
            )
            .await?;
        Ok(v)
    }
    pub async fn delete_illustrated(
        &self,
        date: String,
        version: String,
        confirmed: bool,
    ) -> Result<(), AuthenticError> {
        if !date_shape(&date) || !diary_application::diagnostics::valid_uuid(&version) || !confirmed
        {
            return Err(invalid());
        }
        // The endpoint acknowledges deletion with JSON for the bounded shared transport.
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Ack {
            deleted: bool,
        }
        let ack: Ack = self
            .authentic(
                Method::DELETE,
                &format!("/illustrated/{date}"),
                Some(json!({"version":version,"confirmed":confirmed})),
            )
            .await?;
        if !ack.deleted {
            return Err(invalid());
        }
        Ok(())
    }
    pub async fn read_illustrated_photo(
        &self,
        date: String,
        version: String,
        n: u8,
    ) -> Result<Vec<u8>, AuthenticError> {
        if !date_shape(&date)
            || !diary_application::diagnostics::valid_uuid(&version)
            || n == 0
            || n > 99
        {
            return Err(invalid());
        }
        let c = self.connection.lock().await;
        let d = c
            .discovery
            .as_ref()
            .ok_or_else(|| AuthenticError::new("disconnected"))?;
        let url = format!(
            "{}/api/v1/illustrated/{date}/photos/{n:02}.png?version={version}",
            d.origin
        );
        let token = d.token.clone();
        let session = c.session.clone().ok_or_else(invalid)?;
        drop(c);
        let mut r = self
            .client
            .get(url)
            .bearer_auth(token)
            .header("x-diary-session", session)
            .header("x-diary-trace-id", super::new_trace())
            .send()
            .await
            .map_err(|_| AuthenticError::new("unavailable"))?;
        if !r.status().is_success()
            || r.headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                != Some("image/png")
        {
            return Err(AuthenticError::new("invalid_response"));
        }
        let mut bytes = vec![];
        while let Some(chunk) = r.chunk().await.map_err(|_| invalid())? {
            if bytes.len() + chunk.len() > 32 * 1024 * 1024 {
                return Err(invalid());
            }
            bytes.extend_from_slice(&chunk);
        }
        if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            return Err(invalid());
        }
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::authentic::tests::{fixture, http};
    #[tokio::test]
    async fn list_and_read_reject_malformed_server_values() {
        let (_t, b, task) = fixture(vec![http("200 OK", json!({"dates":["2026-1-1"]}))]);
        b.connect().await.unwrap();
        assert!(b.list_illustrated().await.is_err());
        task.join().unwrap();
        let (_t, b, _task) = fixture(vec![]);
        assert!(b.read_illustrated("../date".into()).await.is_err());
        assert!(
            b.read_illustrated_photo("2026-11-13".into(), "../path".into(), 1)
                .await
                .is_err()
        );
    }
}

//! Typed import commands; every route and authority is constructed from bounded inputs.
use super::{AuthenticError, Bridge, valid_id};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportFailure {
    UnnamedTimezone,
    TimezoneDatabaseUnavailable,
    InvalidDiaryDate,
    StorageError,
    NotFound,
    SourceStale,
    InvalidTimeAuthority,
    FixFilenameDirectoryExternally,
    Cancelled,
    Limit,
    #[serde(other)]
    Unknown,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct ImportJob {
    pub id: String,
    pub sequence: u64,
    pub state: String,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct ImportAck {
    pub import_id: String,
    pub job: ImportJob,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeAuthority {
    Header,
    Filename,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct ResolutionAck {
    pub resolved: bool,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct SourceInstant {
    pub unix_ms: i64,
    pub precision: Precision,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Precision {
    Second,
    Milli,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum SourceDating {
    Agreed {
        header: SourceInstant,
        filename: SourceInstant,
    },
    TimestampConflict {
        header: SourceInstant,
        filename: SourceInstant,
    },
    HeaderOnly {
        header: SourceInstant,
    },
    FilenameOnly {
        filename: SourceInstant,
    },
    Undated,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Source {
    pub id: String,
    pub relative_key: String,
    pub digest: String,
    pub import_id: String,
    pub dating: SourceDating,
    pub duplicate_producer_id: bool,
    pub header_invalid: Option<String>,
    pub filename_invalid: Option<String>,
    pub navigation: Option<SourceDisplay>,
    pub eligible: bool,
    pub capabilities: Capabilities,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceFailure {
    Missing,
    Denied,
    NotDownloaded,
    NotRegular,
    TooLarge,
    SourceChanging,
    InvalidPath,
    Unavailable,
    Malformed,
    Unsupported,
    Conflict,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SourceItem {
    Included {
        source: Source,
        instant: SourceInstant,
        misplaced: bool,
    },
    Warning {
        source: Source,
        misplaced: bool,
    },
    Failure {
        relative_key: String,
        code: SourceFailure,
        group_id: Option<String>,
        source: Option<Source>,
    },
    PathCollision {
        relative_keys: Vec<String>,
        group_id: Option<String>,
    },
}
#[derive(Debug, Serialize, Deserialize)]
pub struct ImportPage {
    pub groups: Option<Vec<SourceGroup>>,
    pub reimport_required: bool,
    pub import_id: String,
    pub date: String,
    pub zone: String,
    pub timezone_database: String,
    pub config_generation: u64,
    pub completed_at: i64,
    pub items: Vec<SourceItem>,
    pub next_cursor: Option<usize>,
    pub incomplete: bool,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct SourceGroup {
    pub id: String,
    pub alias: String,
    #[serde(rename = "type")]
    pub kind: String,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct LocalTime {
    pub date: String,
    pub datetime: String,
    pub offset_seconds: i32,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct SourceDisplay {
    pub group_id: String,
    pub header: Option<LocalTime>,
    pub filename: Option<LocalTime>,
    pub included: Option<LocalTime>,
    pub authority: Option<TimeAuthority>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Capabilities {
    pub read: bool,
    pub save: bool,
    pub trash: bool,
    pub reason: String,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct SourceBody {
    pub source: Source,
    pub snapshot_id: String,
    pub body: String,
    pub capabilities: Capabilities,
}
fn invalid() -> AuthenticError {
    AuthenticError::new("invalid_input")
}
fn date_shape(s: &str) -> bool {
    s.len() == 10
        && s.bytes().enumerate().all(|(i, b)| {
            if i == 4 || i == 7 {
                b == b'-'
            } else {
                b.is_ascii_digit()
            }
        })
}
fn digest_shape(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}
impl Bridge {
    pub async fn read_source(
        &self,
        source_id: String,
        snapshot_id: String,
    ) -> Result<SourceBody, AuthenticError> {
        if !valid_id(&source_id) || !valid_id(&snapshot_id) {
            return Err(invalid());
        }
        self.authentic(
            Method::GET,
            &format!("/sources/{source_id}?snapshot_id={snapshot_id}"),
            None,
        )
        .await
    }

    pub async fn import_date(
        &self,
        date: String,
        request_id: String,
    ) -> Result<ImportAck, AuthenticError> {
        if !date_shape(&date) || !valid_id(&request_id) {
            return Err(invalid());
        }
        self.authentic(
            Method::POST,
            "/imports",
            Some(json!({"date":date,"request_id":request_id})),
        )
        .await
    }
    pub async fn list_sources(
        &self,
        import_id: String,
        cursor: Option<usize>,
    ) -> Result<ImportPage, AuthenticError> {
        if !valid_id(&import_id) || cursor.is_some_and(|c| c > 30_000) {
            return Err(invalid());
        }
        let mut path = format!("/sources?import_id={import_id}");
        if let Some(c) = cursor {
            path.push_str(&format!("&cursor={c}"));
        }
        self.authentic(Method::GET, &path, None).await
    }
    pub async fn resolve_source_time(
        &self,
        import_id: String,
        source_id: String,
        digest: String,
        authority: TimeAuthority,
    ) -> Result<ResolutionAck, AuthenticError> {
        if !valid_id(&import_id) || !valid_id(&source_id) || !digest_shape(&digest) {
            return Err(invalid());
        }
        self.authentic(
            Method::POST,
            &format!("/sources/{source_id}/time-resolution"),
            Some(json!({"import_id":import_id,"digest":digest,"authority":authority})),
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn paths_and_authorities_are_bounded_before_transport() {
        let bridge = Bridge::new(std::path::PathBuf::from("/nonexistent/discovery.json")).unwrap();
        assert_eq!(
            bridge
                .import_date("/private/file".into(), "request".into())
                .await
                .unwrap_err()
                .code,
            "invalid_input"
        );
        assert_eq!(
            bridge
                .list_sources("/private/file".into(), None)
                .await
                .unwrap_err()
                .code,
            "invalid_input"
        );
        assert_eq!(
            bridge
                .resolve_source_time(
                    "id".into(),
                    "../file".into(),
                    "a".repeat(64),
                    TimeAuthority::Header
                )
                .await
                .unwrap_err()
                .code,
            "invalid_input"
        );
    }
}

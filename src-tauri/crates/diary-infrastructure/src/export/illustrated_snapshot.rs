//! Version-bound capture of published text and prepared JPEGs before remote mutation.
use super::*;
use diary_domain::illustrated::{UploadPart, upload_parts};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub(crate) enum Node {
    Text {
        block: Value,
    },
    Photo {
        index: usize,
        sha256: String,
        caption: String,
    },
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Manifest {
    pub version: u32,
    pub warnings: u32,
    pub nodes: Vec<Node>,
}
pub(crate) fn digest(nodes: &[Node]) -> String {
    let semantic: Vec<Value> = nodes
        .iter()
        .map(|n| match n {
            Node::Text { block } => block.clone(),
            Node::Photo {
                sha256, caption, ..
            } => json!({"image":sha256,"caption":caption}),
        })
        .collect();
    hex_sha(&serde_json::to_vec(&semantic).expect("JSON nodes"))
}
impl ExportCoordinatorState {
    pub(super) fn illustrated_snapshot(
        &self,
        date: DiaryDate,
        version: &str,
    ) -> Result<Snapshot, ExportError> {
        use diary_application::illustrated::Error;
        let map = |e| match e {
            Error::Stale | Error::NotFound => ExportError::StaleRevision,
            _ => ExportError::Storage,
        };
        let store = self.illustrated.as_ref().ok_or(ExportError::Disabled)?;
        let entry = store
            .read(date)
            .map_err(map)?
            .ok_or(ExportError::NothingToExport)?;
        if entry.id != version {
            return Err(ExportError::StaleRevision);
        }
        let parts = upload_parts(&entry).map_err(|_| ExportError::Invalid)?;
        let mut photos = Vec::new();
        let mut total = 0;
        let mut source_hashes = Vec::new();
        for n in 1..=entry.photos.len() {
            let png = store.photo(date, version, n).map_err(map)?;
            source_hashes.push(hex_sha(&png));
            let jpeg = super::jpeg::convert(&png)?;
            total += jpeg.len();
            if total > 64 * 1024 * 1024 {
                return Err(ExportError::Limit);
            }
            photos.push(jpeg);
        }
        if store
            .read(date)
            .map_err(map)?
            .is_none_or(|e| e.id != version)
        {
            return Err(ExportError::StaleRevision);
        }
        let mut nodes = Vec::new();
        let mut warnings = 0;
        for part in parts {
            match part {
                UploadPart::Text(text) => {
                    let projected = diary_domain::notion::project(&text);
                    warnings += projected.warnings;
                    nodes.extend(projected.blocks.iter().map(|b| Node::Text {
                        block: notion::wire::block(b),
                    }));
                }
                UploadPart::Photo { index, alt } => nodes.push(Node::Photo {
                    index,
                    sha256: hex_sha(&photos[index]),
                    caption: alt,
                }),
            }
        }
        let bytes = serde_json::to_vec(&Manifest {
            version: 1,
            warnings,
            nodes,
        })
        .map_err(|_| ExportError::Storage)?;
        if bytes.len() > MAX_SNAPSHOT_BYTES {
            return Err(ExportError::Limit);
        }
        Ok(Snapshot {
            content_kind: ContentKind::Illustrated,
            illustrated_version: Some(version.into()),
            authentic_id: date.to_string(),
            date,
            // Legacy persistence columns represent a semantic snapshot hash for this kind.
            revision: diary_domain::authentic::AuthenticRevision { sequence: 0, sha256: hex_sha(&serde_json::to_vec(&json!({"converter":1,"height":480,"quality":85,"version":version,"body":entry.body,"pngs":source_hashes})).expect("manifest")) },
            bytes,
            photos,
        })
    }
}

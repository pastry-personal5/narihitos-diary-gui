//! Atomic Trash admission: ledger intent, first step, reservations, immutable plan,
//! member rows and authorization consumption share one SQLite transaction.
use super::{MAX_AUDIO_BYTES, PairPlan, native::Receipt};
use crate::{
    ledger::{admit_tx, hash_input},
    source::{
        read::FileDigest,
        root::{ResolvedRoot, source_id},
    },
    sqlite::{Db, DbError},
};
use diary_application::operations::{Admission, LedgerError, NewOperation, ResourceKey};
use diary_domain::source::path::normalize;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{fmt, path::PathBuf};

#[derive(Debug, PartialEq, Eq)]
pub enum StoreError {
    Invalid,
    TokenUsed,
    Corrupt,
    Ledger(LedgerError),
    Storage,
}
impl From<DbError> for StoreError {
    fn from(_: DbError) -> Self {
        Self::Storage
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoredPlan {
    pub version: u32,
    pub source_id: String,
    pub root: PathBuf,
    pub insensitive: bool,
    pub dev: u64,
    pub ino: u64,
    pub producer_id: String,
    pub markdown_rel: String,
    pub markdown: FileDigest,
    pub audio_rel: String,
    pub audio: Option<FileDigest>,
}
impl fmt::Debug for StoredPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StoredPlan")
            .field("version", &self.version)
            .finish_non_exhaustive()
    }
}
impl StoredPlan {
    pub fn from_pair(source_id: &str, p: &PairPlan) -> Self {
        Self {
            version: 1,
            source_id: source_id.into(),
            root: p.root.canonical.clone(),
            insensitive: p.root.insensitive,
            dev: p.root.dev,
            ino: p.root.ino,
            producer_id: p.producer_id.clone(),
            markdown_rel: p.markdown_rel.clone(),
            markdown: p.markdown.clone(),
            audio_rel: p.audio_rel.clone(),
            audio: p.audio.clone(),
        }
    }
    pub fn resolved_root(&self) -> ResolvedRoot {
        ResolvedRoot {
            canonical: self.root.clone(),
            insensitive: self.insensitive,
            dev: self.dev,
            ino: self.ino,
        }
    }
    fn encoded(&self) -> Result<String, StoreError> {
        let text = serde_json::to_string(self).map_err(|_| StoreError::Invalid)?;
        if text.len() > 16384 || !self.valid() {
            return Err(StoreError::Invalid);
        }
        Ok(text)
    }
    fn valid(&self) -> bool {
        let root = self.resolved_root();
        let Ok(md) = normalize(&self.markdown_rel, self.insensitive) else {
            return false;
        };
        self.version == 1
            && self.root.is_absolute()
            && self.root.to_str().is_some_and(|s| s.len() <= 4096)
            && self.markdown_rel.len() <= 1024
            && self.audio_rel.len() <= 1024
            && source_id(&root, &md) == self.source_id
            && self.markdown_rel.strip_suffix(".md").is_some_and(|stem| {
                self.audio_rel == format!("{stem}.assets/{}.m4a", self.producer_id)
            })
            && diary_domain::source::stamp::valid_uuid(&self.producer_id)
            && self.markdown.dev == self.dev
            && self.markdown.size <= diary_domain::source::front_matter::MAX_FILE as u64
            && self
                .audio
                .as_ref()
                .is_none_or(|a| a.dev == self.dev && a.size <= MAX_AUDIO_BYTES)
            && normalize(&self.audio_rel, self.insensitive).is_ok()
    }
    pub fn hash(&self) -> Result<String, StoreError> {
        Ok(hex(&Sha256::digest(self.encoded()?.as_bytes())))
    }
    pub fn resources(&self) -> Result<Vec<ResourceKey>, StoreError> {
        if !self.valid() {
            return Err(StoreError::Invalid);
        }
        let audio =
            normalize(&self.audio_rel, self.insensitive).map_err(|_| StoreError::Invalid)?;
        let audio_key = hash_input(
            "source_audio",
            1,
            &[
                ("root", self.root.to_str().ok_or(StoreError::Invalid)?),
                ("key", &audio.key),
            ],
        );
        let mut resources = vec![
            ResourceKey::parse(&format!("source:{}", self.source_id)).ok_or(StoreError::Invalid)?,
            ResourceKey::parse(&format!("audio:{audio_key}")).ok_or(StoreError::Invalid)?,
        ];
        resources.sort();
        Ok(resources)
    }
    pub fn initial_hash(&self) -> Result<String, StoreError> {
        Ok(hash_input(
            "source_trash",
            1,
            &[
                ("action", "initial"),
                ("source_id", &self.source_id),
                ("plan_hash", &self.hash()?),
                ("checkpoint", "0"),
                ("ordinals", "0,1"),
            ],
        ))
    }
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn fingerprint_ok(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemberStatus {
    Pending,
    Intended,
    Moved,
    Missing,
    Failed,
    Uncertain,
}
impl MemberStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Intended => "intended",
            Self::Moved => "moved",
            Self::Missing => "missing",
            Self::Failed => "failed",
            Self::Uncertain => "uncertain",
        }
    }
    fn parse(s: &str) -> Option<Self> {
        [
            Self::Pending,
            Self::Intended,
            Self::Moved,
            Self::Missing,
            Self::Failed,
            Self::Uncertain,
        ]
        .into_iter()
        .find(|v| v.as_str() == s)
    }
}
#[derive(Debug, Clone)]
pub struct MemberRecord {
    pub ordinal: u8,
    pub status: MemberStatus,
    pub receipt: Option<Receipt>,
    pub reason: Option<String>,
    pub checkpoint: u64,
}

impl Db {
    /// Hold the preview mutex through this call; remove the transient token only after
    /// commit succeeds. A process death in between cannot replay the fingerprint.
    pub fn admit_trash(
        &self,
        op: &NewOperation,
        plan: &StoredPlan,
        fingerprint: &str,
    ) -> Result<Admission, StoreError> {
        let encoded = plan.encoded()?;
        let plan_hash = plan.hash()?;
        let initial_hash = plan.initial_hash()?;
        let Some(step) = &op.first_step else {
            return Err(StoreError::Invalid);
        };
        let mut keys = op.resources.clone();
        keys.sort();
        keys.dedup();
        if op.kind != "source_trash"
            || op.hash_version != 1
            || op.input_hash != initial_hash
            || step.input_hash != initial_hash
            || step.expected_checkpoint != 0
            || step.state.is_some()
            || keys != plan.resources()?
            || !fingerprint_ok(fingerprint)
        {
            return Err(StoreError::Invalid);
        }
        self.transact(|tx| {
            // Existing IDs must win before token reuse/expiry validation. The application
            // handles token expiry after its own durable duplicate lookup.
            if let Some(existing) = crate::ledger::fetch(tx, &op.id)? {
                let prior: Option<(String, i64)> = tx.query_row(
                    "SELECT input_hash,expected_checkpoint FROM operation_steps WHERE operation_id=?1 AND step_id=?2",
                    [op.id.as_str(), step.step_id.as_str()], |r| Ok((r.get(0)?,r.get(1)?))
                ).optional().map_err(|_| DbError::Storage)?;
                return Ok(Ok(
                    if existing.kind == op.kind
                        && existing.input_hash == op.input_hash
                        && existing.hash_version == op.hash_version
                        && prior == Some((step.input_hash.clone(),0))
                    {
                        Admission::Existing(existing)
                    } else {
                        Admission::Mismatch
                    },
                ));
            }
            let used = tx
                .query_row(
                    "SELECT 1 FROM trash_authorizations WHERE fingerprint = ?1",
                    [fingerprint],
                    |_| Ok(()),
                )
                .optional()
                .map_err(|_| DbError::Storage)?
                .is_some();
            if used {
                return Ok(Err(StoreError::TokenUsed));
            }
            let admitted = match admit_tx(tx, op)? {
                Ok(a) => a,
                Err(e) => return Ok(Err(StoreError::Ledger(e))),
            };
            tx.execute(
                "INSERT INTO trash_plans VALUES (?1, ?2, ?3, ?4)",
                params![op.id.as_str(), plan.source_id, plan_hash, encoded],
            )
            .map_err(|_| DbError::Storage)?;
            for ordinal in 0..=1 {
                tx.execute(
                    "INSERT INTO trash_members VALUES (?1, ?2, ?3, NULL, NULL, 1)",
                    params![
                        op.id.as_str(),
                        ordinal,
                        if ordinal == 1 && plan.audio.is_none() {
                            "missing"
                        } else {
                            "pending"
                        }
                    ],
                )
                .map_err(|_| DbError::Storage)?;
            }
            tx.execute(
                "INSERT INTO trash_authorizations VALUES (?1, ?2, ?3, ?4)",
                params![
                    fingerprint,
                    op.id.as_str(),
                    step.step_id.as_str(),
                    plan_hash
                ],
            )
            .map_err(|_| DbError::Storage)?;
            Ok(Ok(admitted))
        })?
    }

    /// Decode authoritative rows fail-closed. A corrupt plan/member cannot silently
    /// disappear from recovery or release its existing reservations.
    pub fn trash_plan(
        &self,
        id: &diary_application::operations::OperationId,
    ) -> Result<Option<(StoredPlan, Vec<MemberRecord>)>, StoreError> {
        self.with(|c| load_plan(c, id))?
    }
}

type LoadedPlan = Option<(StoredPlan, Vec<MemberRecord>)>;
type StoreTx<T> = Result<Result<T, StoreError>, DbError>;

fn load_plan(
    c: &rusqlite::Connection,
    id: &diary_application::operations::OperationId,
) -> StoreTx<LoadedPlan> {
    let raw: Option<(String, String, String)> = c
        .query_row(
            "SELECT source_id,plan_hash,plan FROM trash_plans WHERE operation_id=?1",
            [id.as_str()],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(|_| DbError::Storage)?;
    let Some((source, hash, text)) = raw else {
        return Ok(Ok(None));
    };
    if text.len() > 16384 {
        return Ok(Err(StoreError::Corrupt));
    }
    let plan: StoredPlan = match serde_json::from_str(&text) {
        Ok(p) => p,
        Err(_) => return Ok(Err(StoreError::Corrupt)),
    };
    if !plan.valid() || plan.source_id != source || plan.hash().as_ref() != Ok(&hash) {
        return Ok(Err(StoreError::Corrupt));
    }
    let mut stmt = c.prepare("SELECT ordinal,status,receipt,reason,checkpoint FROM trash_members WHERE operation_id=?1 ORDER BY ordinal").map_err(|_| DbError::Storage)?;
    let rows = stmt
        .query_map([id.as_str()], |r| {
            Ok((
                r.get::<_, u8>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, i64>(4)?,
            ))
        })
        .map_err(|_| DbError::Storage)?;
    let mut members = Vec::new();
    for row in rows {
        let (ordinal, status, receipt, reason, checkpoint) = row.map_err(|_| DbError::Damaged)?;
        let Some(status) = MemberStatus::parse(&status) else {
            return Ok(Err(StoreError::Corrupt));
        };
        let receipt: Option<Receipt> = match receipt {
            None => None,
            Some(s) if s.len() <= 8192 => match serde_json::from_str(&s) {
                Ok(r) => Some(r),
                Err(_) => return Ok(Err(StoreError::Corrupt)),
            },
            _ => return Ok(Err(StoreError::Corrupt)),
        };
        let original = if ordinal == 0 {
            Some(&plan.markdown)
        } else {
            plan.audio.as_ref()
        };
        if ordinal as usize != members.len()
            || ordinal > 1
            || checkpoint <= 0
            || (status == MemberStatus::Moved && receipt.is_none())
            || (status == MemberStatus::Moved && reason.is_some())
            || (receipt.is_some()
                && !matches!(status, MemberStatus::Moved | MemberStatus::Uncertain))
            || (status == MemberStatus::Missing && (ordinal != 1 || plan.audio.is_some()))
            || (ordinal == 1 && plan.audio.is_none() && status != MemberStatus::Missing)
            || receipt.as_ref().is_some_and(|r| {
                !r.path.is_absolute()
                    || r.path.to_str().is_none_or(|s| s.len() > 4096)
                    || original.is_none_or(|o| r.dev != o.dev || r.ino != o.ino)
                    || r.path
                        == plan.root.join(if ordinal == 0 {
                            &plan.markdown_rel
                        } else {
                            &plan.audio_rel
                        })
            })
            || reason.as_deref().is_some_and(|s| {
                ![
                    "unavailable",
                    "changed",
                    "unsafe_path",
                    "ownership_ambiguous",
                    "root_changed",
                    "uncertain",
                ]
                .contains(&s)
            })
        {
            return Ok(Err(StoreError::Corrupt));
        }
        members.push(MemberRecord {
            ordinal,
            status,
            receipt,
            reason,
            checkpoint: checkpoint as u64,
        });
    }
    if members.len() != 2 {
        return Ok(Err(StoreError::Corrupt));
    }
    Ok(Ok(Some((plan, members))))
}

pub mod progress;

#[cfg(test)]
mod tests;

//! Body-less source observations; no filesystem paths or producer metadata in tokens.
use diary_domain::source::{
    front_matter::{Newline, ParseError, Value},
    stamp::Dating,
};
use std::fmt;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unavailable {
    Missing,
    Denied,
    NotDownloaded,
    NotRegular,
    TooLarge,
    SourceChanging,
    Io,
    InvalidPath,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnsupportedReason {
    Layout,
    Schema,
    Kind,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictKind {
    IdMismatch,
    IdentityCollision,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Warning {
    DuplicateProducerId,
}
#[derive(Clone, PartialEq, Eq)]
pub struct ObservationToken {
    // Root namespace is a complete digest, not an absolute path. Tuple equality is
    // checked independently of the shortened public ID before observation reuse.
    pub root_namespace: [u8; 32],
    pub key: String,
    pub sha256: [u8; 32],
    pub dev: u64,
    pub ino: u64,
    pub size: u64,
    pub mtime_ns: i128,
    pub ctime_ns: i128,
    pub config_generation: u64,
    /// Absent for raw diagnostic scans; imports bind every observation to provenance.
    pub import_id: Option<String>,
}
impl fmt::Debug for ObservationToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ObservationToken")
            .field("key", &self.key)
            .field("digest_prefix", &&self.sha256[..4])
            .field("size", &self.size)
            .field("generation", &self.config_generation)
            .finish_non_exhaustive()
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceListing {
    pub id: String,
    pub rel: String,
    pub size: u64,
    pub dating: Dating,
    pub token: ObservationToken,
    pub warnings: Vec<Warning>,
}
#[derive(Clone, PartialEq, Eq)]
pub struct SourceView {
    pub listing: SourceListing,
    pub body: String,
    pub raw_front_matter: String,
    pub fields: Vec<(String, Value)>,
    pub newline: Newline,
    pub bom: bool,
}
impl fmt::Debug for SourceView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SourceView")
            .field("listing", &self.listing)
            .field("body_bytes", &self.body.len())
            .field("front_bytes", &self.raw_front_matter.len())
            .field("keys", &self.fields.len())
            .finish()
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceOutcome {
    Ok(Box<SourceListing>),
    Unavailable(Unavailable),
    Malformed(ParseError),
    Unsupported {
        reason: UnsupportedReason,
        viewable: bool,
    },
    Conflict(ConflictKind),
}
#[derive(Debug, Default)]
pub struct ScanReport {
    pub listings: Vec<SourceListing>,
    /// Safely observed unsupported/identity-conflicting rows; never eligible.
    pub readable: Vec<SourceListing>,
    pub failures: Vec<(String, SourceOutcome)>,
    pub collisions: Vec<Vec<String>>,
    pub incomplete: bool,
    pub entries_seen: usize,
    pub candidates_seen: usize,
}
#[derive(Debug, PartialEq, Eq)]
pub enum ViewError {
    Outcome(SourceOutcome),
    Stale,
}

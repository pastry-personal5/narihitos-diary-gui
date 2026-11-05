//! Date import provenance and explicit, complete-byte-bound timestamp authority.
use crate::source::{ScanReport, SourceListing};
use diary_domain::{
    authentic::DiaryDate,
    source::{
        stamp::{Dating, Instant},
        window::ImportZone,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Authority {
    Header,
    Filename,
}

/// A single valid authority still requires a user choice. Directory mismatches
/// are represented as invalid filename evidence and can never be chosen.
pub fn choose(dating: &Dating, authority: Authority) -> Option<Instant> {
    match (dating, authority) {
        (Dating::Agreed(t), _) => Some(*t),
        (
            Dating::Disagree { header, .. } | Dating::HeaderOnly { header, .. },
            Authority::Header,
        ) => Some(*header),
        (
            Dating::Disagree { filename, .. } | Dating::FilenameOnly { filename, .. },
            Authority::Filename,
        ) => Some(*filename),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportContext {
    pub id: String,
    pub date: DiaryDate,
    pub zone: ImportZone,
    pub config_generation: u64,
    pub completed_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolution {
    pub authority: Authority,
    pub instant: Instant,
    pub decided_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncludedSource {
    pub source: SourceListing,
    pub instant: Instant,
}

#[derive(Debug)]
pub struct ImportListing {
    pub context: ImportContext,
    pub included: Vec<IncludedSource>,
    pub warnings: Vec<SourceListing>,
    /// Includes misplaced sources even when an explicit header choice includes them.
    pub misplaced: Vec<String>,
    pub scan: ScanReport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportError {
    UnnamedZone,
    TimezoneDatabase,
    InvalidDiaryDate,
    Storage,
    NotFound,
    Stale,
    InvalidAuthority,
    FilenameDirectoryMismatch,
    Cancelled,
    Limit,
}

pub trait ImportRepository: Send + Sync {
    fn bind_source(&self, id: &str, namespace: &[u8; 32], key: &str) -> Result<(), ImportError>;
    fn begin(&self, context: &ImportContext) -> Result<(), ImportError>;
    fn complete(&self, id: &str, at: i64) -> Result<(), ImportError>;
    /// Incomplete contexts are never returned.
    fn context(&self, id: &str) -> Result<ImportContext, ImportError>;
    fn resolution(
        &self,
        source_id: &str,
        digest: &[u8; 32],
    ) -> Result<Option<Resolution>, ImportError>;
    fn save_resolution(
        &self,
        source_id: &str,
        digest: &[u8; 32],
        choice: &Resolution,
    ) -> Result<(), ImportError>;
}

pub trait ImportStore: Send + Sync {
    /// The caller supplies a server-issued ID and a lease/cancellation predicate.
    fn import_date(
        &self,
        id: &str,
        date: DiaryDate,
        generation: u64,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<ImportListing, ImportError>;
    fn list(
        &self,
        import_id: &str,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<ImportListing, ImportError>;
    fn resolve(
        &self,
        import_id: &str,
        source_id: &str,
        digest: &[u8; 32],
        authority: Authority,
        generation: u64,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), ImportError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use diary_domain::source::stamp::{Precision, StampError};
    #[test]
    fn resolution_authority_matrix_never_infers_missing_evidence() {
        let h = Instant {
            unix_ms: 1,
            precision: Precision::Milli,
        };
        let f = Instant {
            unix_ms: 2,
            precision: Precision::Milli,
        };
        for (dating, header, filename) in [
            (Dating::Agreed(h), Some(h), Some(h)),
            (
                Dating::Disagree {
                    header: h,
                    filename: f,
                },
                Some(h),
                Some(f),
            ),
            (
                Dating::HeaderOnly {
                    header: h,
                    filename: StampError::Format,
                },
                Some(h),
                None,
            ),
            (
                Dating::HeaderOnly {
                    header: h,
                    filename: StampError::DirMismatch,
                },
                Some(h),
                None,
            ),
            (
                Dating::FilenameOnly {
                    filename: f,
                    header: StampError::Offset,
                },
                None,
                Some(f),
            ),
            (
                Dating::Undated {
                    header: StampError::Format,
                    filename: StampError::Calendar,
                },
                None,
                None,
            ),
        ] {
            assert_eq!(choose(&dating, Authority::Header), header);
            assert_eq!(choose(&dating, Authority::Filename), filename);
        }
    }
}

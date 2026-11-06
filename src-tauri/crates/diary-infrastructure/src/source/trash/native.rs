//! NSFileManager Trash only. Callers must durably commit member intent before calling
//! `move_member` and commit its result before attempting the next member.
use super::{MAX_AUDIO_BYTES, PreparationError};
use crate::{
    coordination,
    source::{
        read::{FileDigest, digest_member, open_root_lookup},
        root::ResolvedRoot,
    },
};
use diary_domain::source::path::{SourcePath, normalize};
use objc2_foundation::{NSFileManager, NSString, NSURL};
use std::{fmt, os::unix::fs::MetadataExt, path::PathBuf};

/// Opaque private receipt, never serialized into a WebView response or diagnostic log.
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub(crate) path: PathBuf,
    pub(crate) dev: u64,
    pub(crate) ino: u64,
}
impl fmt::Debug for Receipt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Receipt { .. }")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MoveResult {
    Moved(Receipt),
    /// A returned failure with a freshly verified unchanged original.
    Failed(PreparationError),
    /// A call may have taken effect but its provenance cannot be proven.
    Uncertain {
        receipt: Option<Receipt>,
    },
}

/// The last ownership check runs inside native deletion coordination. This cooperates
/// with native writers, but is not an atomic CAS against arbitrary filesystem writers.
pub fn move_member(
    root: &ResolvedRoot,
    rel: &SourcePath,
    expected: &FileDigest,
    validate_ownership: impl FnOnce() -> Result<(), PreparationError>,
) -> MoveResult {
    let path = root.canonical.join(&rel.spelling);
    coordination::delete(&path, || {
        if let Err(e) = validate_ownership() {
            return MoveResult::Failed(e);
        }
        match digest_member(root, rel, MAX_AUDIO_BYTES, &|| false) {
            Ok(current) if &current == expected => {}
            Ok(_) => return MoveResult::Failed(PreparationError::Changed),
            Err(e) => return MoveResult::Failed(e.into()),
        }
        let Some(text) = path.to_str() else {
            return MoveResult::Failed(PreparationError::UnsafePath);
        };
        let url = NSURL::fileURLWithPath(&NSString::from_str(text));
        let mut destination = None;
        let result = NSFileManager::defaultManager()
            .trashItemAtURL_resultingItemURL_error(&url, Some(&mut destination));
        if result.is_err() {
            // Foundation exceptions/errors and paths never leave this adapter.
            return if digest_member(root, rel, MAX_AUDIO_BYTES, &|| false).as_ref() == Ok(expected)
            {
                MoveResult::Failed(PreparationError::Unavailable)
            } else {
                MoveResult::Uncertain { receipt: None }
            };
        }
        let Some(destination) = destination.filter(|u| u.isFileURL()) else {
            return MoveResult::Uncertain { receipt: None };
        };
        let Some(destination) = destination.path() else {
            return MoveResult::Uncertain { receipt: None };
        };
        let destination = destination.to_string();
        if destination.len() > 4096 || destination.contains('\0') {
            return MoveResult::Uncertain { receipt: None };
        }
        let receipt = Receipt {
            path: PathBuf::from(destination),
            dev: expected.dev,
            ino: expected.ino,
        };
        if !receipt.path.is_absolute()
            || receipt.path == path
            || !verify_receipt(&receipt, expected)
        {
            return MoveResult::Uncertain {
                receipt: Some(receipt),
            };
        }
        MoveResult::Moved(receipt)
    })
    .unwrap_or(MoveResult::Failed(PreparationError::Unavailable))
}

/// Reconcile only an actual persisted native receipt. Never search Trash for names or
/// hashes, and never infer success from source-path absence.
pub fn verify_receipt(receipt: &Receipt, original: &FileDigest) -> bool {
    let Some(parent) = receipt.path.parent() else {
        return false;
    };
    let Some(name) = receipt.path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    let Ok(parent_file) = open_root_lookup(parent) else {
        return false;
    };
    let Ok(metadata) = parent_file.metadata() else {
        return false;
    };
    let root = ResolvedRoot {
        canonical: parent.into(),
        insensitive: false,
        dev: metadata.dev(),
        ino: metadata.ino(),
    };
    let Ok(rel) = normalize(name, false) else {
        return false;
    };
    let observed = digest_member(&root, &rel, MAX_AUDIO_BYTES, &|| false);
    observed.is_ok_and(|observed| {
        observed.dev == receipt.dev
            && observed.ino == receipt.ino
            && observed.dev == original.dev
            && observed.ino == original.ino
            && observed.size == original.size
            && observed.sha256 == original.sha256
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::{read::digest_member, tests::Tree};
    use std::fs;

    /// Separate native evidence run; only freshly created fixtures can reach Trash.
    #[test]
    #[ignore = "native Trash evidence: run explicitly outside the ordinary gate"]
    fn native_receipt_proves_move_and_preserves_neighbors() {
        let tree = Tree::new();
        let name = format!("diary-trash-test-{}.m4a", std::process::id());
        tree.write(&name, b"native Trash receipt fixture");
        tree.write("neighbor", b"must remain");
        let rel = normalize(&name, tree.root.insensitive).unwrap();
        let expected = digest_member(&tree.root, &rel, MAX_AUDIO_BYTES, &|| false).unwrap();
        assert_eq!(
            move_member(&tree.root, &rel, &expected, || Err(
                PreparationError::OwnershipAmbiguous
            )),
            MoveResult::Failed(PreparationError::OwnershipAmbiguous)
        );
        assert!(tree.temp.path().join(&name).exists());
        let result = move_member(&tree.root, &rel, &expected, || Ok(()));
        let MoveResult::Moved(receipt) = result else {
            panic!("native receipt required; got {result:?}");
        };
        assert!(verify_receipt(&receipt, &expected));
        assert!(!tree.temp.path().join(&name).exists());
        tree.write(&name, b"replacement must remain");
        assert_eq!(
            move_member(&tree.root, &rel, &expected, || Ok(())),
            MoveResult::Failed(PreparationError::Changed)
        );
        assert_eq!(
            fs::read(tree.temp.path().join("neighbor")).unwrap(),
            b"must remain"
        );
        assert_eq!(
            fs::read(tree.temp.path().join(&name)).unwrap(),
            b"replacement must remain"
        );
        // Cleanup only this test's proven native receipt. Never enumerate or empty Trash.
        assert!(verify_receipt(&receipt, &expected));
        fs::remove_file(&receipt.path).unwrap();
    }
}

use std::collections::BTreeMap;
use unicode_normalization::UnicodeNormalization;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcePath {
    pub key: String,
    pub spelling: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathError {
    Empty,
    Absolute,
    Dot,
    DotDot,
    NonUtf8,
    ControlChar,
}

pub fn normalize(rel: &str, insensitive: bool) -> Result<SourcePath, PathError> {
    if rel.is_empty() {
        return Err(PathError::Empty);
    }
    if rel.starts_with('/') {
        return Err(PathError::Absolute);
    }
    let mut keys = Vec::new();
    for part in rel.split('/') {
        match part {
            "" => return Err(PathError::Empty),
            "." => return Err(PathError::Dot),
            ".." => return Err(PathError::DotDot),
            _ => {}
        }
        if part.chars().any(char::is_control) {
            return Err(PathError::ControlChar);
        }
        let nfc: String = part.nfc().collect();
        // Deliberately conservative: Unicode lowercase is not APFS's private table.
        // Non-ASCII cases can over-report collisions; ASCII/Latin case pairs are caught.
        keys.push(if insensitive {
            nfc.to_lowercase().nfc().collect()
        } else {
            nfc
        });
    }
    Ok(SourcePath {
        key: keys.join("/"),
        spelling: rel.into(),
    })
}

pub fn group_siblings(names: &[&str], insensitive: bool) -> Vec<Vec<usize>> {
    let mut groups = BTreeMap::<String, Vec<usize>>::new();
    for (index, name) in names.iter().enumerate() {
        if let Ok(path) = normalize(name, insensitive) {
            groups.entry(path.key).or_default().push(index);
        }
    }
    groups.into_values().filter(|g| g.len() > 1).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identity_and_validation() {
        assert_eq!(
            normalize("é/A.md", true).unwrap().key,
            normalize("e\u{301}/a.md", true).unwrap().key
        );
        assert_ne!(
            normalize("A.md", false).unwrap().key,
            normalize("a.md", false).unwrap().key
        );
        for path in ["", "/x", "x//y", "./x", "../x", "x/\0", "x/\n"] {
            assert!(normalize(path, false).is_err());
        }
        assert_eq!(
            group_siblings(&["é", "e\u{301}", "É", "other"], true),
            vec![vec![0, 1, 2]]
        );
    }
}

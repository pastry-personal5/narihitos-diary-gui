//! Versioned storage DTOs keep serialization out of the inward application/domain layers.
use diary_application::{imports::*, source::*};
use diary_domain::source::{front_matter::ParseError, stamp::*};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, io::Write};

const MAX_BYTES: usize = 16 * 1024 * 1024;

// Serde remote derives reuse domain variants without adding an inward dependency.
#[derive(Serialize, Deserialize)]
#[serde(remote = "Precision")]
enum PrecisionDto {
    Second,
    Milli,
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "Instant")]
struct InstantDto {
    unix_ms: i64,
    #[serde(with = "PrecisionDto")]
    precision: Precision,
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "StampError")]
enum StampErrorDto {
    Format,
    Offset,
    Calendar,
    DirMismatch,
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "Dating")]
enum DatingDto {
    Agreed(#[serde(with = "InstantDto")] Instant),
    Disagree {
        #[serde(with = "InstantDto")]
        header: Instant,
        #[serde(with = "InstantDto")]
        filename: Instant,
    },
    HeaderOnly {
        #[serde(with = "InstantDto")]
        header: Instant,
        #[serde(with = "StampErrorDto")]
        filename: StampError,
    },
    FilenameOnly {
        #[serde(with = "InstantDto")]
        filename: Instant,
        #[serde(with = "StampErrorDto")]
        header: StampError,
    },
    Undated {
        #[serde(with = "StampErrorDto")]
        header: StampError,
        #[serde(with = "StampErrorDto")]
        filename: StampError,
    },
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "ObservationToken")]
struct TokenDto {
    root_namespace: [u8; 32],
    key: String,
    sha256: [u8; 32],
    dev: u64,
    ino: u64,
    size: u64,
    mtime_ns: i128,
    ctime_ns: i128,
    config_generation: u64,
    import_id: Option<String>,
}
#[derive(Serialize, Deserialize)]
struct ListingDto {
    id: String,
    rel: String,
    size: u64,
    #[serde(with = "DatingDto")]
    dating: Dating,
    #[serde(with = "TokenDto")]
    token: ObservationToken,
    duplicate_producer_id: bool,
}
impl From<&SourceListing> for ListingDto {
    fn from(s: &SourceListing) -> Self {
        Self {
            id: s.id.clone(),
            rel: s.rel.clone(),
            size: s.size,
            dating: s.dating.clone(),
            token: s.token.clone(),
            duplicate_producer_id: s.warnings.contains(&Warning::DuplicateProducerId),
        }
    }
}
impl From<ListingDto> for SourceListing {
    fn from(s: ListingDto) -> Self {
        Self {
            id: s.id,
            rel: s.rel,
            size: s.size,
            dating: s.dating,
            token: s.token,
            warnings: if s.duplicate_producer_id {
                vec![Warning::DuplicateProducerId]
            } else {
                vec![]
            },
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "Unavailable")]
enum UnavailableDto {
    Missing,
    Denied,
    NotDownloaded,
    NotRegular,
    TooLarge,
    SourceChanging,
    Io,
    InvalidPath,
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "UnsupportedReason")]
enum UnsupportedDto {
    Layout,
    Schema,
    Kind,
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "ConflictKind")]
enum ConflictDto {
    IdMismatch,
    IdentityCollision,
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "ParseError")]
enum ParseErrorDto {
    FileTooLarge,
    Utf8,
    StrayCr,
    MixedNewlines,
    MissingFrontMatter,
    Unclosed,
    DuplicateKey,
    Alias,
    Tag,
    TooDeep,
    FrontMatterTooLarge,
    TooManyKeys,
    ScalarTooLarge,
    BodyTooLarge,
    BadLine,
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "SourceOutcome")]
enum OutcomeDto {
    #[serde(skip)]
    Ok(Box<SourceListing>),
    Unavailable(#[serde(with = "UnavailableDto")] Unavailable),
    Malformed(#[serde(with = "ParseErrorDto")] ParseError),
    Unsupported {
        #[serde(with = "UnsupportedDto")]
        reason: UnsupportedReason,
        viewable: bool,
    },
    Conflict(#[serde(with = "ConflictDto")] ConflictKind),
}
#[derive(Serialize, Deserialize)]
struct Failure(String, #[serde(with = "OutcomeDto")] SourceOutcome);
#[derive(Serialize, Deserialize)]
struct Included(usize, #[serde(with = "InstantDto")] Instant);
#[derive(Serialize, Deserialize)]
struct Snapshot {
    version: u8,
    #[serde(default)]
    navigation: Option<NavigationDto>,
    #[serde(default)]
    readable: Vec<ListingDto>,
    listings: Vec<ListingDto>,
    included: Vec<Included>,
    warnings: Vec<usize>,
    misplaced: Vec<String>,
    failures: Vec<Failure>,
    collisions: Vec<Vec<String>>,
    incomplete: bool,
    entries_seen: usize,
    candidates_seen: usize,
}

#[derive(Serialize, Deserialize)]
struct LocalDto {
    date: String,
    offset_seconds: i32,
    datetime: String,
}
impl From<&LocalTime> for LocalDto {
    fn from(t: &LocalTime) -> Self {
        Self {
            date: t.date.to_string(),
            offset_seconds: t.offset_seconds,
            datetime: t.datetime.clone(),
        }
    }
}
impl TryFrom<LocalDto> for LocalTime {
    type Error = ImportError;
    fn try_from(t: LocalDto) -> Result<Self, Self::Error> {
        Ok(Self {
            date: diary_domain::authentic::DiaryDate::parse(&t.date).ok_or(ImportError::Storage)?,
            offset_seconds: t.offset_seconds,
            datetime: t.datetime,
        })
    }
}
#[derive(Serialize, Deserialize)]
struct GroupDto {
    id: String,
    alias: String,
    kind: String,
}
#[derive(Serialize, Deserialize)]
struct DisplayDto {
    group_id: String,
    header: Option<LocalDto>,
    filename: Option<LocalDto>,
    included: Option<LocalDto>,
    authority: Option<String>,
}
#[derive(Serialize, Deserialize)]
struct NavigationDto {
    groups: Vec<GroupDto>,
    sources: BTreeMap<String, DisplayDto>,
    failure_groups: Vec<String>,
    collision_groups: Vec<String>,
}
impl From<&Navigation> for NavigationDto {
    fn from(n: &Navigation) -> Self {
        Self {
            groups: n
                .groups
                .iter()
                .map(|g| GroupDto {
                    id: g.id.clone(),
                    alias: g.alias.clone(),
                    kind: g.kind.clone(),
                })
                .collect(),
            sources: n
                .sources
                .iter()
                .map(|(id, d)| {
                    (
                        id.clone(),
                        DisplayDto {
                            group_id: d.group_id.clone(),
                            header: d.header.as_ref().map(Into::into),
                            filename: d.filename.as_ref().map(Into::into),
                            included: d.included.as_ref().map(Into::into),
                            authority: d.authority.map(|a| {
                                match a {
                                    Authority::Header => "header",
                                    Authority::Filename => "filename",
                                }
                                .into()
                            }),
                        },
                    )
                })
                .collect(),
            failure_groups: n.failure_groups.clone(),
            collision_groups: n.collision_groups.clone(),
        }
    }
}
impl TryFrom<NavigationDto> for Navigation {
    type Error = ImportError;
    fn try_from(n: NavigationDto) -> Result<Self, Self::Error> {
        Ok(Self {
            groups: n
                .groups
                .into_iter()
                .map(|g| SourceGroup {
                    id: g.id,
                    alias: g.alias,
                    kind: g.kind,
                })
                .collect(),
            sources: n
                .sources
                .into_iter()
                .map(|(id, d)| {
                    Ok((
                        id,
                        SourceDisplay {
                            group_id: d.group_id,
                            header: d.header.map(TryInto::try_into).transpose()?,
                            filename: d.filename.map(TryInto::try_into).transpose()?,
                            included: d.included.map(TryInto::try_into).transpose()?,
                            authority: match d.authority.as_deref() {
                                Some("header") => Some(Authority::Header),
                                Some("filename") => Some(Authority::Filename),
                                None => None,
                                _ => return Err(ImportError::Storage),
                            },
                        },
                    ))
                })
                .collect::<Result<_, _>>()?,
            failure_groups: n.failure_groups,
            collision_groups: n.collision_groups,
        })
    }
}

struct Bounded(Vec<u8>);
impl Write for Bounded {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > MAX_BYTES - self.0.len() {
            return Err(std::io::Error::other("snapshot limit"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub(super) fn encode(r: &ImportListing) -> Result<Vec<u8>, ImportError> {
    let positions: BTreeMap<_, _> = r
        .scan
        .listings
        .iter()
        .enumerate()
        .map(|(i, s)| (&s.id, i))
        .collect();
    let position = |id: &String| positions.get(id).copied().ok_or(ImportError::Storage);
    let snapshot = Snapshot {
        version: 2,
        navigation: r.navigation.as_ref().map(NavigationDto::from),
        readable: r.scan.readable.iter().map(ListingDto::from).collect(),
        listings: r.scan.listings.iter().map(ListingDto::from).collect(),
        included: r
            .included
            .iter()
            .map(|s| position(&s.source.id).map(|i| Included(i, s.instant)))
            .collect::<Result<_, _>>()?,
        warnings: r
            .warnings
            .iter()
            .map(|s| position(&s.id))
            .collect::<Result<_, _>>()?,
        misplaced: r.misplaced.clone(),
        failures: r
            .scan
            .failures
            .iter()
            .map(|(key, outcome)| Failure(key.clone(), outcome.clone()))
            .collect(),
        collisions: r.scan.collisions.clone(),
        incomplete: r.scan.incomplete,
        entries_seen: r.scan.entries_seen,
        candidates_seen: r.scan.candidates_seen,
    };
    let mut output = Bounded(Vec::new());
    serde_json::to_writer(&mut output, &snapshot).map_err(|e| {
        if e.is_io() {
            ImportError::Limit
        } else {
            ImportError::Storage
        }
    })?;
    Ok(output.0)
}

pub(super) fn decode(context: ImportContext, bytes: &[u8]) -> Result<ImportListing, ImportError> {
    if bytes.len() > MAX_BYTES {
        return Err(ImportError::Limit);
    }
    let s: Snapshot = serde_json::from_slice(bytes).map_err(|_| ImportError::Storage)?;
    if !matches!(s.version, 1 | 2)
        || s.listings.len() + s.readable.len() > 10_000
        || s.entries_seen > 100_000
        || s.candidates_seen > 10_000
    {
        return Err(ImportError::Storage);
    }
    let listings: Vec<SourceListing> = s.listings.into_iter().map(Into::into).collect();
    let readable: Vec<SourceListing> = s.readable.into_iter().map(Into::into).collect();
    if listings.iter().chain(&readable).any(|s| {
        s.token.import_id.as_deref() != Some(&context.id)
            || s.token.config_generation != context.config_generation
    }) {
        return Err(ImportError::Storage);
    }
    let source = |i: usize| listings.get(i).cloned().ok_or(ImportError::Storage);
    let included = s
        .included
        .into_iter()
        .map(|Included(i, instant)| source(i).map(|source| IncludedSource { source, instant }))
        .collect::<Result<_, _>>()?;
    let warnings = s
        .warnings
        .into_iter()
        .map(source)
        .collect::<Result<_, _>>()?;
    let navigation = s.navigation.map(Navigation::try_from).transpose()?;
    if let Some(n) = &navigation
        && (n.failure_groups.len() != s.failures.len()
            || n.collision_groups.len() != s.collisions.len())
    {
        return Err(ImportError::Storage);
    }
    Ok(ImportListing {
        navigation,
        context,
        included,
        warnings,
        misplaced: s.misplaced,
        scan: ScanReport {
            listings,
            readable,
            failures: s
                .failures
                .into_iter()
                .map(|Failure(key, outcome)| (key, outcome))
                .collect(),
            collisions: s.collisions,
            incomplete: s.incomplete,
            entries_seen: s.entries_seen,
            candidates_seen: s.candidates_seen,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use diary_domain::{authentic::DiaryDate, source::window::ImportZone};

    fn empty() -> ImportListing {
        ImportListing {
            navigation: None,
            context: ImportContext {
                id: "codec".into(),
                date: DiaryDate::parse("2024-02-29").unwrap(),
                zone: ImportZone {
                    name: "UTC".into(),
                    database_version: "test".into(),
                },
                config_generation: 7,
                completed_at: None,
            },
            included: vec![],
            warnings: vec![],
            misplaced: vec![],
            scan: ScanReport::default(),
        }
    }
    #[test]
    fn failures_collisions_and_incomplete_round_trip_and_encoding_is_bounded() {
        let mut r = empty();
        r.scan.incomplete = true;
        r.scan.entries_seen = 42;
        r.scan.candidates_seen = 8;
        r.scan.collisions = vec![vec!["A.md".into(), "a.md".into()]];
        r.scan.failures = vec![
            (
                "a.md".into(),
                SourceOutcome::Malformed(ParseError::DuplicateKey),
            ),
            (
                "b.md".into(),
                SourceOutcome::Unavailable(Unavailable::NotDownloaded),
            ),
            (
                "c.md".into(),
                SourceOutcome::Unsupported {
                    reason: UnsupportedReason::Schema,
                    viewable: true,
                },
            ),
            (
                "d.md".into(),
                SourceOutcome::Conflict(ConflictKind::IdentityCollision),
            ),
        ];
        let bytes = encode(&r).unwrap();
        let decoded = decode(r.context.clone(), &bytes).unwrap();
        assert_eq!(decoded.scan.failures, r.scan.failures);
        assert_eq!(decoded.scan.collisions, r.scan.collisions);
        assert!(decoded.scan.incomplete);
        assert_eq!(decoded.scan.entries_seen, 42);
        assert_eq!(decoded.scan.candidates_seen, 8);
        assert!(matches!(
            decode(r.context.clone(), b"invalid"),
            Err(ImportError::Storage)
        ));
        let mut invalid: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        invalid["warnings"] = serde_json::json!([0]);
        assert!(matches!(
            decode(r.context.clone(), &serde_json::to_vec(&invalid).unwrap()),
            Err(ImportError::Storage)
        ));
        r.scan.failures[0].0 = "x".repeat(MAX_BYTES);
        assert_eq!(encode(&r), Err(ImportError::Limit));
    }
}

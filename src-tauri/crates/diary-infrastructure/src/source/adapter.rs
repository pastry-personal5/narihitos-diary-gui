use super::{
    read::{Observation, read_fresh},
    root::{ResolvedRoot, source_id},
};
use crate::config::SourceKind;
use diary_application::source::*;
use diary_domain::source::{
    front_matter::{Parsed, Value, parse},
    path::SourcePath,
    stamp::{self, Dating, StampError},
};
use sha2::{Digest, Sha256};

pub(super) trait SourceAdapter {
    fn filename(&self, stem: &str, dir: (i32, u8, u8)) -> Result<stamp::Instant, StampError>;
    fn identity<'a>(&self, stem: &'a str) -> Option<&'a str>;
    fn unsupported(&self, parsed: &Parsed) -> Option<UnsupportedReason>;
}
fn adapter(kind: &SourceKind) -> &dyn SourceAdapter {
    match kind {
        SourceKind::Ayumi => &super::ayumi::Ayumi,
        SourceKind::Narihito => &super::narihito::Narihito,
    }
}
pub(crate) fn scalar<'a>(p: &'a Parsed, key: &str) -> Option<&'a str> {
    p.fields.iter().find_map(|(k, v)| {
        if k == key {
            if let Value::Scalar(s) = v {
                Some(s.as_str())
            } else {
                None
            }
        } else {
            None
        }
    })
}
fn layout(rel: &str) -> Option<((i32, u8, u8), &str)> {
    let p: Vec<_> = rel.split('/').collect();
    if p.len() != 4
        || p[0].len() != 4
        || p[1].len() != 2
        || p[2].len() != 2
        || !p[..3].iter().all(|s| s.bytes().all(|b| b.is_ascii_digit()))
    {
        return None;
    }
    let dir = (p[0].parse().ok()?, p[1].parse().ok()?, p[2].parse().ok()?);
    Some((dir, p[3].strip_suffix(".md")?))
}
fn token(
    root: &ResolvedRoot,
    rel: &SourcePath,
    obs: &Observation,
    generation: u64,
) -> ObservationToken {
    ObservationToken {
        root_namespace: Sha256::digest(root.canonical.to_str().expect("validated root").as_bytes())
            .into(),
        key: rel.key.clone(),
        sha256: obs.sha256,
        dev: obs.dev,
        ino: obs.ino,
        size: obs.size,
        mtime_ns: obs.mtime_ns,
        ctime_ns: obs.ctime_ns,
        config_generation: generation,
        import_id: None,
    }
}
pub(crate) fn interpret(
    root: &ResolvedRoot,
    kind: &SourceKind,
    rel: &SourcePath,
    obs: &Observation,
    generation: u64,
    p: &Parsed,
) -> (SourceOutcome, SourceListing) {
    let header = scalar(p, "created_at")
        .ok_or(StampError::Format)
        .and_then(stamp::header_created_at);
    let lay = layout(&rel.spelling);
    let adapter = adapter(kind);
    let filename = lay
        .ok_or(StampError::Format)
        .and_then(|(d, s)| adapter.filename(s, d));
    let listing = SourceListing {
        id: source_id(root, rel),
        rel: rel.spelling.clone(),
        size: obs.size,
        dating: Dating::from_evidence(header, filename),
        warnings: vec![],
        token: token(root, rel, obs, generation),
    };
    let unsupported = |reason| SourceOutcome::Unsupported {
        reason,
        viewable: true,
    };
    let Some((_, stem)) = lay else {
        return (unsupported(UnsupportedReason::Layout), listing);
    };
    // Timestamp/calendar errors are Dating evidence, not a hidden file. Structural
    // filename shape is the layout gate; independent authorities remain inspectable.
    let Some(expected) = adapter.identity(stem) else {
        return (unsupported(UnsupportedReason::Layout), listing);
    };
    if let Some(reason) = adapter.unsupported(p) {
        return (unsupported(reason), listing);
    }
    if scalar(p, "id") != Some(expected) {
        return (SourceOutcome::Conflict(ConflictKind::IdMismatch), listing);
    }
    (SourceOutcome::Ok(Box::new(listing.clone())), listing)
}
pub(crate) fn outcome(
    root: &ResolvedRoot,
    kind: &SourceKind,
    rel: &SourcePath,
    generation: u64,
) -> (SourceOutcome, Option<String>, Option<SourceListing>) {
    let obs = match read_fresh(root, rel) {
        Ok(o) => o,
        Err(e) => return (SourceOutcome::Unavailable(e), None, None),
    };
    let p = match parse(&obs.bytes) {
        Ok(p) => p,
        Err(e) => return (SourceOutcome::Malformed(e), None, None),
    };
    let producer = scalar(&p, "id").map(str::to_owned);
    let (outcome, listing) = interpret(root, kind, rel, &obs, generation, &p);
    (outcome, producer, Some(listing))
}
/// Re-read an observed path. Unsupported/conflicting but well-formed files remain
/// viewable. Any observation change (including a config reload) requires refresh.
pub fn read_view(
    root: &ResolvedRoot,
    kind: &SourceKind,
    rel: &SourcePath,
    generation: u64,
    expected: &ObservationToken,
) -> Result<SourceView, ViewError> {
    let obs =
        read_fresh(root, rel).map_err(|e| ViewError::Outcome(SourceOutcome::Unavailable(e)))?;
    let mut fresh = token(root, rel, &obs, generation);
    fresh.import_id.clone_from(&expected.import_id);
    if &fresh != expected {
        return Err(ViewError::Stale);
    }
    let p = parse(&obs.bytes).map_err(|e| ViewError::Outcome(SourceOutcome::Malformed(e)))?;
    let (_, mut listing) = interpret(root, kind, rel, &obs, generation, &p);
    listing.token.import_id.clone_from(&expected.import_id);
    Ok(SourceView {
        listing,
        body: p.body,
        raw_front_matter: p.raw_front_matter,
        fields: p.fields,
        newline: p.newline,
        bom: p.bom,
    })
}
/// Obtain the body-less observation even when schema/layout/identity is unsupported.
/// Callers can retain its token to open a bounded, observation-checked view.
pub fn observe(
    root: &ResolvedRoot,
    kind: &SourceKind,
    rel: &SourcePath,
    generation: u64,
) -> Result<(SourceOutcome, SourceListing), SourceOutcome> {
    let obs = read_fresh(root, rel).map_err(SourceOutcome::Unavailable)?;
    let p = parse(&obs.bytes).map_err(SourceOutcome::Malformed)?;
    Ok(interpret(root, kind, rel, &obs, generation, &p))
}

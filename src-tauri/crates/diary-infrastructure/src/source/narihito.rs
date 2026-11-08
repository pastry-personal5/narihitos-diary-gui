use super::adapter::{SourceAdapter, scalar};
use super::{narihito_write::check, write::WRITE_BODY_VERIFIED};
use diary_application::source::{TrashReason, UnsupportedReason, WriteReason};
use diary_domain::source::{
    front_matter::Parsed,
    stamp::{self, Instant, StampError},
};
pub(super) struct Narihito;
impl SourceAdapter for Narihito {
    fn filename(&self, stem: &str, dir: (i32, u8, u8)) -> Result<Instant, StampError> {
        stamp::narihito_stem(stem, dir).map(|(t, _)| t)
    }
    fn identity<'a>(&self, stem: &'a str) -> Option<&'a str> {
        if stem.len() == 56 && stem.as_bytes().get(19) == Some(&b'-') {
            stem.get(20..).filter(|s| stamp::valid_uuid(s))
        } else {
            None
        }
    }
    fn unsupported(&self, parsed: &Parsed) -> Option<UnsupportedReason> {
        if !matches!(scalar(parsed, "schema_version"), Some("1" | "2"))
            || (scalar(parsed, "schema_version") == Some("2")
                && scalar(parsed, "creation_method") != Some("manual"))
        {
            Some(UnsupportedReason::Schema)
        } else if scalar(parsed, "kind") != Some("ndm.diary-snippet") {
            Some(UnsupportedReason::Kind)
        } else {
            None
        }
    }
    fn write_policy(&self, bytes: &[u8], rel: &str) -> Result<(), WriteReason> {
        check(bytes, rel).map_err(|_| WriteReason::UnsupportedFormat)?;
        if WRITE_BODY_VERIFIED {
            Ok(())
        } else {
            Err(WriteReason::WriterUnverified)
        }
    }
    fn view_body(&self, body: String) -> String {
        // The blank line after the closing delimiter is the producer's separator, not text.
        match body.strip_prefix('\n') {
            Some(text) => text.to_owned(),
            None => body,
        }
    }
    fn trash_policy(&self, bytes: &[u8], rel: &str) -> Result<(), TrashReason> {
        let checked = check(bytes, rel).map_err(|_| TrashReason::UnsupportedFormat)?;
        if !checked.recording {
            return Err(TrashReason::UnsupportedFormat);
        }
        if super::trash::TRASH_PAIR_VERIFIED {
            Ok(())
        } else {
            Err(TrashReason::TrashUnverified)
        }
    }
}

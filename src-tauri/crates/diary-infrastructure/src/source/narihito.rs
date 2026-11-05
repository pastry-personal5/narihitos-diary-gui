use super::adapter::{SourceAdapter, scalar};
use diary_application::source::UnsupportedReason;
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
        if scalar(parsed, "schema_version") != Some("1") {
            Some(UnsupportedReason::Schema)
        } else if scalar(parsed, "kind") != Some("ndm.diary-snippet") {
            Some(UnsupportedReason::Kind)
        } else {
            None
        }
    }
}

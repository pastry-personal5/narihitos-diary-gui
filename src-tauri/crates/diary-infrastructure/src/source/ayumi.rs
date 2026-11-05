use super::adapter::SourceAdapter;
use diary_application::source::UnsupportedReason;
use diary_domain::source::{
    front_matter::Parsed,
    stamp::{self, Instant, StampError},
};
pub(super) struct Ayumi;
impl SourceAdapter for Ayumi {
    fn filename(&self, stem: &str, dir: (i32, u8, u8)) -> Result<Instant, StampError> {
        stamp::ayumi_stem(stem, dir)
    }
    fn identity<'a>(&self, stem: &'a str) -> Option<&'a str> {
        (stem.len() == 23 && stem.as_bytes().get(19) == Some(&b'-')).then_some(stem)
    }
    fn unsupported(&self, _parsed: &Parsed) -> Option<UnsupportedReason> {
        None
    }
}

//! Shared shape checks and the `invalid_input` error used at the bridge's command boundary.
use super::AuthenticError;
use diary_application::operations::OperationId;

pub(super) fn invalid() -> AuthenticError {
    AuthenticError::new("invalid_input")
}
pub(super) fn uuid(s: &str) -> bool {
    OperationId::parse(s).is_some()
}
/// Lowercase hex of exactly `len` characters.
pub(super) fn digest(s: &str, len: usize) -> bool {
    s.len() == len
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
pub(super) fn sha256(s: &str) -> bool {
    digest(s, 64)
}
/// The server owns calendar validity; this only keeps malformed text off the wire.
pub(super) fn date_shape(s: &str) -> bool {
    s.len() == 10
        && s.bytes().enumerate().all(|(i, b)| {
            if matches!(i, 4 | 7) {
                b == b'-'
            } else {
                b.is_ascii_digit()
            }
        })
}

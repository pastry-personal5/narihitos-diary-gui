//! Diagnostic logging: bounded, private, asynchronous (see docs/architecture/logging.md).
pub mod clock;
pub mod codec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogError {
    Unavailable,
    Capped,
    Oversize,
    Locked,
}

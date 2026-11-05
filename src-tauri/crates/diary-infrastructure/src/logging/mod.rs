//! Diagnostic logging: bounded, private, asynchronous (see docs/architecture/logging.md).
pub mod clock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogError {
    Unavailable,
    Capped,
    Oversize,
    Locked,
}

//! Diagnostic logging and descriptor-relative private files (see docs/architecture/logging.md).
pub mod clock;
pub mod codec;
pub mod private_fs;
pub mod store;
pub mod trace;
pub mod worker;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogError {
    Unavailable,
    Capped,
    Oversize,
    Locked,
}

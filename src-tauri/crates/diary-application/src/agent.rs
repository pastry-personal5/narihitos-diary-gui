//! Agent port: typed requests, closed events and errors, bounds, the cancel-epoch run guard and
//! the per-action capability model. Pure values only: no I/O, JSON, prompts or process control.
//! JSON encoding, output validation and prompt assets live in `diary-infrastructure::agent`.

mod capability;
mod error;
mod kind;
mod packet;
mod run;
mod status;
#[cfg(test)]
mod tests;

pub use capability::*;
pub use error::*;
pub use kind::*;
pub use packet::*;
pub use run::*;
pub use status::*;

pub const SCHEMA_VERSION: u32 = 1;
pub const MAX_PACKET_BYTES: usize = 256 * 1024;
pub const MAX_USER_MESSAGE_BYTES: usize = 64 * 1024;
pub const MAX_TURNS: usize = 100;
pub const MAX_TRANSCRIPT_BYTES: usize = 128 * 1024;
pub const MAX_TEXT_BYTES: usize = 1024 * 1024; // draft and chat reply
pub const MAX_RESULT_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_PROCESS_OUTPUT_BYTES: usize = 10 * 1024 * 1024; // consumed by P1-M17
pub const MAX_PROTOCOL_RECORD_BYTES: usize = 1024 * 1024; // consumed by P1-M17
pub const INACTIVITY_SECS: u64 = 120; // consumed by P1-M17
pub const WALL_SECS: u64 = 600; // consumed by P1-M17

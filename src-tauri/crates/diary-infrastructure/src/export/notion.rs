//! The Notion export adapter. Transport, retry, wire format and recovery are separate modules;
//! the HTTP implementation is injected so this crate carries no network dependency.
pub mod adapter;
pub mod client;
#[cfg(any(test, feature = "fault-injection"))]
pub mod fake;
pub mod http;
pub mod illustrated;
pub mod steps;
pub mod wire;

pub use adapter::Notion;

#[cfg(test)]
mod tests;

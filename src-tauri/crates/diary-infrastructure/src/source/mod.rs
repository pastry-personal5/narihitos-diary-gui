//! Read-only, bounded producer source adapters. No server or UI exposure.
pub mod adapter;
pub mod read;
pub mod root;
pub mod scan;
#[cfg(test)]
mod tests;

mod ayumi;
mod narihito;

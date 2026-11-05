//! Operating-system adapters: config, workspace lock/initialization and the SQLite operation ledger.
#[cfg(not(target_os = "macos"))]
compile_error!("Diary targets macOS only: coordinated file access needs NSFileCoordinator");

pub mod authentic;
pub mod authentic_fs;
pub mod config;
pub mod coordination;
pub mod ledger;
pub mod lifecycle;
pub mod sqlite;
pub mod watch;
pub mod workspace;

pub mod source;

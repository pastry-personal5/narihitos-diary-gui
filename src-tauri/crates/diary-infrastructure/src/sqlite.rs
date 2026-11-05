//! The one SQLite connection: private file, strict pragmas, checksummed migrations, integrity
//! checks. A mutex serializes every call, which gives the contract's single serialized writer;
//! callers on an async executor must use `spawn_blocking`.
use diary_logging::private_fs::PrivateDir;
use rusqlite::{Connection, OpenFlags, TransactionBehavior};
use sha2::{Digest, Sha256};
use std::{path::Path, sync::Mutex};

pub const DB_FILE: &str = "state.sqlite3";
/// Applied in order; an entry is immutable once released (its checksum is recorded).
pub const MIGRATIONS: &[(u32, &str)] = &[
    (1, include_str!("sqlite/0001_init.sql")),
    (2, include_str!("sqlite/0002_catalog.sql")),
    (3, include_str!("sqlite/0003_imports.sql")),
    (4, include_str!("sqlite/0004_import_snapshots.sql")),
];
const REQUIRED_TABLES: &[&str] = &[
    "schema_migrations",
    "workspace_meta",
    "operations",
    "operation_resources",
    "operation_steps",
    "catalog_authentic",
    "source_identities",
    "import_contexts",
    "time_resolutions",
    "import_snapshots",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbError {
    /// The file does not exist.
    Missing,
    /// Not a database, failed integrity, missing tables, or an altered migration.
    Damaged,
    /// Written by a newer schema than this build knows.
    Newer,
    Storage,
}

pub struct Db {
    conn: Mutex<Connection>,
}

fn checksum(sql: &str) -> String {
    Sha256::digest(sql.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

impl Db {
    /// Creates the private file (0600, exclusive) and migrates it. WAL/SHM inherit its mode.
    pub fn create(directory: &PrivateDir, path: &Path) -> Result<Self, DbError> {
        directory.create(DB_FILE).map_err(|_| DbError::Storage)?;
        let db = Self::connect(path)?;
        db.with(|c| migrate(c, MIGRATIONS))?;
        Ok(db)
    }

    /// Opens an existing file after proving it is a private regular file and intact. A file that
    /// was never migrated (`user_version` 0, e.g. truncated to nothing) is initialized only when
    /// `fresh_ok`; otherwise it is damage and is left byte-for-byte as found.
    pub fn open_existing(
        directory: &PrivateDir,
        path: &Path,
        fresh_ok: bool,
    ) -> Result<Self, DbError> {
        match directory.is_regular(DB_FILE) {
            Ok(true) => (),
            Ok(false) => return Err(DbError::Damaged),
            Err(_) => return Err(DbError::Missing),
        }
        directory.read(DB_FILE).map_err(|_| DbError::Damaged)?; // 0600, owned, not a link
        if !fresh_ok && schema_version(path)? == 0 {
            return Err(DbError::Damaged);
        }
        let db = Self::connect(path)?;
        db.with(|c| {
            migrate(c, MIGRATIONS)?;
            integrity(c)
        })?;
        Ok(db)
    }

    fn connect(path: &Path) -> Result<Self, DbError> {
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX
                | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )
        .map_err(|_| DbError::Damaged)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))
            .map_err(|_| DbError::Storage)?;
        // A file that is not a database fails at the first pragma.
        let journal: String = conn
            .query_row("PRAGMA journal_mode = WAL", [], |r| r.get(0))
            .map_err(|_| DbError::Damaged)?;
        conn.pragma_update(None, "synchronous", "FULL")
            .and_then(|()| conn.pragma_update(None, "foreign_keys", "ON"))
            .map_err(|_| DbError::Storage)?;
        let read = |name: &str| -> Result<i64, DbError> {
            conn.pragma_query_value(None, name, |r| r.get(0))
                .map_err(|_| DbError::Storage)
        };
        // Read the settings back: a silently ignored pragma would weaken durability.
        if journal != "wal" || read("synchronous")? != 2 || read("foreign_keys")? != 1 {
            return Err(DbError::Storage);
        }
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn with<T>(
        &self,
        f: impl FnOnce(&mut Connection) -> Result<T, DbError>,
    ) -> Result<T, DbError> {
        f(&mut *self.conn.lock().map_err(|_| DbError::Storage)?)
    }

    /// One `BEGIN IMMEDIATE` transaction; any error rolls it back.
    pub fn transact<T>(
        &self,
        f: impl FnOnce(&rusqlite::Transaction) -> Result<T, DbError>,
    ) -> Result<T, DbError> {
        self.with(|c| {
            let tx = c
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(|_| DbError::Storage)?;
            let value = f(&tx)?;
            tx.commit().map_err(|_| DbError::Storage)?;
            Ok(value)
        })
    }
}

/// `user_version` read through a read-only handle, so nothing is written to a file that may be
/// empty or foreign (opening for write would already set its journal mode).
fn schema_version(path: &Path) -> Result<u32, DbError> {
    let probe = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )
    .map_err(|_| DbError::Damaged)?;
    probe
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .map_err(|_| DbError::Damaged)
}

/// Applies unapplied migrations, each in one transaction that also advances `user_version`.
pub fn migrate(conn: &mut Connection, migrations: &[(u32, &str)]) -> Result<(), DbError> {
    let current: u32 = conn
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .map_err(|_| DbError::Damaged)?;
    let latest = migrations.last().map_or(0, |m| m.0);
    if current > latest {
        return Err(DbError::Newer);
    }
    for (version, sql) in migrations {
        if *version <= current {
            let recorded: String = conn
                .query_row(
                    "SELECT checksum FROM schema_migrations WHERE version = ?1",
                    [version],
                    |r| r.get(0),
                )
                .map_err(|_| DbError::Damaged)?;
            if recorded != checksum(sql) {
                return Err(DbError::Damaged);
            }
            continue;
        }
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| DbError::Storage)?;
        tx.execute_batch(sql).map_err(|_| DbError::Storage)?;
        tx.execute(
            "INSERT INTO schema_migrations (version, checksum) VALUES (?1, ?2)",
            (version, checksum(sql)),
        )
        .map_err(|_| DbError::Storage)?;
        tx.pragma_update(None, "user_version", version)
            .map_err(|_| DbError::Storage)?;
        tx.commit().map_err(|_| DbError::Storage)?;
    }
    Ok(())
}

fn integrity(conn: &Connection) -> Result<(), DbError> {
    let quick: String = conn
        .query_row("PRAGMA quick_check", [], |r| r.get(0))
        .map_err(|_| DbError::Damaged)?;
    let tables = REQUIRED_TABLES.iter().all(|t| {
        conn.query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [t],
            |_| Ok(()),
        )
        .is_ok()
    });
    if quick == "ok" && tables {
        Ok(())
    } else {
        Err(DbError::Damaged)
    }
}

#[cfg(test)]
mod tests;

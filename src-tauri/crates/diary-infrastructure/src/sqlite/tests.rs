use super::*;
use std::os::unix::fs::PermissionsExt;

fn private_dir() -> (tempfile::TempDir, PrivateDir, std::path::PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().canonicalize().unwrap().join(".diary");
    std::fs::create_dir(&path).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    let dir = PrivateDir::open(&path).unwrap();
    (temp, dir, path)
}
fn pragma(db: &Db, name: &str) -> i64 {
    db.with(|c| {
        c.pragma_query_value(None, name, |r| r.get(0))
            .map_err(|_| DbError::Storage)
    })
    .unwrap()
}

#[test]
fn create_is_private_wal_full_and_versioned() {
    let (_t, dir, path) = private_dir();
    let db = Db::create(&dir, &path.join(DB_FILE)).unwrap();
    let mode = std::fs::metadata(path.join(DB_FILE))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o7777, 0o600);
    assert_eq!(pragma(&db, "synchronous"), 2);
    assert_eq!(pragma(&db, "foreign_keys"), 1);
    assert_eq!(pragma(&db, "user_version"), MIGRATIONS.len() as i64);
    // WAL and SHM, once present, inherit the private mode.
    db.with(|c| {
        c.execute("INSERT INTO workspace_meta VALUES (1, 'w')", [])
            .map_err(|_| DbError::Storage)
    })
    .unwrap();
    for suffix in ["-wal", "-shm"] {
        let side = path.join(format!("{DB_FILE}{suffix}"));
        assert!(
            side.exists(),
            "{suffix} must exist while the connection is open"
        );
        assert_eq!(
            std::fs::metadata(side).unwrap().permissions().mode() & 0o7777,
            0o600
        );
    }
}

#[test]
fn reopen_is_a_no_op_and_second_create_is_refused() {
    let (_t, dir, path) = private_dir();
    drop(Db::create(&dir, &path.join(DB_FILE)).unwrap());
    let db = Db::open_existing(&dir, &path.join(DB_FILE), false).unwrap();
    assert_eq!(pragma(&db, "user_version"), MIGRATIONS.len() as i64);
    assert!(
        Db::create(&dir, &path.join(DB_FILE)).is_err(),
        "never replaces an existing file"
    );
}

#[test]
fn missing_file_is_missing_and_garbage_is_damaged() {
    let (_t, dir, path) = private_dir();
    assert_eq!(
        Db::open_existing(&dir, &path.join(DB_FILE), false).err(),
        Some(DbError::Missing)
    );
    let mut file = dir.create(DB_FILE).unwrap();
    std::io::Write::write_all(&mut file, &[0x42; 4096]).unwrap();
    assert_eq!(
        Db::open_existing(&dir, &path.join(DB_FILE), false).err(),
        Some(DbError::Damaged)
    );
}

#[test]
fn symlinked_database_is_refused() {
    let (temp, dir, path) = private_dir();
    let target = temp.path().join("elsewhere.sqlite3");
    std::fs::write(&target, b"").unwrap();
    std::os::unix::fs::symlink(&target, path.join(DB_FILE)).unwrap();
    assert!(Db::open_existing(&dir, &path.join(DB_FILE), false).is_err());
}

#[test]
fn altered_migration_checksum_is_damage() {
    let (_t, dir, path) = private_dir();
    let db = Db::create(&dir, &path.join(DB_FILE)).unwrap();
    let mut altered = MIGRATIONS.to_vec();
    altered[0] = (1, "CREATE TABLE schema_migrations (version INTEGER);");
    assert_eq!(db.with(|c| migrate(c, &altered)), Err(DbError::Damaged));
}

#[test]
fn newer_schema_is_refused_not_downgraded() {
    let (_t, dir, path) = private_dir();
    let db = Db::create(&dir, &path.join(DB_FILE)).unwrap();
    db.with(|c| {
        c.pragma_update(None, "user_version", 99)
            .map_err(|_| DbError::Storage)
    })
    .unwrap();
    drop(db);
    assert_eq!(
        Db::open_existing(&dir, &path.join(DB_FILE), false).err(),
        Some(DbError::Newer)
    );
}

#[test]
fn failed_migration_rolls_back_whole() {
    let (_t, dir, path) = private_dir();
    let db = Db::create(&dir, &path.join(DB_FILE)).unwrap();
    let version = MIGRATIONS.last().unwrap().0 + 1;
    let mut next = MIGRATIONS.to_vec();
    next.push((
        version,
        "CREATE TABLE half_done (x INTEGER) STRICT; CREATE TABLE half_done (y INTEGER);",
    ));
    assert_eq!(db.with(|c| migrate(c, &next)), Err(DbError::Storage));
    assert_eq!(pragma(&db, "user_version"), MIGRATIONS.len() as i64);
    let leaked: i64 = db
        .with(|c| {
            c.query_row(
                "SELECT count(*) FROM sqlite_master WHERE name = 'half_done'",
                [],
                |r| r.get(0),
            )
            .map_err(|_| DbError::Storage)
        })
        .unwrap();
    assert_eq!(leaked, 0);
    // A later correct migration still applies.
    let mut good = MIGRATIONS.to_vec();
    good.push((version, "CREATE TABLE later (x INTEGER) STRICT;"));
    db.with(|c| migrate(c, &good)).unwrap();
    assert_eq!(pragma(&db, "user_version"), i64::from(version));
}

#[test]
fn missing_required_table_is_damage() {
    let (_t, dir, path) = private_dir();
    let db = Db::create(&dir, &path.join(DB_FILE)).unwrap();
    db.with(|c| {
        c.execute_batch("DROP TABLE operation_steps;")
            .map_err(|_| DbError::Storage)
    })
    .unwrap();
    drop(db);
    assert_eq!(
        Db::open_existing(&dir, &path.join(DB_FILE), false).err(),
        Some(DbError::Damaged)
    );
}

#[test]
fn m10_upgrades_the_released_schema_without_changing_existing_rows() {
    let (_t, dir, path) = private_dir();
    dir.create(DB_FILE).unwrap();
    let mut conn = Connection::open(path.join(DB_FILE)).unwrap();
    migrate(&mut conn, &MIGRATIONS[..2]).unwrap();
    conn.execute(
        "INSERT INTO workspace_meta VALUES (1, 'existing-workspace')",
        [],
    )
    .unwrap();
    drop(conn);
    let db = Db::open_existing(&dir, &path.join(DB_FILE), false).unwrap();
    let id = db
        .with(|c| {
            c.query_row("SELECT workspace_id FROM workspace_meta", [], |r| {
                r.get::<_, String>(0)
            })
            .map_err(|_| DbError::Storage)
        })
        .unwrap();
    assert_eq!(id, "existing-workspace");
    assert_eq!(pragma(&db, "user_version"), MIGRATIONS.len() as i64);
}

#[test]
fn snapshots_migrate_legacy_contexts_without_reconstructing_sources() {
    use diary_application::imports::{ImportError, ImportRepository};
    let (_t, dir, path) = private_dir();
    dir.create(DB_FILE).unwrap();
    let mut conn = Connection::open(path.join(DB_FILE)).unwrap();
    migrate(&mut conn, &MIGRATIONS[..3]).unwrap();
    conn.execute(
        "INSERT INTO import_contexts VALUES ('legacy', '2024-02-29', 'UTC', '2026e', 7, 99)",
        [],
    )
    .unwrap();
    drop(conn);
    let db = Db::open_existing(&dir, &path.join(DB_FILE), false).unwrap();
    assert_eq!(db.context("legacy").unwrap().completed_at, Some(99));
    assert!(matches!(db.snapshot("legacy"), Err(ImportError::Stale)));
    assert_eq!(pragma(&db, "user_version"), MIGRATIONS.len() as i64);
}

// T03: the export migration is additive over the released 0005 schema.
#[test]
fn m25_upgrades_schema_five_keeping_rows_and_adding_export_tables() {
    let (_t, dir, path) = private_dir();
    dir.create(DB_FILE).unwrap();
    let mut conn = Connection::open(path.join(DB_FILE)).unwrap();
    migrate(&mut conn, &MIGRATIONS[..5]).unwrap();
    conn.execute("INSERT INTO workspace_meta VALUES (1, 'kept')", [])
        .unwrap();
    drop(conn);
    let db = Db::open_existing(&dir, &path.join(DB_FILE), false).unwrap();
    assert_eq!(pragma(&db, "user_version"), MIGRATIONS.len() as i64);
    let (kept, tables): (String, i64) = db
        .with(|c| {
            let kept = c
                .query_row("SELECT workspace_id FROM workspace_meta", [], |r| r.get(0))
                .map_err(|_| DbError::Storage)?;
            let tables = c
                .query_row(
                    "SELECT count(*) FROM sqlite_master WHERE name IN \
                     ('export_operations','export_receipts','export_checkpoints','export_authorizations')",
                    [],
                    |r| r.get(0),
                )
                .map_err(|_| DbError::Storage)?;
            Ok((kept, tables))
        })
        .unwrap();
    assert_eq!((kept.as_str(), tables), ("kept", 4));
}

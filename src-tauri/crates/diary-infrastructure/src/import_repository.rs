//! Durable provenance and timestamp choices, separate from transient source bodies.
use crate::sqlite::{Db, DbError};
use diary_application::imports::{
    Authority, ImportContext, ImportError, ImportRepository, Resolution,
};
use diary_domain::{
    authentic::DiaryDate,
    source::{
        stamp::{Instant, Precision},
        window::ImportZone,
    },
};
use rusqlite::OptionalExtension;

fn storage(_: DbError) -> ImportError {
    ImportError::Storage
}

impl ImportRepository for Db {
    fn bind_source(&self, id: &str, namespace: &[u8; 32], key: &str) -> Result<(), ImportError> {
        self.transact(|tx| {
            tx.execute("INSERT INTO source_identities VALUES (?1, ?2, ?3) ON CONFLICT(source_id) DO NOTHING", (id, namespace.as_slice(), key)).map_err(|_| DbError::Storage)?;
            let matches = tx.query_row("SELECT namespace = ?2 AND path_key = ?3 FROM source_identities WHERE source_id = ?1", (id, namespace.as_slice(), key), |r| r.get::<_, bool>(0)).map_err(|_| DbError::Storage)?;
            if matches { Ok(()) } else { Err(DbError::Storage) }
        }).map_err(storage)
    }
    fn bind_sources(&self, items: &[(String, [u8; 32], String)]) -> Result<(), ImportError> {
        self.transact(|tx| {
            for (id, namespace, key) in items {
                tx.execute("INSERT INTO source_identities VALUES (?1, ?2, ?3) ON CONFLICT(source_id) DO NOTHING", (id, namespace.as_slice(), key)).map_err(|_| DbError::Storage)?;
                let matches = tx.query_row("SELECT namespace = ?2 AND path_key = ?3 FROM source_identities WHERE source_id = ?1", (id, namespace.as_slice(), key), |r| r.get::<_, bool>(0)).map_err(|_| DbError::Storage)?;
                if !matches {
                    return Err(DbError::Storage);
                }
            }
            Ok(())
        }).map_err(storage)
    }
    fn discard(&self, id: &str) -> Result<(), ImportError> {
        self.with(|db| {
            db.execute(
                "DELETE FROM import_contexts WHERE import_id = ?1 AND completed_at IS NULL",
                [id],
            )
            .map(|_| ())
            .map_err(|_| DbError::Storage)
        })
        .map_err(storage)
    }
    fn begin(&self, c: &ImportContext) -> Result<(), ImportError> {
        if c.completed_at.is_some() {
            return Err(ImportError::Storage);
        }
        self.with(|db| {
            db.execute(
                "INSERT INTO import_contexts VALUES (?1, ?2, ?3, ?4, ?5, NULL)",
                (
                    &c.id,
                    c.date.to_string(),
                    &c.zone.name,
                    &c.zone.database_version,
                    i64::try_from(c.config_generation).map_err(|_| DbError::Storage)?,
                ),
            )
            .map(|_| ())
            .map_err(|_| DbError::Storage)
        })
        .map_err(storage)
    }
    fn complete(&self, id: &str, at: i64) -> Result<(), ImportError> {
        self.with(|db| {
            let n = db.execute("UPDATE import_contexts SET completed_at = ?2 WHERE import_id = ?1 AND completed_at IS NULL", (id, at)).map_err(|_| DbError::Storage)?;
            if n == 1 { Ok(()) } else { Err(DbError::Storage) }
        }).map_err(storage)
    }
    fn context(&self, id: &str) -> Result<ImportContext, ImportError> {
        self.with(|db| {
            db.query_row("SELECT diary_date, zone, database_version, config_generation, completed_at FROM import_contexts WHERE import_id = ?1 AND completed_at IS NOT NULL", [id], |r| {
                let date: String = r.get(0)?;
                let date = DiaryDate::parse(&date).ok_or(rusqlite::Error::InvalidQuery)?;
                Ok(ImportContext { id: id.into(), date, zone: ImportZone { name: r.get(1)?, database_version: r.get(2)? }, config_generation: r.get::<_, i64>(3)? as u64, completed_at: r.get(4)? })
            }).optional().map_err(|_| DbError::Storage)
        }).map_err(storage)?.ok_or(ImportError::NotFound)
    }
    fn resolution(
        &self,
        source_id: &str,
        digest: &[u8; 32],
    ) -> Result<Option<Resolution>, ImportError> {
        self.with(|db| db.query_row("SELECT authority, instant_ms, precision, decided_at FROM time_resolutions WHERE source_key = ?1 AND digest = ?2", (source_id, digest.as_slice()), |r| {
            let authority: String = r.get(0)?;
            let precision: String = r.get(2)?;
            Ok(Resolution { authority: if authority == "header" { Authority::Header } else { Authority::Filename }, instant: Instant { unix_ms: r.get(1)?, precision: if precision == "second" { Precision::Second } else { Precision::Milli } }, decided_at: r.get(3)? })
        }).optional().map_err(|_| DbError::Storage)).map_err(storage)
    }
    fn save_resolution(
        &self,
        source_id: &str,
        digest: &[u8; 32],
        choice: &Resolution,
    ) -> Result<(), ImportError> {
        self.with(|db| db.execute("INSERT INTO time_resolutions VALUES (?1, ?2, ?3, ?4, ?5, ?6) ON CONFLICT(source_key, digest) DO UPDATE SET authority=excluded.authority, instant_ms=excluded.instant_ms, precision=excluded.precision, decided_at=excluded.decided_at", (source_id, digest.as_slice(), match choice.authority { Authority::Header => "header", Authority::Filename => "filename" }, choice.instant.unix_ms, match choice.instant.precision { Precision::Second => "second", Precision::Milli => "milli" }, choice.decided_at)).map(|_| ()).map_err(|_| DbError::Storage)).map_err(storage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use diary_logging::private_fs::PrivateDir;
    use std::os::unix::fs::PermissionsExt;
    #[test]
    fn bounds_foreign_keys_and_identity_collisions_are_enforced() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        let dir = PrivateDir::open(&root).unwrap();
        let db = Db::create(&dir, &root.join(crate::sqlite::DB_FILE)).unwrap();
        db.bind_source("source", &[0; 32], "2024/02/29/a.md")
            .unwrap();
        db.bind_source("source", &[0; 32], "2024/02/29/a.md")
            .unwrap();
        assert!(
            db.bind_source("source", &[1; 32], "2024/02/29/a.md")
                .is_err()
        );
        assert!(db.bind_source("source", &[0; 32], "different.md").is_err());
        assert!(
            db.bind_source("too-long", &[0; 32], &"x".repeat(4097))
                .is_err()
        );
        for sql in [
            "INSERT INTO time_resolutions VALUES ('missing', zeroblob(32), 'header', 0, 'second', 0)",
            "INSERT INTO time_resolutions VALUES ('source', zeroblob(31), 'header', 0, 'second', 0)",
            "INSERT INTO time_resolutions VALUES ('source', zeroblob(32), 'guessed', 0, 'second', 0)",
            "INSERT INTO time_resolutions VALUES ('source', zeroblob(32), 'header', 0, 'bad', 0)",
        ] {
            assert!(
                db.with(|c| c.execute(sql, []).map(|_| ()).map_err(|_| DbError::Storage))
                    .is_err()
            );
        }
        let mut context = ImportContext {
            id: "import".into(),
            date: DiaryDate::parse("2024-02-29").unwrap(),
            zone: ImportZone {
                name: "UTC".into(),
                database_version: "x".repeat(33),
            },
            config_generation: 1,
            completed_at: None,
        };
        assert!(db.begin(&context).is_err());
        context.zone.database_version = "2026e".into();
        db.begin(&context).unwrap();
        assert_eq!(db.context("import"), Err(ImportError::NotFound));
        db.complete("import", 99).unwrap();
        context.completed_at = Some(99);
        assert_eq!(db.context("import").unwrap(), context);
    }
}

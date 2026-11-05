//! Durable provenance and timestamp choices, separate from transient source bodies.
use crate::sqlite::{Db, DbError};
use diary_application::imports::{
    Authority, ImportContext, ImportError, ImportListing, ImportRepository, Resolution,
};
use diary_domain::{
    authentic::DiaryDate,
    source::{
        stamp::{Instant, Precision},
        window::ImportZone,
    },
};
use rusqlite::OptionalExtension;

mod snapshot;

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
    fn complete(&self, listing: &ImportListing, at: i64) -> Result<(), ImportError> {
        let payload = snapshot::encode(listing)?;
        self.transact(|tx| {
            tx.execute("INSERT INTO import_snapshots VALUES (?1, ?2)", (&listing.context.id, payload)).map_err(|_| DbError::Storage)?;
            let n = tx.execute("UPDATE import_contexts SET completed_at = ?2 WHERE import_id = ?1 AND completed_at IS NULL", (&listing.context.id, at)).map_err(|_| DbError::Storage)?;
            if n == 1 { Ok(()) } else { Err(DbError::Storage) }
        }).map_err(storage)
    }
    fn snapshot(&self, id: &str) -> Result<ImportListing, ImportError> {
        let context = self.context(id)?;
        let payload: Vec<u8> = self.with(|db| {
            db.query_row("SELECT payload FROM import_snapshots WHERE import_id = ?1 AND length(payload) <= 16777216", [id], |r| r.get(0))
                .optional().map_err(|_| DbError::Storage)
        }).map_err(storage)?.ok_or(ImportError::Stale)?;
        snapshot::decode(context, &payload)
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
    fn listing(context: &ImportContext) -> ImportListing {
        ImportListing {
            navigation: None,
            context: context.clone(),
            included: vec![],
            warnings: vec![],
            misplaced: vec![],
            scan: Default::default(),
        }
    }
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
        let batch = vec![
            ("batch-a".into(), [2; 32], "a.md".into()),
            ("batch-b".into(), [2; 32], "b.md".into()),
        ];
        db.bind_sources(&batch).unwrap();
        db.bind_sources(&batch).unwrap();
        for bad in [
            ("source".into(), [3; 32], "collision.md".into()),
            ("different-id".into(), [0; 32], "2024/02/29/a.md".into()),
            ("long".into(), [2; 32], "x".repeat(4097)),
        ] {
            assert!(
                db.bind_sources(&[("rolled-back".into(), [4; 32], "rollback.md".into()), bad])
                    .is_err()
            );
            assert_eq!(
                db.with(|c| c
                    .query_row(
                        "SELECT count(*) FROM source_identities WHERE source_id='rolled-back'",
                        [],
                        |r| r.get::<_, i64>(0)
                    )
                    .map_err(|_| DbError::Storage))
                    .unwrap(),
                0
            );
        }
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
        db.discard("import").unwrap();
        db.discard("import").unwrap();
        assert_eq!(
            db.complete(&listing(&context), 99),
            Err(ImportError::Storage)
        );
        db.begin(&context).unwrap();
        db.complete(&listing(&context), 99).unwrap();
        db.discard("import").unwrap();
        let persisted = db.snapshot("import").unwrap();
        assert_eq!(persisted.context.completed_at, Some(99));
        assert_eq!(
            db.complete(&listing(&context), 100),
            Err(ImportError::Storage)
        );
        for sql in [
            "UPDATE import_snapshots SET payload=x'00' WHERE import_id='import'",
            "DELETE FROM import_snapshots WHERE import_id='import'",
        ] {
            assert!(
                db.with(|c| c.execute(sql, []).map(|_| ()).map_err(|_| DbError::Storage))
                    .is_err()
            );
        }
        let mut failed = context.clone();
        failed.id = "oversized".into();
        db.begin(&failed).unwrap();
        let mut oversized = listing(&failed);
        oversized.scan.failures.push((
            "x".repeat(16 * 1024 * 1024),
            diary_application::source::SourceOutcome::Unavailable(
                diary_application::source::Unavailable::Missing,
            ),
        ));
        assert_eq!(db.complete(&oversized, 100), Err(ImportError::Limit));
        assert_eq!(db.context("oversized"), Err(ImportError::NotFound));
        db.discard("oversized").unwrap();
        // Simulate an existing released context: publication must roll back the
        // snapshot insert when its completion precondition fails.
        let mut legacy = context.clone();
        legacy.id = "legacy".into();
        db.begin(&legacy).unwrap();
        db.with(|c| {
            c.execute(
                "UPDATE import_contexts SET completed_at=1 WHERE import_id='legacy'",
                [],
            )
            .map(|_| ())
            .map_err(|_| DbError::Storage)
        })
        .unwrap();
        assert!(matches!(db.snapshot("legacy"), Err(ImportError::Stale)));
        assert_eq!(
            db.complete(&listing(&legacy), 100),
            Err(ImportError::Storage)
        );
        assert_eq!(
            db.with(|c| c
                .query_row(
                    "SELECT count(*) FROM import_snapshots WHERE import_id='legacy'",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .map_err(|_| DbError::Storage))
                .unwrap(),
            0
        );
        context.completed_at = Some(99);
        assert_eq!(db.context("import").unwrap(), context);
    }
}

//! Durable, content-free progress rows written before each remote request.
use crate::sqlite::{Db, DbError};
use rusqlite::params;
use std::sync::Arc;

pub struct Steps {
    db: Arc<Db>,
    destination: String,
}

fn sql(_: rusqlite::Error) -> DbError {
    DbError::Storage
}

impl Steps {
    pub fn new(db: Arc<Db>, destination: String) -> Self {
        Self { db, destination }
    }
    /// Drops the previous attempt's rows; recovery never needs them once a new write begins.
    pub fn clear(&self, date: &str) -> Result<(), DbError> {
        self.db.with(|c| {
            c.execute(
                "DELETE FROM notion_steps WHERE destination=?1 AND date=?2",
                params![self.destination, date],
            )
            .map(|_| ())
            .map_err(sql)
        })
    }
    /// Persists the intent; the request is sent only after this succeeds.
    pub fn begin(
        &self,
        date: &str,
        kind: &str,
        parent: &str,
        digest: &str,
    ) -> Result<i64, DbError> {
        self.db.transact(|tx| {
            let seq: i64 = tx
                .query_row(
                    "SELECT COALESCE(MAX(seq),0)+1 FROM notion_steps WHERE destination=?1 AND date=?2",
                    params![self.destination, date],
                    |r| r.get(0),
                )
                .map_err(sql)?;
            tx.execute(
                "INSERT INTO notion_steps VALUES (?1,?2,?3,?4,?5,?6,'pending',NULL)",
                params![self.destination, date, seq, kind, parent, digest],
            )
            .map_err(sql)?;
            Ok(seq)
        })
    }
    pub fn finish(&self, date: &str, seq: i64, ok: bool, ids: Option<&str>) {
        let _ = self.db.with(|c| {
            c.execute(
                "UPDATE notion_steps SET state=?4, remote_ids=?5 WHERE destination=?1 AND date=?2 AND seq=?3",
                params![self.destination, date, seq, if ok { "done" } else { "failed" }, ids],
            )
            .map(|_| ())
            .map_err(sql)
        });
    }
    #[cfg(test)]
    pub fn states(&self, date: &str) -> Vec<(String, String)> {
        self.db
            .with(|c| {
                let mut s = c
                    .prepare("SELECT kind,state FROM notion_steps WHERE destination=?1 AND date=?2 ORDER BY seq")
                    .map_err(sql)?;
                let rows = s
                    .query_map(params![self.destination, date], |r| Ok((r.get(0)?, r.get(1)?)))
                    .map_err(sql)?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(sql)?;
                Ok(rows)
            })
            .unwrap()
    }
}

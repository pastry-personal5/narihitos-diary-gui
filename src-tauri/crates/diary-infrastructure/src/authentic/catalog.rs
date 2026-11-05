//! `catalog_authentic`: rebuildable from the Markdown files, never an authority for receipts.
//! A row whose file does not parse keeps its `problem` text, which blocks saving that date.
use super::{MAX_FILE, revision_of};
use crate::{
    authentic_fs::{self as fs, Observed},
    sqlite::{Db, DbError},
};
use diary_application::authentic::{CatalogRow, RebuildReport, StoreError};
use diary_domain::authentic::{AuthenticRevision, DiaryDate, parse};
use rusqlite::{Connection, OptionalExtension, params};
use std::{collections::HashMap, path::Path};

pub(crate) struct Row {
    pub date: DiaryDate,
    pub id: Option<String>,
    pub revision: Option<AuthenticRevision>,
    pub timezone: Option<String>,
    pub size: u64,
    pub mtime_ns: i64,
    pub inode: String,
    pub problem: Option<String>,
}

impl Row {
    pub(crate) fn into_public(self) -> CatalogRow {
        CatalogRow {
            date: self.date,
            revision: self.revision,
            problem: self.problem,
        }
    }
}

fn storage(_: rusqlite::Error) -> DbError {
    DbError::Storage
}

/// Summarizes one observed file; an unparseable file becomes a problem row.
pub(crate) fn row_from(date: &DiaryDate, seen: &Observed) -> Row {
    let base = |problem: Option<String>| Row {
        date: *date,
        id: None,
        revision: None,
        timezone: None,
        size: seen.bytes.len() as u64,
        mtime_ns: seen.mtime_ns,
        inode: format!("{}:{}", seen.dev, seen.ino),
        problem,
    };
    match parse(&seen.bytes, date) {
        Ok((header, _)) => Row {
            id: Some(header.id.clone()),
            revision: Some(revision_of(&seen.bytes, &header)),
            timezone: Some(header.timezone),
            ..base(None)
        },
        Err(e) => base(Some(e.to_string())),
    }
}

/// Inserts or replaces one row. A second date claiming an already used ID marks both as
/// problems instead of overwriting either.
pub(crate) fn upsert(conn: &Connection, row: &Row) -> Result<(), DbError> {
    let mut row_id = row.id.as_deref();
    let mut problem = row.problem.clone();
    if let Some(id) = row_id {
        let other: Option<String> = conn
            .query_row(
                "SELECT date FROM catalog_authentic WHERE id = ?1 AND date != ?2",
                params![id, row.date.to_string()],
                |r| r.get(0),
            )
            .optional()
            .map_err(storage)?;
        if let Some(other) = other {
            conn.execute(
                "UPDATE catalog_authentic SET id = NULL, problem = 'duplicate id' WHERE date = ?1",
                [other],
            )
            .map_err(storage)?;
            row_id = None;
            problem = Some("duplicate id".into());
        }
    }
    conn.execute(
        "INSERT OR REPLACE INTO catalog_authentic \
         (date, id, revision, sha256, timezone, size, mtime_ns, inode, problem) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            row.date.to_string(),
            row_id,
            row.revision.as_ref().map(|r| r.sequence as i64),
            row.revision.as_ref().map(|r| r.sha256.as_str()),
            row.timezone,
            row.size as i64,
            row.mtime_ns,
            row.inode,
            problem
        ],
    )
    .map_err(storage)?;
    Ok(())
}

pub(crate) fn delete(conn: &Connection, date: &DiaryDate) -> Result<(), DbError> {
    conn.execute(
        "DELETE FROM catalog_authentic WHERE date = ?1",
        [date.to_string()],
    )
    .map_err(storage)?;
    Ok(())
}

/// Keeps the date's row equal to what was just observed.
pub(crate) fn observed(db: &Db, date: &DiaryDate, seen: Option<&Observed>) -> Result<(), DbError> {
    db.transact(|tx| match seen {
        Some(seen) => upsert(tx, &row_from(date, seen)),
        None => delete(tx, date),
    })
}

pub(crate) fn page(
    db: &Db,
    after: Option<DiaryDate>,
    limit: usize,
) -> Result<Vec<CatalogRow>, StoreError> {
    db.with(|c| {
        let mut stmt = c
            .prepare(
                "SELECT date, revision, sha256, problem FROM catalog_authentic \
                 WHERE date > ?1 ORDER BY date LIMIT ?2",
            )
            .map_err(storage)?;
        let after = after.map_or_else(String::new, |d| d.to_string());
        let rows = stmt
            .query_map(params![after, limit as i64], |r| {
                let date: String = r.get(0)?;
                let sequence: Option<i64> = r.get(1)?;
                let sha: Option<String> = r.get(2)?;
                Ok((date, sequence, sha, r.get::<_, Option<String>>(3)?))
            })
            .map_err(storage)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(storage)?;
        Ok(rows)
    })
    .map_err(|_| StoreError::Storage)?
    .into_iter()
    .map(|(date, sequence, sha, problem)| {
        Ok(CatalogRow {
            date: DiaryDate::parse(&date).ok_or(StoreError::Storage)?,
            revision: sequence.zip(sha).map(|(s, sha256)| AuthenticRevision {
                sequence: s as u64,
                sha256,
            }),
            problem,
        })
    })
    .collect()
}

/// Rebuilds the whole catalog from the files under `tree`. Symlinks are never followed; stray
/// names that are not `YYYY/MM/YYYY-MM-DD.md` are ignored. Operations and steps are untouched.
pub(crate) fn rebuild(db: &Db, tree: &Path) -> Result<RebuildReport, StoreError> {
    let rows = scan(tree)?;
    let report = RebuildReport {
        files: rows.len(),
        problems: rows.iter().filter(|r| r.problem.is_some()).count(),
    };
    db.transact(|tx| {
        tx.execute("DELETE FROM catalog_authentic", [])
            .map_err(storage)?;
        rows.iter().try_for_each(|row| upsert(tx, row))
    })
    .map_err(|_| StoreError::Storage)?;
    Ok(report)
}

/// Every dated file under `tree` as a row, duplicates already marked, sorted by date.
pub(crate) fn scan(tree: &Path) -> Result<Vec<Row>, StoreError> {
    let mut rows: Vec<Row> = Vec::new();
    for (year, year_dir) in subdirs(tree, 4)? {
        for (month, month_dir) in subdirs(&year_dir, 2)? {
            let Ok(entries) = std::fs::read_dir(&month_dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                let Some(date) = name
                    .strip_suffix(".md")
                    .and_then(DiaryDate::parse)
                    .filter(|d| {
                        format!("{:04}", d.year()) == year && format!("{:02}", d.month()) == month
                    })
                else {
                    continue;
                };
                rows.push(match fs::read_bounded(&entry.path(), MAX_FILE) {
                    Ok(Some(seen)) => row_from(&date, &seen),
                    Ok(None) => continue,
                    Err(_) => Row {
                        date,
                        id: None,
                        revision: None,
                        timezone: None,
                        size: 0,
                        mtime_ns: 0,
                        inode: String::new(),
                        problem: Some("not a readable regular file within the size limit".into()),
                    },
                });
            }
        }
    }
    // Duplicate IDs: every claimant becomes a problem; none is trusted over another.
    let mut claims: HashMap<String, usize> = HashMap::new();
    for id in rows.iter().filter_map(|r| r.id.clone()) {
        *claims.entry(id).or_default() += 1;
    }
    for row in &mut rows {
        if row.id.as_ref().is_some_and(|id| claims[id] > 1) {
            row.id = None;
            row.revision = None;
            row.problem = Some("duplicate id".into());
        }
    }
    rows.sort_by_key(|r| r.date);
    Ok(rows)
}

/// Directories named with exactly `digits` ASCII digits; symlinks and files are skipped.
pub(crate) fn subdirs(
    parent: &Path,
    digits: usize,
) -> Result<Vec<(String, std::path::PathBuf)>, StoreError> {
    let entries = match std::fs::read_dir(parent) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(_) => return Err(StoreError::Storage),
    };
    let mut found = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.len() == digits
            && name.bytes().all(|b| b.is_ascii_digit())
            && entry.file_type().is_ok_and(|t| t.is_dir())
        {
            found.push((name, entry.path()));
        }
    }
    found.sort();
    Ok(found)
}

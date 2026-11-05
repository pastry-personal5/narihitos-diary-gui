//! Read-only, revalidated timeline of the server and client logs for debugging.
//!
//! Every line is parsed strictly and re-rendered from typed values, so nothing that was not an
//! allowlisted code, tag, number or validated ID can reach the output, however the file was
//! edited. Never reads the discovery file and never takes a lock; the logs stay diagnostic only.

use crate::codec::{Header, MAX_LINE, ParsedLine, parse_line, render_line};
use crate::private_fs::PrivateDir;
use crate::store::parse_name;
use diary_application::diagnostics::{Level, OpaqueId, ResourceKind, Role, TraceId};
use std::collections::BTreeSet;

pub const DEFAULT_LIMIT: usize = 200;
pub const MAX_LIMIT: usize = 500;
pub const MAX_OUTPUT_BYTES: usize = 256 * 1024;
pub const MAX_RECENT: u8 = 50;
/// Per file; the store never writes more than 16 MiB to one.
const MAX_FILE_READ: u64 = 20 * 1024 * 1024;
const MAX_FILES_PER_ROLE: usize = 16;

pub const LEGEND: &str = "ts=local time with offset; lvl=level; role=server|client; code=event (the message); \
trace=request id shared by client and server; res=resource {kind,id,rev=revision,bytes,lines,\
fp=keyed content fingerprint comparable only within one process run,state,path=path class}; \
err=error tag; n=count. Logs hold no content, paths or credentials.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selector {
    Trace(TraceId),
    Operation(OpaqueId),
    Job(OpaqueId),
    Resource(ResourceKind, OpaqueId),
    /// The last n warn/error records, to find a trace ID when none is known.
    Recent(u8),
}

impl Selector {
    pub fn parse(value: &str) -> Option<Self> {
        let (kind, rest) = value.split_once(':')?;
        Some(match kind {
            "trace" => Self::Trace(TraceId::parse(rest)?),
            "op" => Self::Operation(OpaqueId::parse(rest)?),
            "job" => Self::Job(OpaqueId::parse(rest)?),
            "res" => {
                let (kind, id) = rest.split_once(':')?;
                Self::Resource(ResourceKind::parse(kind)?, OpaqueId::parse(id)?)
            }
            "recent" => {
                let n: u8 = rest.parse().ok()?;
                (1..=MAX_RECENT).contains(&n).then_some(Self::Recent(n))?
            }
            _ => return None,
        })
    }

    fn matches(&self, line: &ParsedLine) -> bool {
        let r = &line.record;
        match self {
            Self::Trace(t) => r.trace.as_ref() == Some(t),
            Self::Operation(o) => r.operation.as_ref() == Some(o),
            Self::Job(j) => r.job.as_ref() == Some(j),
            Self::Resource(kind, id) => r
                .resource
                .as_ref()
                .is_some_and(|d| d.kind == *kind && d.id == *id),
            Self::Recent(_) => matches!(line.level, Level::Warn | Level::Error),
        }
    }
}

#[derive(Debug, Default)]
pub struct TraceReport {
    /// Re-rendered log lines (each ends with a newline), oldest first.
    pub lines: Vec<String>,
    /// Distinct `kind:id` resources among the returned records.
    pub resources: Vec<String>,
    pub matched: usize,
    pub invalid: u32,
    pub truncated: bool,
    pub files: u8,
}

impl TraceReport {
    pub fn summary_line(&self) -> String {
        serde_json::json!({"summary": {
            "matched": self.matched,
            "returned": self.lines.len(),
            "truncated": self.truncated,
            "invalid": self.invalid,
            "files": self.files,
            "resources": self.resources,
            "legend": LEGEND,
        }})
        .to_string()
    }
}

pub fn collect(workspace_dir: &PrivateDir, selector: &Selector, limit: usize) -> TraceReport {
    collect_bounded(workspace_dir, selector, limit, MAX_OUTPUT_BYTES)
}

pub fn collect_bounded(
    workspace_dir: &PrivateDir,
    selector: &Selector,
    limit: usize,
    max_bytes: usize,
) -> TraceReport {
    let limit = limit.clamp(1, MAX_LIMIT);
    let mut report = TraceReport::default();
    // (epoch_ms, role order, file order) keeps the merge deterministic.
    let mut matched: Vec<(i64, u8, usize, ParsedLine)> = Vec::new();
    if let Ok(logs) = workspace_dir.open_child("logs") {
        for (order, role) in [Role::Server, Role::Client].into_iter().enumerate() {
            let Ok(dir) = logs.open_child(role.as_str()) else {
                continue;
            };
            let Ok(mut names) = dir.list() else { continue };
            names.retain(|n| parse_name(role, n).is_some());
            let skip = names.len().saturating_sub(MAX_FILES_PER_ROLE);
            for name in &names[skip..] {
                let Ok(bytes) = dir.read_bounded(name, MAX_FILE_READ) else {
                    continue;
                };
                report.files = report.files.saturating_add(1);
                for raw in bytes.split(|b| *b == b'\n').filter(|raw| !raw.is_empty()) {
                    let parsed = std::str::from_utf8(raw)
                        .ok()
                        .filter(|s| s.len() < MAX_LINE)
                        .and_then(parse_line);
                    match parsed {
                        None => report.invalid += 1,
                        Some(line) if selector.matches(&line) => {
                            let seq = matched.len();
                            matched.push((line.epoch_ms, order as u8, seq, line));
                        }
                        Some(_) => {}
                    }
                }
            }
        }
    }
    matched.sort_by_key(|m| (m.0, m.1, m.2));
    report.matched = matched.len();
    let selected: Vec<&ParsedLine> = match selector {
        Selector::Recent(n) => {
            let skip = matched.len().saturating_sub(usize::from(*n));
            matched[skip..].iter().map(|m| &m.3).collect()
        }
        _ => {
            report.truncated = matched.len() > limit;
            matched.iter().take(limit).map(|m| &m.3).collect()
        }
    };
    let (mut bytes, mut resources) = (0usize, BTreeSet::new());
    for line in selected {
        let header = Header {
            ts: &line.ts,
            role: line.role,
            instance: line.instance.as_ref(),
        };
        let Some(rendered) = render_line(&header, &line.record) else {
            report.invalid += 1;
            continue;
        };
        if bytes + rendered.len() > max_bytes {
            report.truncated = true;
            break;
        }
        bytes += rendered.len();
        if let Some(d) = &line.record.resource {
            resources.insert(format!("{}:{}", d.kind.as_str(), d.id.as_str()));
        }
        report.lines.push(rendered);
    }
    report.resources = resources.into_iter().collect();
    report
}

#[cfg(test)]
mod tests;

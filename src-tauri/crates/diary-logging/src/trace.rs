//! Read-only, revalidated timeline of the server and client logs for debugging.
//!
//! Every line is parsed strictly and re-rendered from typed values, so nothing that was not an
//! allowlisted code, tag, number or validated ID can reach the output, however the file was
//! edited. Never reads the discovery file and never takes a lock; the logs stay diagnostic only.

use crate::codec::{Header, MAX_LINE, ParsedLine, parse_line, render_line};
use crate::private_fs::PrivateDir;
use crate::store::parse_name;
use diary_application::diagnostics::{Level, OpaqueId, ResourceKind, Role, TraceId};
use std::collections::{BTreeMap, BTreeSet};

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
err=error tag; fault=connection fault; n=count (milliseconds for agent records); agent={kind,action,phase=last phase reached,code=error code,stop,exit,reason,status,changed,esc=escaped processes,ev=events,in/out=bytes,xn=exit number}. Logs hold no content, paths or credentials.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selector {
    Trace(TraceId),
    Operation(OpaqueId),
    Job(OpaqueId),
    Resource(ResourceKind, OpaqueId),
    /// The last n warn/error records, to find a trace ID when none is known.
    Recent(u8),
    /// The last n records of any level, including debug.
    Debug(u8),
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
            "debug" => {
                let n: u8 = rest.parse().ok()?;
                (1..=MAX_RECENT).contains(&n).then_some(Self::Debug(n))?
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
            Self::Debug(_) => true,
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
    // Only the records that can be returned are kept: the last n for `recent`, otherwise the
    // first `limit` in time order (one extra shows truncation). Keys are (epoch_ms, role
    // order, match order) so the merge is deterministic.
    let keep = match selector {
        Selector::Recent(n) | Selector::Debug(n) => usize::from(*n),
        _ => limit + 1,
    };
    let mut kept: BTreeMap<(i64, u8, usize), ParsedLine> = BTreeMap::new();
    let mut seen = 0usize;
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
                            kept.insert((line.epoch_ms, order as u8, seen), line);
                            seen += 1;
                            if kept.len() > keep {
                                if matches!(selector, Selector::Recent(_) | Selector::Debug(_)) {
                                    kept.pop_first();
                                } else {
                                    kept.pop_last();
                                }
                            }
                        }
                        Some(_) => {}
                    }
                }
            }
        }
    }
    report.matched = seen;
    let selected: Vec<&ParsedLine> = match selector {
        Selector::Recent(_) | Selector::Debug(_) => kept.values().collect(),
        _ => {
            report.truncated = seen > limit;
            kept.values().take(limit).collect()
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

//! Log line codec. Lines are rendered only from typed values and parsed strictly, so a
//! line read back can never carry text that was not an allowlisted code, tag or ID.

use super::clock::LocalStamp;
use diary_application::agent::{AgentErrorCode, AgentKind, Reason};
use diary_application::diagnostics::{
    ActionTag, AgentDetail, Component, ErrorTag, EventCode, ExitTag, Level, OpaqueId, PathClass,
    PhaseTag, Record, ResourceDetail, ResourceKind, Role, StateTag, StatusTag, StopTag, TraceId,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::fmt::Write as _;

/// Longest rendered line, including the newline.
pub const MAX_LINE: usize = 2048;

pub struct Header<'a> {
    pub ts: &'a str,
    pub role: Role,
    pub instance: Option<&'a OpaqueId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedLine {
    pub ts: String,
    pub epoch_ms: i64,
    pub level: Level,
    pub role: Role,
    pub instance: Option<OpaqueId>,
    pub record: Record,
}

/// Drops, never truncates, a line that is too long.
fn fit(line: String) -> Option<String> {
    (line.len() <= MAX_LINE).then_some(line)
}

fn hex8(b: &[u8; 8]) -> String {
    b.iter().fold(String::with_capacity(16), |mut s, x| {
        let _ = write!(s, "{x:02x}");
        s
    })
}

/// Every interpolated value is a validated ID, enum string, number or `ts` checked below,
/// so none can contain a quote, backslash or control character.
pub fn render_line(h: &Header, r: &Record) -> Option<String> {
    LocalStamp::parse_rfc3339(h.ts)?;
    let mut s = String::with_capacity(256);
    let _ = write!(
        s,
        r#"{{"ts":"{}","lvl":"{}","role":"{}""#,
        h.ts,
        r.level.as_str(),
        h.role.as_str()
    );
    if let Some(v) = h.instance {
        let _ = write!(s, r#","inst":"{}""#, v.as_str());
    }
    if let Some(v) = &r.session {
        let _ = write!(s, r#","sess":"{}""#, v.as_str());
    }
    let _ = write!(s, r#","code":"{}""#, r.code.as_str());
    if let Some(v) = r.component {
        let _ = write!(s, r#","comp":"{}""#, v.as_str());
    }
    if let Some(v) = &r.trace {
        let _ = write!(s, r#","trace":"{}""#, v.as_str());
    }
    if let Some(v) = &r.operation {
        let _ = write!(s, r#","op":"{}""#, v.as_str());
    }
    if let Some(v) = &r.job {
        let _ = write!(s, r#","job":"{}""#, v.as_str());
    }
    if let Some(d) = &r.resource {
        let _ = write!(
            s,
            r#","res":{{"kind":"{}","id":"{}""#,
            d.kind.as_str(),
            d.id.as_str()
        );
        if let Some(v) = d.revision {
            let _ = write!(s, r#","rev":{v}"#);
        }
        if let Some(v) = d.bytes {
            let _ = write!(s, r#","bytes":{v}"#);
        }
        if let Some(v) = d.lines {
            let _ = write!(s, r#","lines":{v}"#);
        }
        if let Some(v) = &d.fingerprint {
            let _ = write!(s, r#","fp":"{}""#, hex8(v));
        }
        if let Some(v) = d.state {
            let _ = write!(s, r#","state":"{}""#, v.as_str());
        }
        if let Some(v) = d.path {
            let _ = write!(s, r#","path":"{}""#, v.as_str());
        }
        s.push('}');
    }
    if let Some(v) = r.error {
        let _ = write!(s, r#","err":"{}""#, v.as_str());
    }
    if let Some(v) = r.count {
        let _ = write!(s, r#","n":{v}"#);
    }
    if let Some(a) = &r.agent {
        let _ = write!(s, r#","agent":{{"kind":"{}""#, a.kind.asset_dir());
        let tag = |s: &mut String, k: &str, v: Option<&str>| {
            if let Some(v) = v {
                let _ = write!(s, r#","{k}":"{v}""#);
            }
        };
        tag(&mut s, "action", a.action.map(ActionTag::as_str));
        tag(&mut s, "phase", a.phase.map(PhaseTag::as_str));
        tag(&mut s, "code", a.code.map(AgentErrorCode::as_str));
        tag(&mut s, "stop", a.stop.map(StopTag::as_str));
        tag(&mut s, "exit", a.exit.map(ExitTag::as_str));
        tag(&mut s, "reason", a.reason.map(Reason::as_str));
        tag(&mut s, "status", a.status.map(StatusTag::as_str));
        if let Some(v) = a.changed {
            let _ = write!(s, r#","changed":{v}"#);
        }
        for (k, v) in [
            ("esc", a.escaped),
            ("ev", a.events),
            ("in", a.input_bytes),
            ("out", a.output_bytes),
        ] {
            if let Some(v) = v {
                let _ = write!(s, r#","{k}":{v}"#);
            }
        }
        if let Some(v) = a.exit_number {
            let _ = write!(s, r#","xn":{v}"#);
        }
        s.push('}');
    }
    s.push_str("}\n");
    fit(s)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireLine {
    ts: String,
    lvl: String,
    role: String,
    inst: Option<String>,
    sess: Option<String>,
    code: String,
    comp: Option<String>,
    trace: Option<String>,
    op: Option<String>,
    job: Option<String>,
    res: Option<WireRes>,
    err: Option<String>,
    n: Option<u32>,
    agent: Option<WireAgent>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireAgent {
    kind: String,
    action: Option<String>,
    phase: Option<String>,
    code: Option<String>,
    stop: Option<String>,
    exit: Option<String>,
    reason: Option<String>,
    status: Option<String>,
    changed: Option<bool>,
    esc: Option<u32>,
    ev: Option<u32>,
    #[serde(rename = "in")]
    input: Option<u32>,
    out: Option<u32>,
    xn: Option<i32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireRes {
    kind: String,
    id: String,
    rev: Option<u64>,
    bytes: Option<u32>,
    lines: Option<u32>,
    fp: Option<String>,
    state: Option<String>,
    path: Option<String>,
}

fn id(v: &Option<String>) -> Result<Option<OpaqueId>, ()> {
    v.as_deref()
        .map(|s| OpaqueId::parse(s).ok_or(()))
        .transpose()
}

fn fp(v: &Option<String>) -> Result<Option<[u8; 8]>, ()> {
    let Some(s) = v else { return Ok(None) };
    if s.len() != 16 || !s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
        return Err(());
    }
    let mut out = [0u8; 8];
    for (i, o) in out.iter_mut().enumerate() {
        *o = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).map_err(|_| ())?;
    }
    Ok(Some(out))
}

fn opt<T>(v: &Option<String>, f: fn(&str) -> Option<T>) -> Result<Option<T>, ()> {
    v.as_deref().map(|s| f(s).ok_or(())).transpose()
}

fn convert(w: WireLine) -> Result<ParsedLine, ()> {
    let epoch_ms = LocalStamp::parse_rfc3339(&w.ts).ok_or(())?;
    let level = Level::parse(&w.lvl).ok_or(())?;
    let resource = match &w.res {
        None => None,
        Some(r) => Some(ResourceDetail {
            kind: ResourceKind::parse(&r.kind).ok_or(())?,
            id: OpaqueId::parse(&r.id).ok_or(())?,
            revision: r.rev,
            bytes: r.bytes,
            lines: r.lines,
            fingerprint: fp(&r.fp)?,
            state: opt(&r.state, StateTag::parse)?,
            path: opt(&r.path, PathClass::parse)?,
        }),
    };
    let agent = match &w.agent {
        None => None,
        Some(a) => Some(AgentDetail {
            kind: AgentKind::parse(&a.kind).ok_or(())?,
            action: opt(&a.action, ActionTag::parse)?,
            phase: opt(&a.phase, PhaseTag::parse)?,
            code: opt(&a.code, AgentErrorCode::parse)?,
            stop: opt(&a.stop, StopTag::parse)?,
            exit: opt(&a.exit, ExitTag::parse)?,
            reason: opt(&a.reason, Reason::parse)?,
            status: opt(&a.status, StatusTag::parse)?,
            changed: a.changed,
            escaped: a.esc,
            events: a.ev,
            input_bytes: a.input,
            output_bytes: a.out,
            exit_number: a.xn,
        }),
    };
    let record = Record {
        code: EventCode::parse(&w.code).ok_or(())?,
        level,
        trace: opt(&w.trace, TraceId::parse)?,
        session: id(&w.sess)?,
        component: opt(&w.comp, Component::parse)?,
        operation: id(&w.op)?,
        job: id(&w.job)?,
        resource,
        error: opt(&w.err, ErrorTag::parse)?,
        count: w.n,
        agent,
    };
    Ok(ParsedLine {
        ts: w.ts,
        epoch_ms,
        level,
        role: Role::parse(&w.role).ok_or(())?,
        instance: id(&w.inst)?,
        record,
    })
}

/// Strict inverse of `render_line`: any unknown key, tag, ID or malformed JSON gives `None`.
pub fn parse_line(line: &str) -> Option<ParsedLine> {
    let body = line.strip_suffix('\n').unwrap_or(line);
    if line.len() > MAX_LINE || body.contains('\n') {
        return None;
    }
    convert(serde_json::from_str::<WireLine>(body).ok()?).ok()
}

/// Keyed content fingerprints. The key lives only in memory for one process instance.
pub struct Fingerprinter {
    key: [u8; 16],
}

impl Fingerprinter {
    pub fn new(key: [u8; 16]) -> Self {
        Self { key }
    }

    pub fn describe(
        &self,
        kind: ResourceKind,
        id: OpaqueId,
        revision: Option<u64>,
        content: &[u8],
        state: Option<StateTag>,
        path: Option<PathClass>,
    ) -> ResourceDetail {
        let digest = Sha256::new()
            .chain_update(self.key)
            .chain_update(kind.as_str())
            .chain_update([0u8])
            .chain_update(content)
            .finalize();
        let newlines = content.iter().filter(|&&b| b == b'\n').count();
        let tail = usize::from(!content.is_empty() && content.last() != Some(&b'\n'));
        ResourceDetail {
            kind,
            id,
            revision,
            bytes: Some(u32::try_from(content.len()).unwrap_or(u32::MAX)),
            lines: Some(u32::try_from(newlines + tail).unwrap_or(u32::MAX)),
            fingerprint: Some(digest[..8].try_into().expect("8 bytes")),
            state,
            path,
        }
    }
}

#[cfg(test)]
mod tests;

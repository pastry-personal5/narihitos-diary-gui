//! Apple Foundation Models adapter. The system model is reached only through the `apple-fm` Swift
//! helper process (see `native/apple-fm`), supervised by the shared `Host` like a CLI. The helper is
//! embedded in this binary, extracted to the runtime root with mode 0700 and re-hashed before every
//! launch. Availability is a helper command (no session, no inference) and is rechecked before each
//! run. Native errors arrive as closed codes; no native text reaches argv, logs or `AgentError`.
use super::{
    assets::{PromptAssets, SESSION},
    engine::{Adapter, Collected, Ctx, Invocation, Protocol, Stream, probe_fault},
    host::{Host, RunDir},
    process::{Exit, Outcome},
    profiles::{Entry, ProfileTable},
};
use diary_application::{
    agent::*,
    diagnostics::{AgentDetail, EventCode, Level, Record},
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    ffi::OsString,
    io::Write,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

/// The compiled helper, or empty when `swiftc` or the SDK was missing at build time.
static EMBEDDED: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/apple-fm"));

/// Oldest macOS with the Foundation Models framework.
const MIN_OS_MAJOR: u32 = 26;

pub struct Apple {
    helper: Vec<u8>,
    /// Hash of the file extracted by this process; later launches must still match it.
    extracted: Mutex<Option<(PathBuf, [u8; 32])>>,
    os_major: u32,
}
pub type AppleAdapter = Adapter<Apple>;

impl AppleAdapter {
    pub fn new(table: Arc<ProfileTable>, host: Arc<Host>) -> Self {
        Adapter::with_protocol(table, host, Apple::new(EMBEDDED.to_vec(), os_major()))
    }
    /// For tests: a stand-in helper program and OS version.
    pub fn with_helper(
        table: Arc<ProfileTable>,
        host: Arc<Host>,
        helper: Vec<u8>,
        os_major: u32,
    ) -> Self {
        Adapter::with_protocol(table, host, Apple::new(helper, os_major))
    }
}

impl Apple {
    fn new(helper: Vec<u8>, os_major: u32) -> Self {
        Self {
            helper,
            extracted: Mutex::new(None),
            os_major,
        }
    }

    fn log_helper(host: &Host, reason: Reason) {
        let mut d = AgentDetail::new(AgentKind::AppleFoundationModels);
        d.reason = Some(reason);
        host.emit(Record::new(EventCode::AgentHelper, Level::Error).agent(d));
    }

    /// Extracts once per process (fresh, atomic), then verifies before every launch.
    fn helper_path(&self, host: &Host) -> Result<PathBuf, Reason> {
        if self.helper.is_empty() {
            return Err(Reason::HelperMissing);
        }
        let want: [u8; 32] = Sha256::digest(&self.helper).into();
        let mut state = self.extracted.lock().expect("helper state poisoned");
        let path = match state.as_ref() {
            Some((p, h)) if *h == want => p.clone(),
            _ => {
                let dir = host.runtime_root.join("native");
                std::fs::DirBuilder::new()
                    .recursive(true)
                    .mode(0o700)
                    .create(&dir)
                    .map_err(|_| Reason::HelperFailed)?;
                let hex: String = want[..8].iter().map(|b| format!("{b:02x}")).collect();
                let path = dir.join(format!("apple-fm-{hex}"));
                let tmp = dir.join(format!("apple-fm-{hex}.tmp"));
                let _ = std::fs::remove_file(&tmp);
                let write = || -> std::io::Result<()> {
                    let mut f = std::fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .mode(0o700)
                        .open(&tmp)?;
                    f.write_all(&self.helper)?;
                    f.sync_all()?;
                    std::fs::rename(&tmp, &path)
                };
                write().map_err(|_| Reason::HelperFailed)?;
                *state = Some((path.clone(), want));
                path
            }
        };
        // Verify what is on disk now, not what was written: a swapped or edited file is untrusted.
        let meta = std::fs::symlink_metadata(&path).map_err(|_| Reason::HelperUntrusted)?;
        let intact = meta.is_file()
            && meta.permissions().mode() & 0o022 == 0
            && std::fs::read(&path).is_ok_and(|b| Sha256::digest(&b)[..] == want[..]);
        if !intact {
            Self::log_helper(host, Reason::HelperUntrusted);
            return Err(Reason::HelperUntrusted);
        }
        Ok(path)
    }
}

/// Major macOS version from the kernel; 0 when unknown (treated as unsupported).
fn os_major() -> u32 {
    let mut buf = [0u8; 32];
    let mut len = buf.len();
    // SAFETY: `buf` and `len` describe a valid writable buffer; the name is NUL-terminated.
    let rc = unsafe {
        libc::sysctlbyname(
            c"kern.osproductversion".as_ptr(),
            buf.as_mut_ptr().cast(),
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    if rc != 0 {
        return 0;
    }
    std::str::from_utf8(&buf[..len.min(buf.len())])
        .ok()
        .and_then(|s| s.trim_end_matches('\0').split('.').next()?.parse().ok())
        .unwrap_or(0)
}

/// What `availability` printed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Availability {
    pub available: bool,
    pub reason: Option<Reason>,
    pub os: String,
    pub context: Option<u64>,
    pub languages: Vec<String>,
}

/// Parses the helper's availability line out of probe text (stdout lines plus stderr tail).
pub fn parse_availability(text: &str) -> Result<Availability, Reason> {
    let v: Value = text
        .lines()
        .filter(|l| l.starts_with('{'))
        .find_map(|l| serde_json::from_str(l).ok())
        .ok_or(Reason::HelperFailed)?;
    let reason = match v["state"].as_str() {
        Some("available") => None,
        Some("unavailable") => Some(match v["reason"].as_str() {
            Some("device_ineligible") => Reason::DeviceIneligible,
            Some("apple_intelligence_disabled") => Reason::AppleIntelligenceDisabled,
            Some("model_not_ready") => Reason::ModelNotReady,
            _ => Reason::ModelUnavailableUnknown,
        }),
        _ => return Err(Reason::HelperFailed),
    };
    Ok(Availability {
        available: reason.is_none(),
        reason,
        os: v["os"].as_str().unwrap_or_default().to_owned(),
        context: v["context"].as_u64(),
        languages: v["languages"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|l| l.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default(),
    })
}

/// Closed native error code to the caller's error code and, where one exists, a log reason.
pub fn map_native(code: &str) -> (AgentErrorCode, Option<Reason>) {
    match code {
        "context" => (AgentErrorCode::Context, None),
        "unsupported_language" => (AgentErrorCode::UnsupportedLanguage, None),
        "locale_unsupported" => (
            AgentErrorCode::UnsupportedLanguage,
            Some(Reason::LocaleUnsupported),
        ),
        "refusal" => (AgentErrorCode::Refusal, None),
        "rate_limited" => (AgentErrorCode::RateLimited, None),
        "invalid_output" => (AgentErrorCode::InvalidOutput, None),
        "unavailable" => (AgentErrorCode::Unavailable, Some(Reason::ModelNotReady)),
        "busy" => (AgentErrorCode::Busy, Some(Reason::NativeSessionBusy)),
        "timeout" => (AgentErrorCode::Timeout, None),
        "cancelled" => (AgentErrorCode::Cancelled, None),
        // Anything else, including a code this build does not know, is a failure we log as unmapped.
        _ => (AgentErrorCode::Failed, Some(Reason::NativeErrorUnmapped)),
    }
}

#[derive(Default)]
struct AppleStream {
    started: bool,
    receiving: bool,
    result: Option<String>,
    error: Option<String>,
    bad: bool,
    hint: Option<Reason>,
}
impl Stream for AppleStream {
    fn record(&mut self, rec: &[u8], chat: bool, run: &Run, sink: &mut dyn EventSink) {
        if rec.iter().all(u8::is_ascii_whitespace) {
            return;
        }
        let Ok(v) = serde_json::from_slice::<Value>(rec) else {
            self.bad = true;
            return;
        };
        let mut emit = |e: AgentEvent| {
            if let Some(e) = run.accept(e) {
                sink.event(e);
            }
        };
        if !self.started {
            self.started = true;
            emit(AgentEvent::Phase(Phase::WaitingForOutput));
        }
        match v["type"].as_str() {
            Some("delta") => {
                if let Some(t) = v["text"].as_str().filter(|_| chat) {
                    if !self.receiving {
                        self.receiving = true;
                        emit(AgentEvent::Phase(Phase::ReceivingOutput));
                    }
                    emit(AgentEvent::Delta(t.to_owned()));
                }
            }
            Some("result") => match v["text"].as_str() {
                Some(t) => {
                    if !self.receiving {
                        self.receiving = true;
                        emit(AgentEvent::Phase(Phase::ReceivingOutput));
                    }
                    self.result = Some(t.to_owned());
                }
                None => self.bad = true,
            },
            Some("error") => {
                self.error = v["code"]
                    .as_str()
                    .map(str::to_owned)
                    .or(Some(String::new()))
            }
            _ => self.bad = true,
        }
    }
    fn conclude(&mut self, o: &Outcome, _: &Invocation, _: &Prompt) -> Result<(), AgentError> {
        // The terminal line decides; the exit code is advisory.
        if let Some(code) = &self.error {
            let (c, r) = map_native(code);
            self.hint = r;
            return Err(c.into());
        }
        if self.bad {
            return Err(AgentErrorCode::InvalidOutput.into());
        }
        match &self.result {
            Some(t) if t.len() <= MAX_RESULT_BYTES => Ok(()),
            Some(_) => Err(AgentErrorCode::InvalidOutput.into()),
            None if o.exit != Exit::Success => Err(AgentErrorCode::Failed.into()),
            None => Err(AgentErrorCode::InvalidOutput.into()),
        }
    }
    fn collect(&mut self, _: &Invocation) -> Result<Collected, AgentError> {
        let text = self.result.take().expect("concluded with a result");
        Ok(Collected {
            bytes: text.into_bytes(),
            // Generation uses guided output; the validator still checks every field.
            structured: Some(true),
        })
    }
    fn hint(&self) -> Option<Reason> {
        self.hint
    }
}

use super::assets::Prompt;

impl Protocol for Apple {
    fn kind(&self) -> AgentKind {
        AgentKind::AppleFoundationModels
    }
    fn executable(&self, host: &Host, _: &Entry) -> Result<PathBuf, Reason> {
        if self.os_major < MIN_OS_MAJOR {
            return Err(Reason::OsUnsupported);
        }
        self.helper_path(host)
    }
    fn env(&self, host: &Host, exe: &Path, _: &Path, tmp: &Path) -> Vec<(OsString, OsString)> {
        host.env(exe, tmp, Vec::new())
    }
    fn stdin(
        &self,
        assets: &PromptAssets,
        rendered: String,
        locale: &Locale,
        chat: bool,
    ) -> Result<Vec<u8>, Reason> {
        let session = assets
            .get(AgentKind::AppleFoundationModels, SESSION)
            .map_err(|_| Reason::AssetsInvalid)?;
        serde_json::to_vec(&serde_json::json!({
            "instructions": session.render(locale, None),
            "prompt": rendered,
            "locale": locale.as_str(),
            "mode": if chat { "chat" } else { "generate" },
        }))
        .map_err(|_| Reason::AssetsInvalid)
    }
    /// Availability, never inference: the helper's `availability` command.
    fn version(&self, ctx: &Ctx, cancel: &CancelToken) -> Result<(), Reason> {
        let (o, text) = ctx
            .probe(&["availability"], cancel)
            .ok_or(Reason::SpawnFailed)?;
        if o.exit != Exit::Success || o.stop.is_some() || o.escaped > 0 {
            return Err(probe_fault(&o));
        }
        match parse_availability(&text)? {
            a if a.available => Ok(()),
            a => Err(a.reason.unwrap_or(Reason::ModelUnavailableUnknown)),
        }
    }
    fn invocation(&self, _: &RunDir, _: &Prompt, _: bool) -> Result<Invocation, AgentError> {
        // Fixed argv: no model, effort, path or user text.
        Ok(Invocation {
            argv: vec![OsString::from("run")],
            result_file: None,
        })
    }
    fn stream(&self) -> Box<dyn Stream> {
        Box::new(AppleStream::default())
    }
}

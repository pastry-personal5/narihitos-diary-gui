//! Protocol-independent part of a CLI adapter, shared by Codex and Claude: private run directory,
//! allowlisted environment, stop flags, busy counter, executable resolution, probe runner, shared
//! error classes and the run log record. Protocol modules hold an `Arc<Host>` and add only argv,
//! event parsing and status mapping.
use super::{
    process::{Limits, Outcome, Spec, Supervisor},
    profiles::Entry,
};
use crate::config::select_executable;
use diary_application::{
    agent::*,
    diagnostics::{Diagnostics, ErrorTag, EventCode, Level, Record},
};
use std::{
    ffi::OsString,
    io::Read,
    os::unix::fs::DirBuilderExt,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Instant,
};

/// Strict structured-output schema for schema-1 generation: closed objects, every property required.
pub const GENERATION_SCHEMA: &str = r#"{"type":"object","additionalProperties":false,"required":["draft_markdown","claim_support","warnings"],"properties":{"draft_markdown":{"type":"string"},"claim_support":{"type":"array","items":{"type":"object","additionalProperties":false,"required":["claim","snippet_ids"],"properties":{"claim":{"type":"string"},"snippet_ids":{"type":"array","items":{"type":"string"}}}}},"warnings":{"type":"array","items":{"type":"string"}}}}"#;

/// Fresh owner-private directory under `<runtime_root>/agent-runs`, removed on drop.
pub struct RunDir(PathBuf);
impl RunDir {
    pub fn create(runtime_root: &Path) -> std::io::Result<Self> {
        let parent = runtime_root.join("agent-runs");
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&parent)?;
        let name = crate::lifecycle::random_uuid()
            .map_err(|_| std::io::Error::other("entropy unavailable"))?;
        let dir = parent.join(name);
        std::fs::DirBuilder::new().mode(0o700).create(&dir)?;
        Ok(Self(dir.canonicalize()?))
    }
    pub fn path(&self) -> &Path {
        &self.0
    }
}
impl Drop for RunDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
/// Startup cleanup: nothing is running yet, so every leftover run directory is stale.
pub fn sweep_stale_runs(runtime_root: &Path) {
    let _ = std::fs::remove_dir_all(runtime_root.join("agent-runs"));
}

pub struct StopFlag<'a>(&'a Host, pub Arc<AtomicBool>);
impl Drop for StopFlag<'_> {
    fn drop(&mut self) {
        self.0
            .flags
            .lock()
            .expect("flags poisoned")
            .retain(|f| !Arc::ptr_eq(f, &self.1));
    }
}
pub struct Active<'a>(&'a AtomicUsize);
impl Drop for Active<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

pub fn code_tag(c: AgentErrorCode) -> ErrorTag {
    match c {
        AgentErrorCode::Busy => ErrorTag::Busy,
        AgentErrorCode::Unavailable | AgentErrorCode::AssetsUnavailable => ErrorTag::Unavailable,
        AgentErrorCode::RateLimited | AgentErrorCode::Context | AgentErrorCode::Timeout => {
            ErrorTag::Limit
        }
        AgentErrorCode::InvalidOutput | AgentErrorCode::UnsupportedLanguage => ErrorTag::Invalid,
        _ => ErrorTag::Internal,
    }
}

/// Fixed class patterns recorded with the fixtures. Observed and synthetic groups are named in each
/// protocol's fixture README.
pub fn classify(message: &str) -> AgentErrorCode {
    let m = message.to_ascii_lowercase();
    let has = |needles: &[&str]| needles.iter().any(|n| m.contains(n));
    // Status codes match whole tokens only, so "req_14290" is not a 429.
    let code = |c: &str| {
        m.split(|ch: char| !ch.is_ascii_alphanumeric())
            .any(|t| t == c)
    };
    if has(&["usage limit", "rate limit", "too many requests"]) || code("429") {
        AgentErrorCode::RateLimited
    } else if has(&["context window", "context length", "maximum context"]) {
        AgentErrorCode::Context
    } else if code("401") || has(&["unauthorized", "not logged in", "log in", "sign in"]) {
        AgentErrorCode::Unavailable
    } else {
        AgentErrorCode::Failed
    }
}

pub fn read_bounded(path: &Path) -> Result<Vec<u8>, AgentError> {
    let f = std::fs::File::open(path).map_err(|_| AgentError::from(AgentErrorCode::Failed))?;
    let mut buf = Vec::new();
    f.take(MAX_RESULT_BYTES as u64 + 1)
        .read_to_end(&mut buf)
        .map_err(|_| AgentError::from(AgentErrorCode::Failed))?;
    if buf.len() > MAX_RESULT_BYTES {
        return Err(AgentErrorCode::InvalidOutput.into());
    }
    Ok(buf)
}

pub struct Host {
    pub runtime_root: PathBuf,
    pub launch_path: OsString,
    pub launch_cwd: PathBuf,
    pub home: Option<OsString>,
    lang: Option<OsString>,
    pub limits: Limits,
    pub probe_limits: Limits,
    diagnostics: Arc<dyn Diagnostics>,
    active: AtomicUsize,
    shutdown: Arc<AtomicBool>,
    /// One stop flag per in-flight probe or run, so a lease end can stop them without poisoning later ones.
    flags: Mutex<Vec<Arc<AtomicBool>>>,
}

impl Host {
    pub fn new(
        runtime_root: PathBuf,
        launch_path: OsString,
        launch_cwd: PathBuf,
        diagnostics: Arc<dyn Diagnostics>,
        limits: Limits,
        probe_limits: Limits,
    ) -> Self {
        Self {
            runtime_root,
            launch_path,
            launch_cwd,
            home: std::env::var_os("HOME"),
            lang: std::env::var_os("LANG"),
            limits,
            probe_limits,
            diagnostics,
            active: AtomicUsize::new(0),
            shutdown: Arc::new(AtomicBool::new(false)),
            flags: Mutex::new(Vec::new()),
        }
    }
    /// Server shutdown: in-flight runs and probes stop like a cancel (TERM, grace, KILL, reap) and
    /// new ones are refused. Lease expiry and close reach a run through its job's `Run` cancel.
    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
        self.cancel_all();
    }
    pub fn is_shutdown(&self) -> bool {
        self.shutdown.load(Ordering::SeqCst)
    }
    /// The client lease ended (expiry or close): stop everything in flight; later runs are unaffected.
    pub fn cancel_all(&self) {
        for f in self.flags.lock().expect("flags poisoned").iter() {
            f.store(true, Ordering::SeqCst);
        }
    }
    pub fn stop_flag(&self) -> StopFlag<'_> {
        let f = Arc::new(AtomicBool::new(self.is_shutdown()));
        self.flags.lock().expect("flags poisoned").push(f.clone());
        StopFlag(self, f)
    }
    pub fn is_busy(&self) -> bool {
        self.active.load(Ordering::SeqCst) > 0
    }
    pub fn enter(&self) -> Active<'_> {
        self.active.fetch_add(1, Ordering::SeqCst);
        Active(&self.active)
    }

    /// Allowlisted child environment; `extra` carries the protocol's home variable and flags.
    pub fn env(
        &self,
        exe: &Path,
        tmp: &Path,
        extra: Vec<(OsString, OsString)>,
    ) -> Vec<(OsString, OsString)> {
        // The launch PATH (absolute entries) lets a symlinked shim such as `#!/usr/bin/env node` find
        // its interpreter; `exe` is already canonical, so its own directory alone is not enough.
        let mut dirs: Vec<PathBuf> = exe.parent().map(Path::to_path_buf).into_iter().collect();
        dirs.extend(std::env::split_paths(&self.launch_path).filter(|d| d.is_absolute()));
        dirs.extend(["/usr/bin", "/bin", "/usr/sbin", "/sbin"].map(PathBuf::from));
        let path = std::env::join_paths(dirs).unwrap_or_default();
        let mut env = vec![
            ("PATH".into(), path),
            ("TMPDIR".into(), tmp.as_os_str().to_owned()),
        ];
        env.extend(extra);
        env.extend(self.home.clone().map(|h| ("HOME".into(), h)));
        env.extend(self.lang.clone().map(|l| ("LANG".into(), l)));
        env
    }

    pub fn resolve(&self, e: &Entry, default_name: &str) -> Option<PathBuf> {
        select_executable(
            e.executable.as_deref(),
            default_name,
            &self.launch_path,
            &self.launch_cwd,
        )
    }

    /// Runs a status probe and returns its outcome plus stdout text and stderr tail for matching.
    pub fn probe(
        &self,
        exe: &Path,
        env: Vec<(OsString, OsString)>,
        dir: &RunDir,
        args: &[&str],
        cancel: &CancelToken,
    ) -> Option<(Outcome, String)> {
        let stop = self.stop_flag();
        let spec = Spec {
            executable: exe.to_path_buf(),
            argv: args.iter().map(OsString::from).collect(),
            env,
            stdin: Vec::new(),
            cwd: dir.path().to_path_buf(),
            limits: self.probe_limits,
            shutdown: Some(stop.1.clone()),
        };
        let mut text = String::new();
        let o = Supervisor::run(spec, cancel, &mut |r| {
            text.push_str(&String::from_utf8_lossy(r));
            text.push('\n');
        })
        .ok()?;
        text.push_str(&String::from_utf8_lossy(&o.stderr_tail));
        Some((o, text))
    }

    /// Content-free `agent_run` record: code tag and duration only.
    pub fn log_run<T>(&self, result: &Result<T, AgentError>, started: Instant) {
        let mut rec = Record::new(
            EventCode::AgentRun,
            if result.is_ok() {
                Level::Info
            } else {
                Level::Warn
            },
        )
        .count(u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX));
        if let Err(e) = result {
            rec = rec.error(code_tag(e.0));
        }
        self.diagnostics.emit(rec);
    }
}

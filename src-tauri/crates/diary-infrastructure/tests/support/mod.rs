//! Shared test kit: the one place a `Host` is built for adapter tests.
#![allow(dead_code)]
use diary_application::diagnostics::Diagnostics;
use diary_infrastructure::agent::{host::Host, process::Limits};
use std::{path::Path, sync::Arc, time::Duration};

/// A host rooted in a temp dir: `<root>/runtime` for run dirs, `<root>` as the launch cwd.
pub fn host(root: &Path, diag: Arc<dyn Diagnostics>, limits: Limits) -> Arc<Host> {
    Arc::new(Host::new(
        root.join("runtime"),
        "/usr/bin:/bin".into(),
        root.to_path_buf(),
        diag,
        limits,
        Limits {
            inactivity: Duration::from_secs(5),
            wall: Duration::from_secs(5),
            ..limits
        },
    ))
}

/// Host for env-gated live tests: the real PATH and cwd, production limits.
pub fn live_host(runtime_root: &Path) -> Arc<Host> {
    Arc::new(Host::new(
        runtime_root.to_path_buf(),
        std::env::var_os("PATH").unwrap_or_default(),
        std::env::current_dir().unwrap(),
        Arc::new(diary_application::diagnostics::NullDiagnostics),
        Limits::production(),
        Limits::probe(),
    ))
}

/// Like `host`, but the status probes keep their own limits (a run-limit test must not break them).
pub fn host_with_probe(
    root: &Path,
    diag: Arc<dyn Diagnostics>,
    limits: Limits,
    probe: Limits,
) -> Arc<Host> {
    Arc::new(Host::new(
        root.join("runtime"),
        "/usr/bin:/bin".into(),
        root.to_path_buf(),
        diag,
        limits,
        probe,
    ))
}

#[cfg(feature = "scaffold-smoke")]
mod native_smoke;
use diary_application::{
    diagnostics::{Diagnostics, EventCode, Level, OpaqueId, Record, Role},
    operations::{Class, Readiness},
};
use diary_infrastructure::{
    config,
    lifecycle::{Discovery, LifecycleError, PublishedDiscovery, WorkspaceLock, bind_loopback},
    workspace::Workspace,
};
use diary_logging::{
    clock::SystemClock,
    private_fs::PrivateDir,
    store::LogStore,
    trace::{self, Selector},
    worker::LogWorker,
};
use std::{io::Write, path::PathBuf, sync::Arc, time::Duration};

/// A log directory problem must never stop the server: without a store the worker writes
/// diagnostics to stderr only.
fn start_logging(
    runtime_root: &std::path::Path,
    workspace_key: &str,
    instance: &str,
) -> Arc<LogWorker> {
    let clock = Arc::new(SystemClock);
    let store = PrivateDir::open(runtime_root)
        .and_then(|d| d.child(workspace_key))
        .ok()
        .and_then(|d| LogStore::open(&d, Role::Server, clock.clone()).ok());
    LogWorker::start(
        store,
        Role::Server,
        OpaqueId::parse(instance),
        clock,
        Box::new(std::io::stderr()),
    )
}

#[tokio::main]
async fn main() {
    if let Err(code) = run().await {
        eprintln!("diary-server: {code}");
        std::process::exit(1);
    }
}
/// `diary-server trace --config PATH --select SELECTOR [--limit N]`: a read-only, redacted,
/// bounded timeline of the server and client logs for debugging. It never opens the discovery
/// file or the workspace lock.
fn parse_trace_args(
    mut args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(PathBuf, Selector, usize), &'static str> {
    let (mut config, mut select, mut limit) = (None, None, trace::DEFAULT_LIMIT);
    while let Some(flag) = args.next() {
        let value = args.next().and_then(|v| v.into_string().ok());
        match (flag.to_str(), value) {
            (Some("--config"), Some(v)) => config = Some(PathBuf::from(v)),
            (Some("--select"), Some(v)) => select = Selector::parse(&v),
            (Some("--limit"), Some(v)) => limit = v.parse().map_err(|_| "invalid --limit")?,
            _ => return Err("invalid arguments"),
        }
    }
    Ok((
        config.ok_or("expected --config PATH")?,
        select.ok_or("invalid or missing --select")?,
        limit,
    ))
}
fn trace_mode(args: impl Iterator<Item = std::ffi::OsString>) -> Result<(), &'static str> {
    let (config, selector, limit) = parse_trace_args(args)?;
    let config = config::load(&config).map_err(|_| "configuration rejected")?;
    let key = diary_infrastructure::lifecycle::workspace_key(&config.paths.entries_root);
    let report = PrivateDir::open(&config.paths.runtime_root)
        .and_then(|d| d.open_child(&key))
        .map(|d| trace::collect(&d, &selector, limit))
        .unwrap_or_default();
    // A closed pipe (`| head`) ends the output quietly instead of panicking.
    let mut out = std::io::stdout().lock();
    for line in &report.lines {
        if out.write_all(line.as_bytes()).is_err() {
            return Ok(());
        }
    }
    let _ = writeln!(out, "{}", report.summary_line());
    Ok(())
}
/// Flushes queued diagnostics on every exit path, including early error returns.
struct Drain(Arc<LogWorker>);
impl Drop for Drain {
    fn drop(&mut self) {
        self.0.shutdown(Duration::from_secs(2));
    }
}
async fn run() -> Result<(), &'static str> {
    let mut args = std::env::args_os().skip(1);
    let first = args.next();
    if first.as_deref() == Some(std::ffi::OsStr::new("trace")) {
        return trace_mode(args);
    }
    if first.as_deref() != Some(std::ffi::OsStr::new("--config")) {
        return Err("expected --config PATH");
    }
    let path = PathBuf::from(args.next().ok_or("expected config path")?);
    let development = match args.next() {
        None => false,
        Some(arg)
            if arg == "--development-origin"
                && args.next().as_deref()
                    == Some(std::ffi::OsStr::new("http://localhost:1420")) =>
        {
            true
        }
        _ => return Err("invalid arguments"),
    };
    if args.next().is_some() {
        return Err("invalid arguments");
    }
    let config = config::load(&path).map_err(|_| "configuration rejected")?;
    let lock = WorkspaceLock::acquire(&config.paths.entries_root).map_err(|e| match e {
        LifecycleError::RootNotEmpty => {
            "workspace folder has files but no .diary: inspect it first"
        }
        LifecycleError::WorkspaceBusy => "workspace busy",
        _ => "workspace unavailable",
    })?;
    // Declared after the lock, so the database closes before the lock is released.
    let workspace = Workspace::open(&lock).map_err(|_| "workspace unavailable")?;
    let listener = bind_loopback(config.server.port, config.server.port_fallback_count)
        .map_err(|_| "listener unavailable")?;
    let port = listener
        .local_addr()
        .map_err(|_| "listener unavailable")?
        .port();
    listener
        .set_nonblocking(true)
        .map_err(|_| "listener unavailable")?;
    let listener =
        tokio::net::TcpListener::from_std(listener).map_err(|_| "listener unavailable")?;
    let record = Discovery::fresh(&lock.workspace_key, port).map_err(|_| "entropy unavailable")?;
    let diagnostics = start_logging(
        &config.paths.runtime_root,
        &lock.workspace_key,
        &record.instance_id,
    );
    let _drain = Drain(diagnostics.clone());
    diagnostics.emit(Record::new(EventCode::ServerStarted, Level::Info));
    let (code, level) = match workspace.readiness {
        Readiness::Ready => (EventCode::WorkspaceOpened, Level::Info),
        Readiness::InspectionRequired => (EventCode::WorkspaceInspection, Level::Warn),
        Readiness::Recovery => (EventCode::WorkspaceRecovery, Level::Error),
    };
    diagnostics.emit(Record::new(code, level));
    let blocked = workspace.classify().map_or(0, |c| {
        c.iter()
            .filter(|c| matches!(c.class, Class::Blocked(_)))
            .count()
    });
    if blocked > 0 {
        diagnostics.emit(
            Record::new(EventCode::OperationBlocked, Level::Warn)
                .count(u32::try_from(blocked).unwrap_or(u32::MAX)),
        );
    }
    let authentic: Option<Arc<dyn diary_application::authentic::AuthenticStore>> =
        match workspace.open_authentic() {
            Some(Ok((store, report))) => {
                let n = |v: usize| u32::try_from(v).unwrap_or(u32::MAX);
                if report.reconciled + report.orphans_removed > 0 {
                    diagnostics.emit(
                        Record::new(EventCode::AuthenticRecovered, Level::Info)
                            .count(n(report.reconciled + report.orphans_removed)),
                    );
                }
                diagnostics.emit(
                    Record::new(EventCode::AuthenticRebuilt, Level::Info)
                        .count(n(report.catalog.files)),
                );
                Some(store)
            }
            Some(Err(_)) => {
                diagnostics.emit(Record::new(EventCode::AuthenticBlocked, Level::Error));
                None
            }
            None => workspace
                .read_only_authentic()
                .map(|s| s as Arc<dyn diary_application::authentic::AuthenticStore>),
        };
    diagnostics.emit(Record::new(EventCode::ListenerBound, Level::Info).count(u32::from(port)));
    let mut state = diary_api::ServerState::new(
        record.token.clone(),
        port,
        record.instance_id.clone(),
        record.workspace_key.clone(),
        development,
    );
    diary_api::ServerState::set_diagnostics(&mut state, diagnostics.clone());
    diary_api::ServerState::set_storage_ready(&mut state, workspace.readiness == Readiness::Ready);
    if let Some(store) = workspace.open_imports(config.sources.clone()) {
        diary_api::ServerState::set_imports(&mut state, store);
    }
    let mut source_ready = true;
    if let Some(service) = workspace.source_trash(config.sources.clone()) {
        source_ready = service.recover().is_ok();
        if !source_ready {
            diary_api::ServerState::set_storage_ready(&mut state, false);
            diagnostics.emit(Record::new(EventCode::OperationBlocked, Level::Error));
        }
        diary_api::ServerState::set_source_trash(&mut state, service);
    }
    if source_ready && let Some(Ok(writer)) = workspace.open_source_writer(config.sources.clone()) {
        diary_api::ServerState::set_source_writer(&mut state, writer);
    }
    let watched_store = authentic.clone();
    if let Some(store) = authentic {
        diary_api::ServerState::set_authentic(&mut state, store);
    }
    let launch_path = std::env::var_os("PATH").unwrap_or_default();
    let launch_cwd = std::env::current_dir().map_err(|_| "launch directory unavailable")?;
    diary_api::ServerState::set_startup(
        &mut state,
        config.observe_startup(&launch_path, &launch_cwd),
    );
    // Read-only profile status; nothing here starts a run (P1-M21/P1-M22 own that).
    diary_infrastructure::agent::host::sweep_stale_runs(&config.paths.runtime_root);
    let profile_table =
        Arc::new(diary_infrastructure::agent::profiles::ProfileTable::from_config(&config));
    let adapter = Arc::new(diary_infrastructure::agent::codex::CodexAdapter::new(
        profile_table.clone(),
        config.paths.runtime_root.clone(),
        launch_path.clone(),
        launch_cwd.clone(),
        diagnostics.clone(),
    ));
    let claude = Arc::new(diary_infrastructure::agent::claude::ClaudeAdapter::new(
        profile_table.clone(),
        adapter.host(),
    ));
    let qwen = Arc::new(diary_infrastructure::agent::qwen::QwenAdapter::new(
        profile_table.clone(),
        adapter.host(),
    ));
    {
        // One shared host: a single cancel/shutdown covers Codex, Claude and Qwen runs.
        let adapter = adapter.clone();
        diary_api::ServerState::set_lease_ended(&mut state, Arc::new(move || adapter.cancel_all()));
    }
    diary_api::ServerState::set_profiles(
        &mut state,
        Arc::new(diary_infrastructure::agent::profiles::ProfileService::new(
            profile_table.clone(),
            adapter.clone(),
            claude,
            qwen,
        )),
    );
    diary_api::ServerState::set_reloader(
        &mut state,
        Arc::new(
            config::ConfigHandle::new(&path, &config, launch_path.clone(), launch_cwd.clone())
                .with_profiles(profile_table),
        ),
    );
    #[cfg(feature = "scaffold-smoke")]
    diary_api::ServerState::set_smoke_folders(&mut state, native_smoke::folders(&config));
    // shortcut: a tree that does not exist yet is not watched until the next start.
    let tree = config.paths.entries_root.join("assets/diary/authentic");
    let _watch = (workspace.readiness == Readiness::Ready && tree.is_dir())
        .then(|| diary_infrastructure::watch::watch(&tree, diary_infrastructure::watch::DEBOUNCE))
        .and_then(Result::ok)
        .map(|w| {
            let state = state.clone();
            std::thread::spawn(move || {
                // Moving the whole watcher in (not just `signals`) keeps it alive for the process.
                let w = w;
                while w.signals.recv().is_ok() {
                    // The date list reads the catalog, so bring it in line with the disk first.
                    if let Some(store) = &watched_store {
                        let _ = store.refresh();
                    }
                    state.authentic_changed();
                }
            });
        });
    let app = diary_api::router(state.clone());
    let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .map_err(|_| "signal registration failed")?;
    let mut interrupt = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
        .map_err(|_| "signal registration failed")?;
    let published = PublishedDiscovery::publish(&config.paths.runtime_root, &record, &lock)
        .map_err(|_| "discovery unavailable")?;
    diagnostics.emit(Record::new(EventCode::DiscoveryPublished, Level::Info));
    println!("Discovery: {}", published.path.display());
    let sweeper = tokio::spawn(state.clone().sweep());
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
    let service = axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(async move {
        let _ = stop_rx.await;
    });
    let mut service = std::pin::pin!(std::future::IntoFuture::into_future(service));
    tokio::select! {
        result = &mut service => { result.map_err(|_| "server transport failed")?; },
        _ = async { tokio::select! { _ = interrupt.recv() => (), _ = term.recv() => () } } => {
            diagnostics.emit(Record::new(EventCode::ServerStopping, Level::Info));
            adapter.shutdown();
            state.shutdown();
            let _ = stop_tx.send(());
            // No durable work is implemented; stop streams and cap network drain.
            if let Ok(result) = tokio::time::timeout(std::time::Duration::from_secs(10), &mut service).await {
                result.map_err(|_| "server transport failed")?;
            }
        },
    }
    sweeper.abort();
    drop(published);
    drop(workspace);
    drop(lock);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args(v: &[&str]) -> impl Iterator<Item = std::ffi::OsString> {
        v.iter()
            .map(std::ffi::OsString::from)
            .collect::<Vec<_>>()
            .into_iter()
    }
    #[test]
    fn trace_args_accept_config_select_and_limit() {
        let (config, selector, limit) = parse_trace_args(args(&[
            "--config", "/x.yaml", "--select", "recent:5", "--limit", "7",
        ]))
        .unwrap();
        assert_eq!(config, PathBuf::from("/x.yaml"));
        assert_eq!(selector, Selector::Recent(5));
        assert_eq!(limit, 7);
        let (_, _, default) =
            parse_trace_args(args(&["--config", "/x.yaml", "--select", "recent:1"])).unwrap();
        assert_eq!(default, trace::DEFAULT_LIMIT);
    }
    #[test]
    fn trace_args_reject_missing_unknown_and_malformed_input() {
        for bad in [
            &["--select", "recent:5"][..],
            &["--config", "/x.yaml"],
            &["--config", "/x.yaml", "--select", "bogus"],
            &[
                "--config", "/x.yaml", "--select", "recent:5", "--limit", "many",
            ],
            &[
                "--config",
                "/x.yaml",
                "--select",
                "recent:5",
                "--verbose",
                "1",
            ],
            &["--config", "/x.yaml", "--select"],
        ] {
            assert!(parse_trace_args(args(bad)).is_err(), "{bad:?}");
        }
    }
}

#[cfg(feature = "scaffold-smoke")]
mod native_smoke;
use diary_application::diagnostics::{Diagnostics, EventCode, Level, OpaqueId, Record, Role};
use diary_infrastructure::{
    config,
    lifecycle::{Discovery, PublishedDiscovery, WorkspaceLock, bind_loopback},
};
use diary_logging::{
    clock::SystemClock, private_fs::PrivateDir, store::LogStore, worker::LogWorker,
};
use std::{path::PathBuf, sync::Arc, time::Duration};

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
async fn run() -> Result<(), &'static str> {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() != Some(std::ffi::OsStr::new("--config")) {
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
    let lock = WorkspaceLock::acquire(&config.paths.entries_root)
        .map_err(|_| "workspace unavailable or busy")?;
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
    diagnostics.emit(Record::new(EventCode::ServerStarted, Level::Info));
    diagnostics.emit(Record::new(EventCode::ListenerBound, Level::Info).count(u32::from(port)));
    let mut state = diary_api::ServerState::new(
        record.token.clone(),
        port,
        record.instance_id.clone(),
        record.workspace_key.clone(),
        development,
    );
    diary_api::ServerState::set_diagnostics(&mut state, diagnostics.clone());
    let launch_path = std::env::var_os("PATH").unwrap_or_default();
    let launch_cwd = std::env::current_dir().map_err(|_| "launch directory unavailable")?;
    diary_api::ServerState::set_startup(
        &mut state,
        config.observe_startup(&launch_path, &launch_cwd),
    );
    #[cfg(feature = "scaffold-smoke")]
    diary_api::ServerState::set_smoke_folders(&mut state, native_smoke::folders(&config));
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
            state.shutdown();
            let _ = stop_tx.send(());
            // No durable work is implemented; stop streams and cap network drain.
            if let Ok(result) = tokio::time::timeout(std::time::Duration::from_secs(10), &mut service).await {
                result.map_err(|_| "server transport failed")?;
            }
        },
    }
    sweeper.abort();
    diagnostics.shutdown(Duration::from_secs(2));
    drop(published);
    drop(lock);
    Ok(())
}

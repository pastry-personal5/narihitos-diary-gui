//! Native watching as an invalidation hint. FSEvents may coalesce or drop events, so the
//! signal carries no path and no content: a receiver always rescans.
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::{
    path::Path,
    sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel},
    time::Duration,
};

pub const DEBOUNCE: Duration = Duration::from_millis(250);

pub struct AuthenticWatch {
    _watcher: RecommendedWatcher,
    pub signals: Receiver<()>,
}

/// Watches `tree` recursively. Events (and watcher errors, which may mean overflow) within
/// `debounce` of each other become one `()` signal on `signals`.
pub fn watch(tree: &Path, debounce: Duration) -> notify::Result<AuthenticWatch> {
    let (raw_tx, raw_rx) = channel::<()>();
    let (out_tx, signals) = channel::<()>();
    let mut watcher = RecommendedWatcher::new(
        move |event: notify::Result<Event>| {
            let relevant = invalidates(&event);
            if relevant {
                let _ = raw_tx.send(());
            }
        },
        Config::default(),
    )?;
    watcher.watch(tree, RecursiveMode::Recursive)?;
    std::thread::spawn(move || debounce_loop(raw_rx, out_tx, debounce));
    Ok(AuthenticWatch {
        _watcher: watcher,
        signals,
    })
}

/// Reads and metadata-only access do not change content; an error may mean lost events (an
/// overflow), so it forces a rescan like any change.
fn invalidates(event: &notify::Result<Event>) -> bool {
    match event {
        Ok(e) => !matches!(e.kind, EventKind::Access(_)),
        Err(_) => true,
    }
}

fn debounce_loop(raw: Receiver<()>, out: Sender<()>, debounce: Duration) {
    while raw.recv().is_ok() {
        loop {
            match raw.recv_timeout(debounce) {
                Ok(()) => {}
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => {
                    let _ = out.send(());
                    return;
                }
            }
        }
        if out.send(()).is_err() {
            return;
        }
    }
}

#[cfg(test)]
mod tests;

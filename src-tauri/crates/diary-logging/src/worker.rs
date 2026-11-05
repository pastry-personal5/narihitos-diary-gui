//! Bounded asynchronous log writer.
//!
//! Producers render a line on their own thread and `try_send` it; one worker thread owns every
//! disk and stderr operation. A full queue, a stalled disk or a stalled stderr can only drop
//! diagnostics, never delay a caller.

use crate::LogError;
use crate::clock::Clock;
use crate::codec::{Header, render_line};
use crate::store::LogStore;
use diary_application::diagnostics::{Diagnostics, EventCode, Level, OpaqueId, Record, Role};
use std::{
    collections::HashMap,
    io::Write,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
        mpsc::{Receiver, RecvTimeoutError, SyncSender, channel, sync_channel},
    },
    time::{Duration, Instant},
};

/// The only text ever written to stderr on a logging failure: no path, no error detail.
pub const NOTICE: &str = "diary-log: write failed\n";
pub const QUEUE_RECORDS: usize = 256;

#[derive(Debug, Clone, Copy)]
pub struct WorkerConfig {
    pub queue: usize,
    /// How often an idle worker wakes to check the injected clock.
    pub tick: Duration,
    pub prune_every_secs: i64,
    pub notice_every_secs: i64,
    /// Non-error records accepted per event code per minute; the excess is counted in
    /// `log.dropped`, so one noisy code cannot fill the 8-file cap and silence the rest.
    pub per_code_per_minute: u32,
}

impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            queue: QUEUE_RECORDS,
            tick: Duration::from_secs(1),
            prune_every_secs: 3600,
            notice_every_secs: 60,
            per_code_per_minute: 60,
        }
    }
}

pub struct LogWorker {
    tx: SyncSender<String>,
    role: Role,
    instance: Option<OpaqueId>,
    clock: Arc<dyn Clock>,
    dropped: Arc<AtomicU32>,
    processed: Arc<AtomicU64>,
    stopping: Arc<AtomicBool>,
    drain_ms: Arc<AtomicU64>,
    done: Mutex<Option<Receiver<()>>>,
    limit: u32,
    rates: Mutex<HashMap<&'static str, (i64, u32)>>,
}

struct Core {
    store: Option<LogStore>,
    stderr: Box<dyn Write + Send>,
    clock: Arc<dyn Clock>,
    role: Role,
    instance: Option<OpaqueId>,
    last_notice: Option<i64>,
    notice_every: i64,
    dropped: Arc<AtomicU32>,
    processed: Arc<AtomicU64>,
}

impl Core {
    fn put(&mut self, line: &str) -> Result<(), LogError> {
        match &mut self.store {
            Some(store) => store.append(line),
            None => self
                .stderr
                .write_all(line.as_bytes())
                .map_err(|_| LogError::Unavailable),
        }
    }

    fn write(&mut self, line: &str) {
        match self.put(line) {
            Ok(()) => {}
            Err(LogError::Capped | LogError::Oversize) => {
                self.dropped.fetch_add(1, Ordering::Relaxed);
            }
            Err(_) => self.notice(),
        }
        self.processed.fetch_add(1, Ordering::Relaxed);
    }

    fn notice(&mut self) {
        let now = self.clock.now().epoch_secs;
        if self
            .last_notice
            .is_none_or(|t| now - t >= self.notice_every)
        {
            self.last_notice = Some(now);
            let _ = self.stderr.write_all(NOTICE.as_bytes());
        }
    }

    /// Reports lines dropped by a full queue or a capped directory; keeps the count if it
    /// cannot be written yet.
    fn flush_dropped(&mut self) {
        let n = self.dropped.swap(0, Ordering::Relaxed);
        if n == 0 {
            return;
        }
        let ts = self.clock.now().rfc3339();
        let header = Header {
            ts: &ts,
            role: self.role,
            instance: self.instance.as_ref(),
        };
        let record = Record::new(EventCode::LogDropped, Level::Warn).count(n);
        let written = render_line(&header, &record).is_some_and(|l| self.put(&l).is_ok());
        if !written {
            self.dropped.fetch_add(n, Ordering::Relaxed);
        }
    }
}

impl LogWorker {
    pub fn start(
        store: Option<LogStore>,
        role: Role,
        instance: Option<OpaqueId>,
        clock: Arc<dyn Clock>,
        stderr: Box<dyn Write + Send>,
    ) -> Arc<Self> {
        Self::start_with(
            store,
            role,
            instance,
            clock,
            stderr,
            WorkerConfig::default(),
        )
    }

    pub fn start_with(
        store: Option<LogStore>,
        role: Role,
        instance: Option<OpaqueId>,
        clock: Arc<dyn Clock>,
        stderr: Box<dyn Write + Send>,
        config: WorkerConfig,
    ) -> Arc<Self> {
        let (tx, rx) = sync_channel::<String>(config.queue);
        let (done_tx, done_rx) = channel();
        let worker = Arc::new(Self {
            tx,
            role,
            instance: instance.clone(),
            clock: clock.clone(),
            dropped: Arc::default(),
            processed: Arc::default(),
            stopping: Arc::default(),
            drain_ms: Arc::default(),
            done: Mutex::new(Some(done_rx)),
            limit: config.per_code_per_minute,
            rates: Mutex::default(),
        });
        let mut core = Core {
            store,
            stderr,
            clock,
            role,
            instance,
            last_notice: None,
            notice_every: config.notice_every_secs,
            dropped: worker.dropped.clone(),
            processed: worker.processed.clone(),
        };
        let (stopping, drain_ms) = (worker.stopping.clone(), worker.drain_ms.clone());
        let spawned = std::thread::Builder::new()
            .name("diary-log".into())
            .spawn(move || {
                run(&mut core, &rx, config, &stopping, &drain_ms);
                let _ = done_tx.send(());
            });
        if spawned.is_err() {
            // No thread: every emit will see a full channel and count a drop.
            worker.stopping.store(true, Ordering::Relaxed);
        }
        worker
    }

    fn allow(&self, code: &'static str, epoch_secs: i64) -> bool {
        let minute = epoch_secs.div_euclid(60);
        let mut rates = self.rates.lock().unwrap_or_else(|e| e.into_inner());
        let slot = rates.entry(code).or_insert((minute, 0));
        if slot.0 != minute {
            *slot = (minute, 0);
        }
        if slot.1 >= self.limit {
            return false;
        }
        slot.1 += 1;
        true
    }

    /// Queues an already rendered line (for example one buffered before the log was open).
    pub fn emit_line(&self, line: String) {
        if self.tx.try_send(line).is_err() {
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Lines the worker has taken from the queue, written or failed.
    pub fn processed(&self) -> u64 {
        self.processed.load(Ordering::Relaxed)
    }

    /// Best-effort drain for at most `drain`, then abandon whatever is queued or blocked.
    pub fn shutdown(&self, drain: Duration) {
        self.drain_ms.store(
            drain.as_millis().min(u64::MAX as u128) as u64,
            Ordering::Relaxed,
        );
        self.stopping.store(true, Ordering::Relaxed);
        // Wake an idle worker now instead of after its tick; an empty line is never written.
        let _ = self.tx.try_send(String::new());
        let done = self.done.lock().ok().and_then(|mut d| d.take());
        if let Some(done) = done {
            let _ = done.recv_timeout(drain + Duration::from_millis(50));
        }
    }
}

fn run(
    core: &mut Core,
    rx: &Receiver<String>,
    config: WorkerConfig,
    stopping: &AtomicBool,
    drain_ms: &AtomicU64,
) {
    let mut next_prune = core.clock.now().epoch_secs + config.prune_every_secs;
    loop {
        match rx.recv_timeout(config.tick) {
            Ok(line) if line.is_empty() => {}
            Ok(line) => core.write(&line),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        core.flush_dropped();
        let now = core.clock.now().epoch_secs;
        if now >= next_prune {
            if let Some(store) = core.store.as_mut() {
                let _ = store.prune();
            }
            next_prune = now + config.prune_every_secs;
        }
        if stopping.load(Ordering::Relaxed) {
            let deadline = Instant::now() + Duration::from_millis(drain_ms.load(Ordering::Relaxed));
            while Instant::now() < deadline {
                match rx.try_recv() {
                    Ok(line) if line.is_empty() => {}
                    Ok(line) => core.write(&line),
                    Err(_) => break,
                }
            }
            core.flush_dropped();
            break;
        }
    }
}

impl Diagnostics for LogWorker {
    fn emit(&self, record: Record) {
        let now = self.clock.now();
        if record.level != Level::Error && !self.allow(record.code.as_str(), now.epoch_secs) {
            self.dropped.fetch_add(1, Ordering::Relaxed);
            return;
        }
        let ts = now.rfc3339();
        let header = Header {
            ts: &ts,
            role: self.role,
            instance: self.instance.as_ref(),
        };
        let Some(line) = render_line(&header, &record) else {
            self.dropped.fetch_add(1, Ordering::Relaxed);
            return;
        };
        if self.tx.try_send(line).is_err() {
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod tests;

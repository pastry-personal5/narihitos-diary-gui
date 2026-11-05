//! Client diagnostics: the typed `log_client_event` boundary and a log that buffers (at most
//! 256 lines, keeping the earliest) until the discovery directory is validated, then writes the
//! client log; if that log cannot be opened it stays on stderr. No WebView string is logged:
//! every field is an allowlisted code, tag, number or validated ID.

use diary_application::diagnostics::{
    Component, ErrorTag, EventCode, Level, Record, Role, TraceId,
};
use diary_logging::{
    clock::{Clock, SystemClock},
    codec::{Header, render_line},
    private_fs::PrivateDir,
    store::LogStore,
    worker::{LogWorker, WorkerConfig},
};
use serde::Deserialize;
use std::{
    io::Write,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

const BUFFER_LINES: usize = 256;
const MAX_DETAIL_BYTES: usize = 512;
const EVENTS_PER_SECOND: u32 = 20;

type MakeStderr = Arc<dyn Fn() -> Box<dyn Write + Send> + Send + Sync>;
type Millis = Arc<dyn Fn() -> u64 + Send + Sync>;

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Buffering,
    File,
    Stderr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reject {
    Invalid,
    Rate,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClientEvent {
    code: String,
    level: String,
    trace_id: Option<String>,
    detail: Option<serde_json::Value>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Detail {
    component: String,
    count: Option<u32>,
    error: Option<String>,
}

fn build(event: &ClientEvent) -> Result<Record, ()> {
    let code = EventCode::parse(&event.code)
        .filter(|c| c.webview_allowed())
        .ok_or(())?;
    let mut record = Record::new(code, Level::parse(&event.level).ok_or(())?);
    if let Some(trace) = &event.trace_id {
        record = record.trace(TraceId::parse(trace).ok_or(())?);
    }
    if let Some(value) = &event.detail {
        if serde_json::to_vec(value).map_err(|_| ())?.len() > MAX_DETAIL_BYTES {
            return Err(());
        }
        let detail: Detail = serde_json::from_value(value.clone()).map_err(|_| ())?;
        record = record.component(Component::parse(&detail.component).ok_or(())?);
        if let Some(count) = detail.count {
            record = record.count(count);
        }
        if let Some(error) = &detail.error {
            record = record.error(ErrorTag::parse(error).ok_or(())?);
        }
    }
    Ok(record)
}

enum State {
    Buffering { lines: Vec<String>, dropped: u32 },
    File(Arc<LogWorker>),
    Stderr,
}

struct Limiter {
    window_start: u64,
    count: u32,
}
impl Limiter {
    /// Fixed one-second window of at most 20 events.
    fn allow(&mut self, now_ms: u64) -> bool {
        if now_ms.saturating_sub(self.window_start) >= 1000 {
            self.window_start = now_ms;
            self.count = 0;
        }
        if self.count >= EVENTS_PER_SECOND {
            return false;
        }
        self.count += 1;
        true
    }
}

pub struct ClientLog {
    clock: Arc<dyn Clock>,
    now_ms: Millis,
    make_stderr: MakeStderr,
    stderr_worker: Arc<LogWorker>,
    state: Mutex<State>,
    limiter: Mutex<Limiter>,
}

impl ClientLog {
    pub fn new() -> Arc<Self> {
        let start = Instant::now();
        Self::with(
            Arc::new(SystemClock),
            Arc::new(move || start.elapsed().as_millis() as u64),
            Arc::new(|| Box::new(std::io::stderr())),
        )
    }

    pub(crate) fn with(
        clock: Arc<dyn Clock>,
        now_ms: Millis,
        make_stderr: MakeStderr,
    ) -> Arc<Self> {
        let stderr_worker =
            LogWorker::start(None, Role::Client, None, clock.clone(), make_stderr());
        Arc::new(Self {
            clock,
            now_ms,
            make_stderr,
            stderr_worker,
            state: Mutex::new(State::Buffering {
                lines: Vec::new(),
                dropped: 0,
            }),
            limiter: Mutex::new(Limiter {
                window_start: 0,
                count: 0,
            }),
        })
    }

    pub fn log(&self, record: Record) {
        let ts = self.clock.now().rfc3339();
        let header = Header {
            ts: &ts,
            role: Role::Client,
            instance: None,
        };
        let Some(line) = render_line(&header, &record) else {
            return;
        };
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        match &mut *state {
            State::Buffering { lines, dropped } => {
                if lines.len() < BUFFER_LINES {
                    lines.push(line.clone());
                } else {
                    *dropped += 1;
                }
                self.stderr_worker.emit_line(line);
            }
            State::File(worker) => worker.emit_line(line),
            State::Stderr => self.stderr_worker.emit_line(line),
        }
    }

    /// Opens the client log once the discovery directory is validated. A lock held by another
    /// client process, or any open failure, leaves the client on stderr; it never competes
    /// for the file and never searches for another location.
    pub fn attach(&self, workspace_dir: &PrivateDir) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let (lines, dropped) = match std::mem::replace(&mut *state, State::Stderr) {
            State::Buffering { lines, dropped } => (lines, dropped),
            other => {
                *state = other;
                return;
            }
        };
        let Ok(store) = LogStore::open(workspace_dir, Role::Client, self.clock.clone()) else {
            return;
        };
        let worker = LogWorker::start_with(
            Some(store),
            Role::Client,
            None,
            self.clock.clone(),
            (self.make_stderr)(),
            // Room for a full 256-line buffer flush plus the drop report.
            WorkerConfig {
                queue: BUFFER_LINES * 2,
                ..WorkerConfig::default()
            },
        );
        for line in lines {
            worker.emit_line(line);
        }
        if dropped > 0 {
            use diary_application::diagnostics::Diagnostics;
            worker.emit(Record::new(EventCode::LogDropped, Level::Warn).count(dropped));
        }
        *state = State::File(worker);
    }

    pub fn client_event(&self, event: ClientEvent) -> Result<(), Reject> {
        let allowed = self
            .limiter
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .allow((self.now_ms)());
        if !allowed {
            return Err(Reject::Rate);
        }
        match build(&event) {
            Ok(record) => {
                self.log(record);
                Ok(())
            }
            Err(()) => {
                self.log(
                    Record::new(EventCode::ClientEventRejected, Level::Warn)
                        .error(ErrorTag::Invalid),
                );
                Err(Reject::Invalid)
            }
        }
    }

    /// Best-effort drain on exit.
    pub fn shutdown(&self, drain: Duration) {
        let worker = match &*self.state.lock().unwrap_or_else(|e| e.into_inner()) {
            State::File(worker) => Some(worker.clone()),
            _ => None,
        };
        if let Some(worker) = worker {
            worker.shutdown(drain);
        }
        self.stderr_worker.shutdown(drain);
    }

    #[cfg(test)]
    pub fn mode(&self) -> Mode {
        match &*self.state.lock().unwrap() {
            State::Buffering { .. } => Mode::Buffering,
            State::File(_) => Mode::File,
            State::Stderr => Mode::Stderr,
        }
    }

    #[cfg(test)]
    pub fn buffered(&self) -> usize {
        match &*self.state.lock().unwrap() {
            State::Buffering { lines, .. } => lines.len(),
            _ => 0,
        }
    }
}

#[cfg(test)]
mod tests;

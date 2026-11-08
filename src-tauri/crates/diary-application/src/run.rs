//! Execution of one agent run into an observable, bounded snapshot.
//!
//! The pump turns adapter events into a stage, batched provisional text and change
//! notifications. Provisional text is never an `AgentOutput`; clients read it by query and
//! rebuild it after a gap, so the event stream carries no text.
use crate::agent::{
    AgentError, AgentEvent, AgentOutput, AgentRunner, EventSink, MAX_TEXT_BYTES, Phase,
    PreparedAgentRun, Run,
};
use std::sync::{Arc, Mutex};

pub const FLUSH_MS: u64 = 50;
pub const FLUSH_BYTES: usize = 16 * 1024;

/// What a client may observe of a running job. `revision` grows with every visible change.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Snapshot {
    pub phase: Option<Phase>,
    pub text: String,
    pub revision: u64,
    pub last_activity_at: Option<u64>,
}
pub type Shared = Arc<Mutex<Snapshot>>;

struct Pump<'a> {
    run: &'a Run,
    shared: &'a Shared,
    clock: &'a dyn Fn() -> u64,
    notify: &'a mut dyn FnMut(),
    pending: String,
    last_flush: u64,
}
impl Pump<'_> {
    fn flush(&mut self) {
        if self.run.token().is_cancelled() {
            self.pending.clear();
            return;
        }
        if self.pending.is_empty() {
            return;
        }
        let mut s = self.shared.lock().expect("snapshot poisoned");
        // shortcut: text past the reply bound is dropped here; the validator rejects such a
        // reply anyway, upgrade if a streaming consumer needs a truncation marker.
        let room = MAX_TEXT_BYTES.saturating_sub(s.text.len());
        let mut take = self.pending.len().min(room);
        while !self.pending.is_char_boundary(take) {
            take -= 1;
        }
        s.text.push_str(&self.pending[..take]);
        s.revision += 1;
        drop(s);
        self.pending.clear();
        self.last_flush = (self.clock)();
        (self.notify)();
    }
}
impl EventSink for Pump<'_> {
    fn event(&mut self, event: AgentEvent) {
        // After a committed cancel nothing more becomes visible.
        let Some(event) = self.run.accept(event) else {
            return;
        };
        self.shared
            .lock()
            .expect("snapshot poisoned")
            .last_activity_at = Some((self.clock)());
        match event {
            AgentEvent::Phase(p) => {
                self.flush();
                let mut s = self.shared.lock().expect("snapshot poisoned");
                s.phase = Some(p);
                s.revision += 1;
                drop(s);
                (self.notify)();
            }
            AgentEvent::Delta(d) => {
                self.pending.push_str(&d);
                if self.pending.len() >= FLUSH_BYTES
                    || (self.clock)().saturating_sub(self.last_flush) >= FLUSH_MS
                {
                    self.flush();
                }
            }
            AgentEvent::Replace(t) => {
                self.pending.clear();
                let mut s = self.shared.lock().expect("snapshot poisoned");
                let mut end = t.len().min(MAX_TEXT_BYTES);
                while !t.is_char_boundary(end) {
                    end -= 1;
                }
                s.text = t[..end].to_owned();
                s.revision += 1;
                drop(s);
                self.last_flush = (self.clock)();
                (self.notify)();
            }
        }
    }
}

/// Runs one request to its end. `notify` is called after each visible change. The result goes
/// through [`Run::finish`], so a cancel committed first always wins over a late success.
pub fn execute(
    runner: &dyn AgentRunner,
    prepared: PreparedAgentRun,
    run: &Run,
    shared: &Shared,
    clock: &dyn Fn() -> u64,
    notify: &mut dyn FnMut(),
) -> Result<AgentOutput, AgentError> {
    let start = clock();
    let mut pump = Pump {
        run,
        shared,
        clock,
        notify,
        pending: String::new(),
        last_flush: start,
    };
    let result = runner.run_prepared(prepared, &mut pump, &run.token());
    pump.flush();
    run.finish(result)
}

#[cfg(test)]
mod tests;

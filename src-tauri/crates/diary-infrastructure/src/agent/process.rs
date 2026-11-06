//! Generic one-shot CLI supervisor: fixed argv, no shell, new process group, allowlisted
//! environment, concurrent bounded pipe draining, deadlines, TERM/KILL escalation, reaping and
//! escaped-descendant detection. It knows nothing about any agent; adapters interpret the records.
//!
//! The leader is polled with `waitid(WNOWAIT)` so its zombie keeps the PID and process group
//! reserved until we have signalled the group; only then is it reaped. Output content is passed to
//! the caller's record callback and never retained here (only a 2 KiB stderr tail, for the
//! adapter's error classification).
use diary_application::agent::{
    CancelToken, INACTIVITY_SECS, MAX_PROCESS_OUTPUT_BYTES, MAX_PROTOCOL_RECORD_BYTES, WALL_SECS,
};
use std::{
    collections::HashSet,
    ffi::OsString,
    io::{Read, Write},
    os::unix::process::{CommandExt, ExitStatusExt},
    path::PathBuf,
    process::{Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};

const TICK: Duration = Duration::from_millis(100);
const SCAN_EVERY: Duration = Duration::from_millis(300);
const STDERR_TAIL: usize = 2048;

#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub inactivity: Duration,
    pub wall: Duration,
    pub record: usize,
    pub total: usize,
    /// Time between TERM and KILL.
    pub grace: Duration,
    /// Time to keep draining pipes once the leader is gone.
    pub drain: Duration,
}
impl Limits {
    pub const fn production() -> Self {
        Self {
            inactivity: Duration::from_secs(INACTIVITY_SECS),
            wall: Duration::from_secs(WALL_SECS),
            record: MAX_PROTOCOL_RECORD_BYTES,
            total: MAX_PROCESS_OUTPUT_BYTES,
            grace: Duration::from_secs(2),
            drain: Duration::from_secs(1),
        }
    }
    /// Status probes: a few seconds and a small output bound.
    pub const fn probe() -> Self {
        Self {
            inactivity: Duration::from_secs(5),
            wall: Duration::from_secs(5),
            record: 4096,
            total: 16 * 1024,
            grace: Duration::from_millis(500),
            drain: Duration::from_millis(500),
        }
    }
}

pub struct Spec {
    /// Absolute path; resolved by the caller, never searched here.
    pub executable: PathBuf,
    pub argv: Vec<OsString>,
    /// The complete environment: nothing is inherited.
    pub env: Vec<(OsString, OsString)>,
    pub stdin: Vec<u8>,
    pub cwd: PathBuf,
    pub limits: Limits,
    /// Set at server shutdown; stops the run like a cancel (TERM, grace, KILL, reap).
    pub shutdown: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    Success,
    Status(i32),
    Signal(i32),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stop {
    Cancelled,
    Inactivity,
    Wall,
    RecordTooLarge,
    OutputTooLarge,
}
#[derive(Debug)]
pub struct Outcome {
    pub exit: Exit,
    pub stop: Option<Stop>,
    /// Descendants that left the process group and were still alive after the leader was reaped.
    pub escaped: usize,
    /// Last bytes of stderr; for pattern classification only, never logged or returned.
    pub stderr_tail: Vec<u8>,
    /// Bytes the child wrote to stdout and stderr (counted for the output limit).
    pub output_bytes: usize,
}

fn spawn_failed() -> std::io::Error {
    std::io::Error::other("spawn failed")
}

enum Msg {
    Out(Vec<u8>),
    Err(Vec<u8>),
    Eof(bool),
}

fn pump<R: Read + Send + 'static>(mut r: R, stdout: bool, tx: mpsc::Sender<Msg>) {
    std::thread::spawn(move || {
        let mut buf = vec![0u8; 16 * 1024];
        loop {
            match r.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let chunk = buf[..n].to_vec();
                    let msg = if stdout {
                        Msg::Out(chunk)
                    } else {
                        Msg::Err(chunk)
                    };
                    if tx.send(msg).is_err() {
                        return;
                    }
                }
            }
        }
        let _ = tx.send(Msg::Eof(stdout));
    });
}

/// One process-table row: enough to identify a process across PID reuse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Ident {
    pub pid: i32,
    pub start: (u64, u64),
}
#[derive(Debug, Clone, Copy)]
struct Row {
    ident: Ident,
    ppid: i32,
    pgid: i32,
    zombie: bool,
}

fn table() -> Vec<Row> {
    // SAFETY: a null buffer asks only for the byte count.
    let bytes = unsafe { libc::proc_listallpids(std::ptr::null_mut(), 0) };
    if bytes <= 0 {
        return Vec::new();
    }
    // libproc reports a PID count here, not bytes; sizing by `bytes / 4` truncated the table to
    // the newest ~quarter of processes. Over-allocating is harmless if a build reports bytes.
    let mut pids = vec![0i32; bytes as usize + 64];
    // SAFETY: the buffer is writable for `size_of_val` bytes.
    let got = unsafe {
        libc::proc_listallpids(
            pids.as_mut_ptr().cast(),
            std::mem::size_of_val(pids.as_slice()) as i32,
        )
    };
    pids.truncate(usize::try_from(got).unwrap_or(0).min(pids.len()));
    pids.into_iter().filter_map(row).collect()
}

fn row(pid: i32) -> Option<Row> {
    if pid <= 0 {
        return None;
    }
    // SAFETY: proc_bsdinfo is plain old data; all-zero is a valid value.
    let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of::<libc::proc_bsdinfo>() as i32;
    // SAFETY: `info` is writable for `size` bytes.
    let got = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDTBSDINFO,
            0,
            (&mut info as *mut libc::proc_bsdinfo).cast(),
            size,
        )
    };
    (got == size).then_some(Row {
        ident: Ident {
            pid,
            start: (info.pbi_start_tvsec, info.pbi_start_tvusec),
        },
        ppid: info.pbi_ppid as i32,
        pgid: info.pbi_pgid as i32,
        zombie: info.pbi_status == libc::SZOMB,
    })
}

/// Identity of a live, non-zombie process.
pub fn identify(pid: i32) -> Option<Ident> {
    row(pid).filter(|r| !r.zombie).map(|r| r.ident)
}

/// SIGKILL `target` only if the PID still belongs to the same process (same start time).
pub fn kill_if_same(target: Ident) -> bool {
    match row(target.pid) {
        Some(r) if r.ident == target && !r.zombie => {
            // SAFETY: the PID was just verified to be the observed process.
            unsafe { libc::kill(target.pid, libc::SIGKILL) == 0 }
        }
        _ => false,
    }
}

/// Descendants seen so far (by ancestry) keyed by identity, refreshed while the leader runs because
/// an orphan is reparented to launchd and its ancestry disappears.
struct Family {
    leader: i32,
    seen: HashSet<Ident>,
}
impl Family {
    fn scan(&mut self) {
        let rows = table();
        // Only identities still alive count as ancestors, so a reused PID cannot adopt strangers.
        let mut tracked: HashSet<i32> = HashSet::from([self.leader]);
        tracked.extend(
            rows.iter()
                .filter(|r| self.seen.contains(&r.ident))
                .map(|r| r.ident.pid),
        );
        let mut changed = true;
        while changed {
            changed = false;
            for r in &rows {
                if r.ident.pid != self.leader
                    && !r.zombie
                    && tracked.contains(&r.ppid)
                    && self.seen.insert(r.ident)
                {
                    tracked.insert(r.ident.pid);
                    changed = true;
                }
            }
        }
    }
    /// Group members still running; a leader that has exited is a zombie and does not count.
    fn group_alive(&self, pgid: i32) -> bool {
        table().iter().any(|r| r.pgid == pgid && !r.zombie)
    }
    /// Tracked descendants alive outside the run's group: kill them (identity-checked) and count.
    fn reap_escaped(&self, pgid: i32) -> usize {
        let rows = table();
        let mut count = 0;
        for r in rows {
            if self.seen.contains(&r.ident) && !r.zombie && r.pgid != pgid {
                count += 1;
                kill_if_same(r.ident);
            }
        }
        count
    }
}

fn leader_exited(pid: i32) -> bool {
    // SAFETY: siginfo_t is plain old data; zeroed means "no child state change".
    let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
    // SAFETY: WNOWAIT leaves the zombie in place, so the PID stays reserved.
    let r = unsafe {
        libc::waitid(
            libc::P_PID,
            pid as libc::id_t,
            &mut info,
            libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
        )
    };
    r == 0 && info.si_pid != 0
}

/// If `run` unwinds or returns early, the child's group must not outlive it: KILL and reap.
struct GroupGuard {
    pgid: i32,
    armed: bool,
}
impl Drop for GroupGuard {
    fn drop(&mut self) {
        if self.armed {
            // SAFETY: the leader is unreaped while armed, so its PID and group are still ours.
            unsafe {
                libc::killpg(self.pgid, libc::SIGKILL);
                let mut status = 0;
                libc::waitpid(self.pgid, &mut status, 0);
            }
        }
    }
}

pub struct Supervisor;

impl Supervisor {
    /// Runs one process to a terminal state. Terminal is reported only after the leader is reaped.
    /// `on_record` receives each complete stdout line without its newline.
    pub fn run(
        spec: Spec,
        cancel: &CancelToken,
        on_record: &mut dyn FnMut(&[u8]),
    ) -> std::io::Result<Outcome> {
        let lim = spec.limits;
        let mut cmd = Command::new(&spec.executable);
        cmd.args(&spec.argv)
            .env_clear()
            .envs(spec.env.iter().map(|(k, v)| (k, v)))
            .current_dir(&spec.cwd)
            .process_group(0)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = cmd.spawn()?;
        let pid = child.id() as i32;
        let pgid = pid; // process_group(0) makes the child its own group leader
        let mut guard = GroupGuard { pgid, armed: true };
        let started = Instant::now();

        let (tx, rx) = mpsc::channel();
        pump(
            child.stdout.take().ok_or_else(spawn_failed)?,
            true,
            tx.clone(),
        );
        pump(child.stderr.take().ok_or_else(spawn_failed)?, false, tx);
        let mut stdin = child.stdin.take().ok_or_else(spawn_failed)?;
        let input = spec.stdin;
        // Own thread: a child that never reads stdin must not stall the control loop. A write
        // error after the child exits is expected and ignored; dropping closes stdin.
        std::thread::spawn(move || {
            let _ = stdin.write_all(&input);
        });

        let mut family = Family {
            leader: pid,
            seen: HashSet::new(),
        };
        let (mut line, mut total) = (Vec::<u8>::new(), 0usize);
        let (mut out_eof, mut err_eof) = (false, false);
        let mut tail = Vec::<u8>::new();
        let mut stop: Option<Stop> = None;
        let mut last_byte = Instant::now();
        let mut last_scan = Instant::now() - SCAN_EVERY;
        let mut exited_at: Option<Instant> = None;

        loop {
            if last_scan.elapsed() >= SCAN_EVERY && exited_at.is_none() {
                family.scan();
                last_scan = Instant::now();
            }
            match rx.recv_timeout(TICK) {
                Ok(Msg::Out(b)) => {
                    last_byte = Instant::now();
                    total += b.len();
                    line.extend_from_slice(&b);
                    while let Some(i) = line.iter().position(|&c| c == b'\n') {
                        if i > lim.record {
                            stop = Some(Stop::RecordTooLarge);
                            break;
                        }
                        let rec: Vec<u8> = line.drain(..=i).collect();
                        on_record(&rec[..i]);
                    }
                    if stop.is_none() && line.len() > lim.record {
                        stop = Some(Stop::RecordTooLarge);
                    }
                    if stop.is_none() && total > lim.total {
                        stop = Some(Stop::OutputTooLarge);
                    }
                }
                Ok(Msg::Err(b)) => {
                    last_byte = Instant::now();
                    total += b.len();
                    tail.extend_from_slice(&b);
                    if tail.len() > STDERR_TAIL {
                        tail.drain(..tail.len() - STDERR_TAIL);
                    }
                    if total > lim.total {
                        stop = Some(Stop::OutputTooLarge);
                    }
                }
                Ok(Msg::Eof(true)) => out_eof = true,
                Ok(Msg::Eof(false)) => err_eof = true,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    out_eof = true;
                    err_eof = true;
                }
            }
            if stop.is_none() {
                if cancel.is_cancelled()
                    || spec
                        .shutdown
                        .as_ref()
                        .is_some_and(|f| f.load(std::sync::atomic::Ordering::SeqCst))
                {
                    stop = Some(Stop::Cancelled);
                } else if started.elapsed() >= lim.wall {
                    stop = Some(Stop::Wall);
                } else if last_byte.elapsed() >= lim.inactivity {
                    stop = Some(Stop::Inactivity);
                }
            }
            if stop.is_some() {
                break;
            }
            if exited_at.is_none() && leader_exited(pid) {
                exited_at = Some(Instant::now());
            }
            if let Some(at) = exited_at
                && ((out_eof && err_eof) || at.elapsed() >= lim.drain)
            {
                break;
            }
        }

        if stop.is_none() && !line.is_empty() {
            on_record(&line); // final record without a trailing newline
        }

        // Terminate the group: TERM, wait for the other members (bounded), then KILL.
        // SAFETY: the leader is unreaped, so `pgid` cannot belong to an unrelated group.
        unsafe { libc::killpg(pgid, libc::SIGTERM) };
        let deadline = Instant::now() + lim.grace;
        while family.group_alive(pgid) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        // SAFETY: as above; the leader is still unreaped.
        unsafe { libc::killpg(pgid, libc::SIGKILL) };
        let status = child.wait()?;
        guard.armed = false; // reaped: the PID and group are no longer ours to signal
        let escaped = family.reap_escaped(pgid);

        // Drain what the pipes still hold, then abandon the reader threads: a descendant that
        // escaped may hold a pipe open forever.
        let drain_until = Instant::now() + lim.drain;
        while !(out_eof && err_eof) {
            let left = drain_until.saturating_duration_since(Instant::now());
            match rx.recv_timeout(left) {
                Ok(Msg::Eof(true)) => out_eof = true,
                Ok(Msg::Eof(false)) => err_eof = true,
                Ok(Msg::Err(b)) if stop.is_none() => {
                    tail.extend_from_slice(&b);
                    if tail.len() > STDERR_TAIL {
                        tail.drain(..tail.len() - STDERR_TAIL);
                    }
                }
                Ok(_) => {}
                Err(_) => break,
            }
        }

        let exit = match (status.code(), status.signal()) {
            (Some(0), _) => Exit::Success,
            (Some(c), _) => Exit::Status(c),
            (None, Some(s)) => Exit::Signal(s),
            _ => Exit::Status(-1),
        };
        Ok(Outcome {
            exit,
            stop,
            escaped,
            stderr_tail: tail,
            output_bytes: total,
        })
    }
}

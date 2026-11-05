# P1-M4: Diagnostic logging work plan

Status: Done

Implementation plan for the [P1-M4 overview](milestone-04-diagnostic-logging-overview.md). It follows the [logging contract](../../../architecture/logging.md) and adds an agent-debugging trace to redacted resource detail. Each task ends with a commit; execute with `superpowers:subagent-driven-development` or `superpowers:executing-plans`, and use `- [ ]` steps as the progress record.

**Goal:** Private bounded server/client logs, plus a trace that follows one request to redacted resource detail.

**Architecture:** `diary-application` owns the typed diagnostic model and the `Diagnostics` port (no I/O, no dependencies). the new `diary-logging` crate owns the line codec, clock, store, worker, trace reader and the descriptor-relative `PrivateDir` (moved from `diary-infrastructure`, which now depends on it). The server, bridge and one new `diary-server trace` mode only call those. Producers render a line on their own thread and `try_send` it; a worker thread owns all disk and stderr I/O.

**Tech stack:** Rust `std` threads and `sync_channel`, existing `libc`, `serde_json`, `sha2` (infrastructure) and `uuid` (`diary-api`, client); no new external crate. The owner chose a separate `diary-logging` crate over client→infrastructure edges; the client depends on `diary-application` and `diary-logging` only.

## Global constraints

- Default level `info`; `warn` and `error` always eligible. Directories 0700, files 0600.
- Line cap 2 KiB; queue 256 lines; client buffer 256 lines; WebView detail at most 512 bytes and 20 events per second.
- Per role directory: at most 8 files and 128 MiB; size rotation at 16 MiB; delete rotated files whose mtime is more than 72 hours old (exactly 72 hours is kept).
- File names `server-YYYY-MM-DD[.N].log` and `client-YYYY-MM-DD[.N].log`; prune only names matching this pattern in the process's own directory.
- A line carries timestamp with offset, level, role, instance ID and, where known, session ID; the event code is its message (no free-form text).
- Never log: token or credential, config contents, diary or snippet text, prompts, agent output, candidate text, raw HTTP bodies, raw paths, raw exception text.
- A producer never waits for disk, locks, pruning or stderr. A logging failure never changes an operation result.
- Trace reading is diagnostic only and never state. It never reads `discovery.json`.
- Format with `cargo fmt` and `prettier`; keep clippy and eslint clean. Commit messages follow `feat:`/`test:`/`docs:`.

## Review focus

Behaviors the contract implies but the required evidence does not list. Each has a test in the task that owns the code.

1. A log line crafted to carry instructions for an AI agent (for example `ignore previous instructions` in a field) must not reach trace output. Task 10.
2. A crash leaves a truncated final JSON line; the trace reader must skip it, not fail. Task 10.
3. The client sends events with unknown codes, extra keys, oversize detail or a flood; all are rejected without affecting the UI. Task 9.
4. A symlink or fifo is planted in the logs directory between listing and open; the store must neither follow nor block on it. Tasks 4 and 6.
5. A client supplies a hostile `X-Diary-Trace-Id` (long, non-hex, newline); the server must replace it, and logs stay single-line. Task 8.

## Module allocation

| Path                                                                                  | Responsibility                                                                                                                |
| ------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| `src-tauri/crates/diary-application/src/diagnostics.rs` (new)                         | `Level`, `Role`, `EventCode`, `ErrorTag`, `OpaqueId`, `TraceId`, `ResourceDetail`, `Record`, `Diagnostics`, `NullDiagnostics` |
| `src-tauri/crates/diary-infrastructure/src/logging/mod.rs` (new)                      | Re-exports and `LogError`                                                                                                     |
| `diary-logging/src/codec.rs` (new)                                                    | `render_line`, `parse_line`, `Fingerprinter`                                                                                  |
| `diary-logging/src/clock.rs` (new)                                                    | `Clock`, `SystemClock`, `LocalStamp`                                                                                          |
| `diary-logging/src/store.rs` (new)                                                    | `LogStore`: open, append, rotate, cap, prune                                                                                  |
| `diary-logging/src/worker.rs` (new)                                                   | `LogWorker`: queue, thread, stderr notice, drain; implements `Diagnostics`                                                    |
| `diary-logging/src/trace.rs` (new)                                                    | `Selector`, `collect`, `TraceReport`                                                                                          |
| `diary-logging/src/private_fs.rs` (moved from infrastructure)                         | `FsError`, `PrivateDir`; add `list`, `metadata`, `append`, `lock`                                                             |
| `src-tauri/crates/diary-infrastructure/src/lifecycle.rs`                              | Extract `entropy`, `workspace_key`                                                                                            |
| `src-tauri/crates/diary-api/src/lib.rs`                                               | Trace-ID middleware, request-scoped records                                                                                   |
| `src-tauri/crates/diary-server/src/main.rs`                                           | Start worker, emit lifecycle events, `trace` mode                                                                             |
| `src-tauri/src/diagnostics.rs` (new), `main.rs`, `build.rs`, `capabilities/main.json` | Client sink, `log_client_event`, trace header, permission                                                                     |
| `src/adapters/tauri-bridge.ts`, `src/application/bridge.ts`                           | `logClientEvent` adapter and DTO                                                                                              |

## Record format

One JSON object per line, fixed key order, absent keys omitted:

```json
{
  "ts": "2026-11-05T09:12:03.123+09:00",
  "lvl": "warn",
  "role": "server",
  "inst": "<instance id>",
  "sess": "<session id>",
  "code": "save.rejected",
  "comp": "editor",
  "trace": "<32 hex>",
  "op": "<id>",
  "job": "<id>",
  "res": {
    "kind": "authentic_entry",
    "id": "2026-11-05",
    "rev": 7,
    "bytes": 1042,
    "lines": 21,
    "fp": "ab12cd34ef56ab78",
    "state": "stale",
    "path": "entries_root"
  },
  "err": "stale_revision",
  "n": 3
}
```

- `fp` is the first 8 bytes of SHA-256(per-process random key ‖ kind ‖ 0x00 ‖ bytes), hex. The key lives only in memory, so a fingerprint compares content across events of one process instance but cannot be reversed by a dictionary and is meaningless after restart.
- `id` is one of: UUID, `YYYY-MM-DD` date, or 16–64 lowercase hex. Anything else is not constructible.
- `path` is a class (`entries_root`, `runtime_root`, `source_root`, `outside_root`, `symlink`, `unknown`), never a path string.
- `state` and `err` are enum tags. There is no free-text field anywhere.

An agent reads a failure like `save.rejected … rev=7 fp=ab12… state=stale` and finds the earlier `save.admitted` for the same `trace`, comparing `fp` and `bytes` to see whether content changed between admission and rejection, without seeing the content.

## Tasks

### Task 1: Amend contracts

**Files:** modify `docs/architecture/logging.md`, `docs/architecture/api-and-processes.md`, `docs/phase-1/changelog.md`.

- [ ] **Step 1:** In `logging.md` add a "Trace and resource detail" section holding the Record format above, the `fp` key rule, the five selectors (`trace:<hex>`, `op:<id>`, `job:<id>`, `res:<kind>:<id>`, `recent:<n>` with n ≤ 50, the last n warn/error records) and the trace command's bounds (200 default, 500 maximum records, 256 KiB). Change "never read back as application state" to add: "except by the read-only trace command, which revalidates every line and never feeds state".
- [ ] **Step 2:** In `api-and-processes.md` add the `X-Diary-Trace-Id` request header (32 lowercase hex; otherwise replaced) and its echo on every response, and list `diary-server trace` in the launch section. Do not add a body field to the error schema. In the `log_client_event` row allow the optional `trace_id` and say the event code is the line's message.
- [ ] **Step 3:** Add a changelog entry dated 2026-11-05: owner requested an agent-debugging trace to redacted resource detail; no dependency changed; `tracing` not adopted.
- [ ] **Step 4:** Run `npx prettier --write docs` and a relative-link check. Expected: no broken links.
- [ ] **Step 5:** Commit `docs: define diagnostic trace and resource detail`.

### Task 2: Diagnostic model

**Files:** create `diary-application/src/diagnostics.rs`; modify `diary-application/src/lib.rs` (`pub mod diagnostics;`); tests in `diary-application/src/tests.rs`.

**Produces** (later tasks rely on these names):

```rust
pub enum Level { Error, Warn, Info }                 // as_str() -> "error"|"warn"|"info"
pub enum Role { Server, Client }
pub enum EventCode { ServerStarted, ServerStopping, ListenerBound, DiscoveryPublished, ConfigRejected,
    SessionOpened, SessionClosed, ClientBusy, AuthRejected, JobAdmitted, JobFinished,
    LogDropped, UiBridgeError, UiRenderError, ClientEventRejected }
impl EventCode { pub const fn as_str(self) -> &'static str; pub fn parse(&str) -> Option<Self>;
                 pub const fn webview_allowed(self) -> bool; }  // only the Ui* codes
pub enum ErrorTag { StaleRevision, ExternalChange, Invalid, Busy, Unavailable, Limit, Internal }
pub enum StateTag { Saved, Stale, Conflict, Missing, Unreadable, Pending }
pub enum PathClass { EntriesRoot, RuntimeRoot, SourceRoot, OutsideRoot, Symlink, Unknown }
pub enum ResourceKind { AuthenticEntry, SourceSnippet, Audio, ExportTarget, Config, Job, Operation }
pub struct OpaqueId(String);  // OpaqueId::parse(&str) -> Option<Self>; UUID | YYYY-MM-DD | 16–64 lowercase hex
pub struct TraceId(String);   // TraceId::parse(&str) -> Option<Self> (32 lowercase hex); producers build one with Uuid::new_v4().simple()
pub enum Component { Editor, Bridge, Layout, Settings }   // as_str / parse; the only WebView-supplied detail

pub struct ResourceDetail { pub kind: ResourceKind, pub id: OpaqueId, pub revision: Option<u64>,
    pub bytes: Option<u32>, pub lines: Option<u32>, pub fingerprint: Option<[u8; 8]>,
    pub state: Option<StateTag>, pub path: Option<PathClass> }
pub struct Record { pub code: EventCode, pub level: Level, pub trace: Option<TraceId>, pub session: Option<OpaqueId>, pub operation: Option<OpaqueId>,
    pub job: Option<OpaqueId>, pub resource: Option<ResourceDetail>, pub error: Option<ErrorTag>, pub count: Option<u32> }
impl Record { pub fn new(code: EventCode, level: Level) -> Self; /* builder methods: trace, session, operation, job, resource, error, count */ }
pub trait Diagnostics: Send + Sync { fn emit(&self, record: Record); }
pub struct NullDiagnostics;  // implements Diagnostics, drops everything
```

- [ ] **Step 1:** Write failing tests: `opaque_id_accepts_uuid_date_and_hex_only` (rejects `"my diary"`, `"2026-13-40"`, 15-char hex, uppercase hex, newline), `trace_id_requires_32_lowercase_hex`, `event_code_round_trips_and_webview_subset_is_ui_only`.
- [ ] **Step 2:** Run `cargo test --manifest-path src-tauri/Cargo.toml -p diary-application diagnostics`. Expected: FAIL (module missing).
- [ ] **Step 3:** Implement the types above. Parsers return `None`; no constructor takes a free `String` into a record field. Calendar validation of the date form uses plain integer checks (no new dependency).
- [ ] **Step 4:** Re-run. Expected: PASS.
- [ ] **Step 5:** Commit `feat: add typed diagnostic model`.

### Task 3: Line codec and fingerprinter

**Files:** create `diary-logging/src/codec.rs` and its tests. `diary-application` is a dependency of the crate.

**Consumes:** Task 2 types. **Produces:**

```rust
pub struct Header<'a> { pub ts: &'a str, pub role: Role, pub instance: Option<&'a OpaqueId> }  // ts from LocalStamp::rfc3339
pub fn render_line(h: &Header, r: &Record) -> Option<String>;   // None if > 2048 bytes incl. newline
pub struct ParsedLine { pub epoch_ms: i64, pub level: Level, pub role: Role, pub instance: Option<OpaqueId>, pub record: Record }
pub fn parse_line(line: &str) -> Option<ParsedLine>;            // strict: unknown key, enum, id or length -> None
pub struct Fingerprinter { /* 16-byte key */ }
impl Fingerprinter { pub fn new(key: [u8; 16]) -> Self;
    pub fn describe(&self, kind: ResourceKind, id: OpaqueId, revision: Option<u64>, content: &[u8],
                    state: Option<StateTag>, path: Option<PathClass>) -> ResourceDetail; }  // fills bytes, lines, fingerprint
```

- [ ] **Step 1:** Failing tests: `render_then_parse_round_trips_every_field`; `render_is_single_line_with_fixed_key_order`; `render_drops_line_over_2kib` (many-field record cannot exceed it, so test with maximum fields and assert `Some` ≤ 2048, plus a direct oversize header ts); `parse_rejects_unknown_key_bad_enum_bad_id_and_truncated_json`; `parse_derives_epoch_ms_from_offset_timestamp`; `fingerprint_differs_by_key_and_content_and_never_contains_content` (describe a body containing `SECRET-BODY`, render, assert the line lacks it and that two keys give different `fp`); `describe_counts_bytes_and_lines`.
- [ ] **Step 2:** Run `cargo test … -p diary-logging codec`. Expected: FAIL.
- [ ] **Step 3:** Implement with `serde_json` (`#[serde(deny_unknown_fields)]` wire structs, converted to typed values through the Task 2 parsers) and `sha2`. Lines are written by `render_line` only; `parse_line` re-validates by converting every field again.
- [ ] **Step 4:** Re-run. Expected: PASS.
- [ ] **Step 5:** Commit `feat: add log line codec and resource fingerprints`.

### Task 4: Logging crate, private directory helpers and shared extractions

**Files:** create crate `diary-logging` (workspace member and default member; `diary-infrastructure` and `diary-server` depend on it); move `private_fs.rs` and the owner/mode check from infrastructure with its own `FsError` (infrastructure converts with `From<FsError> for ConfigError`/`LifecycleError`); update `tests/architecture.test.js` allowed edges; modify `lifecycle.rs` in infrastructure for the extractions.

**Produces:**

```rust
impl PrivateDir {
    pub fn list(&self) -> Result<Vec<String>, ConfigError>;                     // names only, no "." or ".."
    pub fn metadata(&self, name: &str) -> Result<FileMeta, ConfigError>;        // opened object; Err for symlink/nonregular
    pub fn lock(&self, name: &str) -> Result<File, ConfigError>;                // flock LOCK_EX|LOCK_NB on an opened 0600 file
}
pub struct FileMeta { pub len: u64, pub modified_epoch_secs: i64 }
pub fn entropy(buf: &mut [u8]) -> Result<(), LifecycleError>;                   // extracted from Discovery::fresh
pub fn workspace_key(canonical_entries_root: &Path) -> String;                  // extracted from WorkspaceLock::acquire
```

- [ ] **Step 1:** Failing tests: `list_returns_names_and_skips_nothing_hostile` (symlink and fifo appear in `list` but `metadata` returns `Err` for each and does not block); `lock_is_exclusive_across_handles`; `workspace_key_matches_existing_lock_key`.
- [ ] **Step 2:** Run the three tests. Expected: FAIL.
- [ ] **Step 3:** Implement with `fdopendir`/`readdir` on a dup of the directory fd, `openat` with `O_NOFOLLOW|O_NONBLOCK` then `fstat` for regular-file check, and `flock`. Make `Discovery::fresh` and `WorkspaceLock::acquire` call the extracted functions; their existing tests must still pass.
- [ ] **Step 4:** Run `cargo test … -p diary-infrastructure`. Expected: PASS including pre-existing tests.
- [ ] **Step 5:** Commit `feat: add private directory listing and lock helpers`.

### Task 5: Clock

**Files:** create `diary-logging/src/clock.rs`.

**Produces:**

```rust
pub struct LocalStamp { pub year: i32, pub month: u8, pub day: u8, pub epoch_secs: i64, pub offset_secs: i32, pub millis: u16 }
impl LocalStamp { pub fn date_string(&self) -> String; pub fn rfc3339(&self) -> String;   // "2026-11-05", "…+09:00"
                  pub fn parse_rfc3339(s: &str) -> Option<i64>; }                            // epoch milliseconds
pub trait Clock: Send + Sync { fn now(&self) -> LocalStamp; }
pub struct SystemClock;   // libc::clock_gettime + localtime_r
```

- [ ] **Step 1:** Failing tests with a pure `LocalStamp::from_epoch(epoch_secs, offset_secs, millis)` constructor: `rfc3339_formats_offset_east_and_west`, `date_changes_at_local_midnight_not_utc_midnight`, `dst_day_has_23_or_25_hours` (feed stamps either side of a transition using explicit offsets), `parse_rfc3339_round_trips_and_rejects_malformed`.
- [ ] **Step 2:** Run. Expected: FAIL.
- [ ] **Step 3:** Implement. `SystemClock::now` calls `localtime_r` for the offset; tests never use the system clock. A `#[cfg(test)] FakeClock(Mutex<LocalStamp>)` with `set` lives in the module for Tasks 6–7.
- [ ] **Step 4:** Run. Expected: PASS.
- [ ] **Step 5:** Commit `feat: add injectable local clock`.

### Task 6: Log store

**Files:** create `diary-logging/src/store.rs`; `LogError { Unavailable, Capped, Oversize, Locked }`).

**Consumes:** Tasks 4–5. **Produces:**

```rust
pub struct LogStore { /* PrivateDir for logs/<role>, lock handle, active file, size, clock */ }
impl LogStore {
    pub fn open(workspace_dir: &PrivateDir, role: Role, clock: Arc<dyn Clock>) -> Result<Self, LogError>;
    pub fn append(&mut self, line: &str) -> Result<(), LogError>;   // rotates by date/size; Err(Capped) = drop
    pub fn prune(&mut self) -> Result<usize, LogError>;             // returns deleted count
}
```

- [ ] **Step 1:** Failing tests, each with `FakeClock` and a `tempfile` workspace: `open_creates_0700_dirs_and_0600_file`; `second_open_is_locked`; `rolls_over_at_local_midnight_including_dst`; `size_rotation_uses_next_free_suffix_and_never_truncates_after_restart`; `backward_clock_change_appends_to_existing_date_file_without_truncation`; `caps_at_eight_files_or_128_mib_drops_without_deleting_younger_files`; `prune_keeps_exactly_72h_and_deletes_72h_plus_one_second` (set mtime with `File::set_modified`); `prune_runs_on_open_and_on_rotation`; `prune_never_deletes_active_unrelated_or_symlinked_names`; `append_rejects_line_over_2kib`.
- [ ] **Step 2:** Run `cargo test … logging::store`. Expected: FAIL.
- [ ] **Step 3:** Implement. Name parsing accepts only `^role-\d{4}-\d{2}-\d{2}(\.\d+)?\.log$`. Size rotation happens before an append that would pass 16 MiB. File mtime comes from `PrivateDir::metadata`. Shortcut: total-size accounting rescans the directory on rotation only; note `shortcut: rescan at rotation, per-append accounting if profiling shows need`.
- [ ] **Step 4:** Run. Expected: PASS.
- [ ] **Step 5:** Commit `feat: add rotating private log store`.

### Task 7: Log worker

**Files:** create `diary-logging/src/worker.rs`.

**Consumes:** Tasks 3, 5, 6. **Produces:**

```rust
pub struct LogWorker { /* sync_channel(256), JoinHandle, dropped: AtomicU32 */ }
impl LogWorker {
    pub fn start(store: Option<LogStore>, role: Role, instance: Option<OpaqueId>,
                 clock: Arc<dyn Clock>, stderr: Box<dyn Write + Send>) -> Arc<Self>;
    pub fn shutdown(&self, drain: Duration);          // bounded best-effort drain, then abandon
}
impl Diagnostics for LogWorker { fn emit(&self, record: Record); }   // render, try_send, count drops
```

Producers render with `render_line` and `try_send`; the thread appends, runs `prune` hourly (clock-driven, also while idle or capped), emits one `log.dropped` record with the drop count when nonzero, and writes at most one redacted stderr notice per minute on store errors. With `store: None` it writes accepted lines to stderr under the same drop policy.

- [ ] **Step 1:** Failing tests: `emit_never_blocks_when_writer_is_stalled` (writer blocked on a `Mutex`; 10,000 emits finish under 1 s and extra are counted dropped); `stalled_stderr_does_not_block_emit`; `at_most_one_stderr_notice_per_minute` (store fails, fake clock advances 59 s then 61 s); `hourly_prune_runs_while_idle`; `shutdown_drain_is_bounded`; `log_dropped_record_reports_count`.
- [ ] **Step 2:** Run `cargo test … logging::worker`. Expected: FAIL.
- [ ] **Step 3:** Implement with `std::thread` and `sync_channel`; the thread loop uses `recv_timeout` against a clock-derived next-prune time.
- [ ] **Step 4:** Run. Expected: PASS.
- [ ] **Step 5:** Commit `feat: add bounded asynchronous log worker`.

### Task 8: Server integration and trace header

**Files:** modify `diary-server/src/main.rs`, `diary-api/src/lib.rs`, `diary-api/src/tests.rs`; `ServerState::new` takes `Arc<dyn Diagnostics>`.

- [ ] **Step 1:** Failing tests in `diary-api`: `valid_trace_header_is_echoed_and_recorded`; `invalid_trace_header_is_replaced` (long value, uppercase hex, embedded newline, 33 hex); `auth_rejection_emits_auth_rejected_without_token_or_body` (uses a recording `Diagnostics` and a seeded token and body, asserts no rendered line contains either); `session_open_and_close_emit_events`; `fake_job_admit_and_finish_emit_events_with_job_id`; `job_admission_and_status_unaffected_by_stalled_diagnostics` (a `Diagnostics` impl that blocks forever on a full channel, a real `LogWorker` with a blocked writer and 300 queued emits; admission, status and cancel of the fake job still return normally; this is the E12 isolation proof until real saves exist).
- [ ] **Step 2:** Run `cargo test … -p diary-api`. Expected: FAIL.
- [ ] **Step 3:** Add a `from_fn` middleware layered outermost: take header `x-diary-trace-id`, `TraceId::parse`, else `TraceId::parse(&Uuid::new_v4().simple().to_string())` (`uuid` is already a `diary-api` dependency, so `diary-api` takes no new edge); store it in request extensions; set the response header. Handlers add `trace` to records. In `main.rs`, after `WorkspaceLock::acquire`, open `PrivateDir` for `runtime_root/<workspace_key>`, build `LogStore`, start `LogWorker`, emit `ServerStarted`, `ListenerBound`, `DiscoveryPublished`, `ServerStopping`; on any logging setup error continue with `store: None` (stderr only). On configuration failure before the lock, print the existing short error only.
- [ ] **Step 4:** Run the crate tests and `cargo clippy …`. Expected: PASS and clean.
- [ ] **Step 5:** Commit `feat: log server lifecycle and trace requests`.

### Task 9: Client logging

**Needs owner confirmation first:** add `diary-application` and `diary-infrastructure` (workspace path dependencies) to the `diary-client` manifest; see the overview. If declined, stop and redesign.

**Files:** create `src-tauri/src/diagnostics.rs` (+ `diagnostics/tests.rs`); modify `src-tauri/src/main.rs`, `bridge.rs`, `build.rs`, `capabilities/main.json`, `src/adapters/tauri-bridge.ts`, `src/application/bridge.ts` and their Vitest files.

**Produces:**

```rust
pub struct ClientLog { /* Buffering(VecDeque<String> cap 256) | Active(Arc<LogWorker>) | StderrOnly */ }
impl ClientLog {
    pub fn log(&self, record: Record);
    pub fn attach(&self, workspace_dir: &PrivateDir);           // flush buffer in order, then write directly
}
pub struct ClientEvent { code: String, level: String, trace_id: Option<String>, detail: Option<ClientDetail> }  // deny_unknown_fields; code must be webview_allowed
pub struct ClientDetail { component: Component, count: Option<u32>, error: Option<ErrorTag> }
#[tauri::command] fn log_client_event(event: ClientEvent, log: State<ClientLog>) -> Result<(), &'static str>
```

```ts
export type ClientEvent = {
  code: "ui.bridge_error" | "ui.render_error";
  level: "warn" | "error";
  traceId?: string;
  detail?: {
    component: "editor" | "bridge" | "layout" | "settings";
    count?: number;
  };
};
export function logClientEvent(event: ClientEvent): Promise<void>;
```

- [ ] **Step 1:** Failing Rust tests: `unknown_code_extra_key_and_server_only_code_rejected`; `detail_over_512_bytes_rejected`; `twenty_one_events_in_one_second_drops_the_21st` (injected monotonic ms); `buffer_keeps_first_256_lines_and_drops_newer_ones` (drop newest keeps the earliest startup context; dropped count is reported as `log.dropped` after attach); `attach_flushes_buffer_in_order`; `attach_falls_back_when_log_is_locked` (hold the Task 6 lock from another handle; the client keeps buffering to 256 and writes stderr, and a second client process does not compete); `no_log_path_in_any_command_response`; `command_manifest_invoke_handler_and_capability_lists_match` (extend the existing inventory test). Failing Vitest: `logClientEvent invokes the command with the typed payload` and `rejects an unknown code at the type boundary` (compile-time via `// @ts-expect-error`).
- [ ] **Step 2:** Run `cargo test --manifest-path src-tauri/Cargo.toml -p diary-client diagnostics` and `npx vitest run`. Expected: FAIL.
- [ ] **Step 3:** Implement. Add `log_client_event` to `COMMANDS`, `generate_handler!` and `allow-log-client-event` in `capabilities/main.json`. `attach` runs when discovery validates (existing `discovery.rs` path). The bridge generates one `TraceId` per outgoing server request and sends `X-Diary-Trace-Id`; bridge errors emit `UiBridgeError` with the same trace. Detail rendering reuses `render_line`; a record that cannot be built is dropped.
- [ ] **Step 4:** Run both suites plus clippy and eslint. Expected: PASS and clean.
- [ ] **Step 5:** Commit `feat: add typed client logging command`.

### Task 10: Trace reader and command

**Files:** create `diary-logging/src/trace.rs`; modify `diary-server/src/main.rs` (mode switch before config-only startup).

**Consumes:** Tasks 3, 4. **Produces:**

```rust
pub enum Selector { Trace(TraceId), Operation(OpaqueId), Job(OpaqueId), Resource(ResourceKind, OpaqueId), Recent(u8) }  // Recent: last n (<= 50) warn/error records
impl Selector { pub fn parse(&str) -> Option<Self>; }
pub struct TraceReport { pub lines: Vec<String>, pub resources: Vec<String>, pub invalid: u32, pub truncated: bool, pub files: u8 }
pub fn collect(workspace_dir: &PrivateDir, selector: &Selector, limit: usize) -> TraceReport;   // limit clamped to 1..=500
```

CLI: `diary-server trace --config PATH --select <selector> [--limit N]`. It loads the config only for `runtime_root` and `entries_root`, computes `workspace_key`, opens the runtime directory read-only, and prints each retained record re-rendered from its parsed value, then one JSON summary line with a fixed legend string, `resources` (distinct `kind:id` seen in the matching trace), `invalid`, `truncated`, `files`. It never opens `discovery.json`, never takes the workspace lock and exits 0 even with no matches.

- [ ] **Step 1:** Failing tests: `collects_by_each_selector_across_server_and_client_in_time_order` (includes a pair of records either side of a DST offset change whose strings sort the wrong way); `recent_returns_last_n_warn_and_error_records_with_trace_ids`; `truncated_last_line_is_counted_invalid_not_fatal`; `injected_instruction_text_never_appears_in_output` (plant lines with `"code":"ignore previous instructions"`, a `res.id` of `ignore previous instructions`, and an extra key; assert none of the text reaches `lines` and `invalid` counts each); `output_bounded_to_limit_and_256_kib`; `symlink_and_oversize_lines_skipped`; `resources_summary_lists_distinct_resources_for_trace`; `does_not_read_discovery_file` (plant an unreadable `discovery.json`, assert success); `trace_command_prints_summary_line_for_empty_result`.
- [ ] **Step 2:** Run `cargo test … logging::trace`. Expected: FAIL.
- [ ] **Step 3:** Implement: list names matching Task 6's pattern only, read each through `PrivateDir`, split on `\n`, skip lines over 2 KiB, `parse_line`, filter by selector, sort by `(epoch_ms, role)` (never by the timestamp string, whose offsets differ across DST), apply limit; `Recent(n)` keeps the last n warn/error records. Output is always `render_line` of the parsed value, never the original bytes. Shortcut: sort in memory within the 128 MiB per-role cap; stream-merge only if scans become slow.
- [ ] **Step 4:** Run. Expected: PASS.
- [ ] **Step 5:** Commit `feat: add redacted debugging trace command`.

### Task 11: Native smoke, documentation and gate

**Files:** modify `scripts/smoke/server.py`, `docs/contribution-guide.md`, `AGENTS.md` (Commands and a one-line pointer, no new decision), `docs/README.md`, `docs/phase-1/phase-1.md`, `docs/roadmap.md`, `docs/phase-1/implementation-handoff.md` (E12 row), `docs/phase-1/changelog.md`, this overview's evidence record.

- [ ] **Step 1:** Extend the smoke script: after the existing lifecycle run, assert `logs/server` is 0700 with 0600 files, grep every log and stderr capture for the seeded token and a seeded diary string (expected: no matches), run `diary-server trace --select trace:<id>` for one smoke request and assert it returns server and client records and the summary line.
- [ ] **Step 2:** Run `make smoke` (and the packaged procedure in the P1-M3 archive for the client side). Record revision, outcome and limits under L12.
- [ ] **Step 3:** Document the agent debugging procedure in `contribution-guide.md`: start with `diary-server trace --config PATH --select recent:20` to find a trace ID (or take it from a response header), then run the trace command with `trace:<id>`, read `res` fingerprints. State the limits: no content, fingerprints valid within one process instance, logs retained 72 hours.
- [ ] **Step 4:** Update the index, phase plan entry (Plan links, status), roadmap "Next", handoff E12 row, and changelog. Do not mark Done until every checklist item has evidence.
- [ ] **Step 5:** Run `scripts/gate.sh`. Expected: all six PASS. Record G01–G06 evidence in the overview; commit `docs: record P1-M4 evidence`.

## Acceptance matrix

| ID      | Evidence (tests named above)                                                                             |
| ------- | -------------------------------------------------------------------------------------------------------- |
| L01     | Task 1 diff; prettier and link check                                                                     |
| L02     | Task 2 tests; Task 3 `render_drops_line_over_2kib`, `parse_rejects_…`                                    |
| L03     | Task 4 `list_returns_names…`, `lock_is_exclusive…`; Task 6 `open_creates_0700…`, `second_open_is_locked` |
| L04     | Task 5 DST tests; Task 6 rollover, suffix, backward-clock tests                                          |
| L05     | Task 6 cap and prune tests; Task 7 `hourly_prune_runs_while_idle`                                        |
| L06     | Task 7 stall, notice, drain and `synthetic_save_and_job_unaffected…` tests                               |
| L07     | Task 8 server event and `auth_rejection…` tests                                                          |
| L08     | Task 9 Rust and Vitest tests, inventory test                                                             |
| L09     | Task 8 trace header tests; Task 9 bridge header assertion                                                |
| L10     | Task 3 fingerprint test; Task 8 and Task 9 seeded-secret scans; Task 11 smoke grep                       |
| L11     | Task 10 tests                                                                                            |
| L12     | Task 11 steps 1–2                                                                                        |
| L13     | Task 11 steps 3–4                                                                                        |
| G01–G06 | Task 11 step 5                                                                                           |

## Plan review record

Revised 2026-11-05 during execution: the owner chose the separate `diary-logging` crate (logging modules and `PrivateDir` moved; architecture test updated to seven packages; client edges are `diary-application` and `diary-logging`). The `Component`-style tags and all task interfaces are unchanged.

Reviewed 2026-11-05 before implementation (self-review plus advisor pass). Fixed:

- Crate edges: the client has no `diary-*` dependency; Task 9 now requires owner confirmation of two internal path dependencies, and the overview states the yaml-linking side effect. `diary-api` already has `uuid`, so trace-ID generation needs no new edge; the false claim that infrastructure has `uuid` is removed.
- Trace ordering: sorting by RFC 3339 string breaks across a DST offset change; lines now parse to `epoch_ms` and sort on it, with a test.
- Contract gaps: added `sess` (session ID), stated that the event code is the line's message, added `trace_id` to the `log_client_event` contract amendment, defined the `Component` enum and matching TypeScript union.
- Tests: moved the stalled-diagnostics isolation test to Task 8 where `ServerState` is available and named it as a job-admission proof (no save exists before P1-M6); added the competing client writer test; fixed the client buffer to drop newest.
- Debugging entry point: added the `recent:<n>` selector so an agent can find a trace ID with no UI change.

Resolved: the Task 9 crate edges (see the revision above).

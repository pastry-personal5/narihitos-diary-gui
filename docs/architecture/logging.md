# Logging

Status: Active

This contract defines diagnostic logging for the independent server and the Tauri client: where logs live, automatic rotation, the 3-day retention period, and what may never be written. Logging is diagnostic only; it is never a source of truth for saves, jobs or recovery.

## Locations and ownership

Each process writes its own files under the workspace's runtime directory, next to its [discovery record](api-and-processes.md#launch-and-discovery):

```text
<runtime_root>/<workspace-key>/logs/server/server-YYYY-MM-DD.log
<runtime_root>/<workspace-key>/logs/client/client-YYYY-MM-DD.log
```

The server derives the path from `paths.runtime_root`. The client has no config; it derives the same directory only after validating the `--connect` discovery file and its parent. Before that path is known, or if it is unusable, the client keeps a bounded in-memory buffer (256 lines) and writes to stderr only; it never searches for another location. Directories are owner-only (0700) and files 0600, like the rest of the [runtime directory](configuration-and-exports.md#runtime-directory). Reject symlinks/nonregular files during open, rotation and pruning, checking the opened objects rather than an earlier pathname observation. An OS lock gives one writer per role directory; another client process falls back to its bounded buffer/stderr instead of competing for the same log. Logs are not workspace, source or export data and are never exported, synced or read back as application state. The one exception is the read-only [trace command](#trace-and-resource-detail), which revalidates every line and never feeds state.

The Rust bridge logs its own events. The WebView reports diagnostics through one typed `log_client_event` command (see [operations](api-and-processes.md#operations-and-tauri-commands)); the bridge accepts only an allowlisted event code, a level and a bounded redacted detail (at most 512 bytes), at most 20 events per second, and writes them to the client log. The WebView never receives a log path or file access.

## Rotation and 3-day retention

- **Daily rotation:** a new file starts at the first write after local midnight, named for its local date. Rotate before a bounded record would exceed 16 MiB, using `…-YYYY-MM-DD.1.log`, `.2.log` and so on. Allocate unused suffixes without truncating earlier logs, including after restart or a backward clock change.
- **Total bounds:** at most eight log files/128 MiB per role directory, including the active log. Size rotation alone is not a total disk bound. At the cap, close a full or past-date active file and drop diagnostic records until ordinary age pruning frees capacity; do not delete younger files to make room or fail a user operation. At most 256 records of 2 KiB each wait in the asynchronous writer queue; overflow drops diagnostics. Error/warn records are eligible regardless of level but obey the same bounds.
- **Flood control:** a process accepts at most 60 non-error records per event code per minute; the excess is counted into one `log.dropped` record, so a noisy code (for example repeated rejected requests) cannot fill the file cap and silence the others. At the cap the store retries creating a file at most once a minute, not once per record. A file that ends in a partial line (crash) gets a newline before the next record, so the new record parses.
- **Retention:** delete rotated files whose last-modified time is more than 72 hours old. Retention is fixed at 3 days in config schema 1; it is not a config key.
- **When pruning runs:** at process start, every rotation and at least hourly from the diagnostic worker, including while capped or idle, over its own directory only. A process that runs for days therefore prunes and recovers capacity without a restart or successful write. Never delete the active file, and never touch files outside the process's own `logs/<role>/` directory or whose names do not match the pattern above.
- **Failure:** producers enqueue without waiting for disk, prune, file locks or stderr. A logging error (disk full, permission, prune failure) permits at most one redacted stderr notice per minute from the diagnostic worker. Slow/stalled stderr cannot block application work; use the same bounded drop policy. Shutdown abandons queued diagnostics after a bounded best-effort drain. Logging must not change any operation result.
- **Crash and clock changes:** a crashed process leaves its file for the next start's prune. A clock moving backwards can keep a file longer than 72 hours; moving forwards can prune early. Diagnostics tolerate both.

## Content rules

Default level is `info`; `warn` and `error` are always eligible subject to the bounds above. Each line carries timestamp with offset, level, role, instance and, where known, session ID, and the event code; the code is the short message (there is no free-form text). Operation, job and resource IDs may appear. Messages come from allowlisted codes and typed fields; a free-form WebView string cannot prove itself redacted. `log_client_event` accepts only the event schema's bounded noncontent fields (code, level, optional `trace_id` and an optional detail of a component tag, count and error tag), rejecting extra keys and raw exception/body strings.

Never log: the API token or any credential (including Notion), the config contents, diary or snippet body text, prompts, agent or chat output, candidate text, raw HTTP bodies, private home paths beyond what the error contract already allows in `details`, or raw exception text that may embed those. Apply the same redaction as [error bodies](api-and-processes.md#errors-and-bounds): codes and IDs, not content. A line that cannot be built without private content is dropped.

## Trace and resource detail

For debugging by a person or an AI coding agent, records can be followed across processes to redacted resource detail. One JSON object per line, fixed key order, absent keys omitted:

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

- `comp` is an optional component tag (`editor`, `bridge`, `layout`, `settings`) set only for WebView-reported events.
- `trace` is a 32-character lowercase hex ID per client request, carried in the `X-Diary-Trace-Id` header; the server replaces a missing or invalid value.
- `res` describes a resource with a kind, an ID that is only a UUID, a `YYYY-MM-DD` date or 16–64 lowercase hex, optional revision, byte and line counts, a state tag and a path class (`entries_root`, `runtime_root`, `source_root`, `outside_root`, `symlink`, `unknown`). It never holds a path string or content.
- `fp` is the first 8 bytes of SHA-256(per-process random key, kind, 0x00, bytes). The key lives only in memory: fingerprints compare content across events of one process instance, cannot be reversed by a dictionary, and mean nothing after restart.

### Agent records

Agent activity (P1-M20) uses four event codes and one optional `agent` object, written after `n`. Every field is a closed tag or a number; none takes text.

- `agent.check`: a status check (`status`, `reason`, `changed` = the `(status, reason)` pair differs from the previous check of that alias, `n` = milliseconds).
- `agent.blocked`: a run refused before any process started (the `reason` says why; `code` is the `AgentErrorCode` the caller saw).
- `agent.run`: a run that started a process, whatever its end (`stop`, `exit`, `xn` exit number, `esc` escaped descendants, `out` bytes, `n` = milliseconds).
- `agent.helper`: native helper events (extraction, hash check, launch, unmapped native error), used from the Apple adapter on.

```json
"agent": {"kind": "codex", "action": "chat", "phase": "receiving_output", "code": "timeout",
          "stop": "inactivity", "exit": "signal", "reason": "record_too_large", "status": "failed",
          "changed": true, "esc": 0, "ev": 9, "in": 1234, "out": 560, "xn": 15}
```

Tags: `kind` (`codex`, `claude`, `qwen`, `apple-foundation-models`); `action` (`check`, `chat`, `generate`); `phase` = last phase reached; `code` = one of the 11 `AgentErrorCode` values; `stop` (`cancelled`, `inactivity`, `wall`, `record_too_large`, `output_too_large`); `exit` (`success`, `code`, `signal`); `status` = a `ProfileStatus`; `reason` = the closed `Reason` set (see [editing and agents](editing-and-agents.md)). Levels: successes, cancellations and busy or unauthenticated checks are `info`; refusals and failures `warn`; escaped descendants `error`. Child stderr and native errors are classified into a `code` or `reason` in code and never logged; prompts, replies, diary text, paths, executable names and environment values never reach a record. Old lines without `agent` still parse, and the parser rejects any unknown agent key or value.

- `state` and `err` are enum tags; no field accepts free text.

`diary-server trace --config PATH --select <selector> [--limit N]` prints a bounded timeline merged from the server and client logs. Selectors: `trace:<hex>`, `op:<id>`, `job:<id>`, `res:<kind>:<id>`, `recent:<n>` (the last n ≤ 50 warn/error records, to find a trace ID). It is read-only, never reads the discovery file or takes the workspace lock, parses every line strictly, drops lines that fail validation (including a truncated last line), re-renders output from parsed values rather than copying bytes, sorts by epoch time, and prints at most 200 records (limit up to 500) and 256 KiB, followed by a summary line with distinct resources, invalid-line count and truncation.

## Required evidence

Synthetic fixtures with an injected clock must show: daily rollover at local midnight including DST changes, size/total-cap behavior and restart suffix allocation, pruning at exactly 72 hours (kept just under, deleted just over), pruning on start and at rotation in a long-running process, active file never deleted, unrelated files/symlinks untouched, 0700/0600 modes, competing client log writers, bounded client buffer/async queue, event rate/size limits and schema rejection, and a scan proving seeded token and body strings never appear. For the trace: all selectors, a truncated line, injected instruction text in a code, ID or extra key never reaching output, DST offset ordering, output bounds and no discovery-file read. A stalled writer/stderr and full queue must leave a save and a job unaffected.

`tracing` with a custom age-based pruner is the preferred candidate; `tracing-appender` rotates but prunes by file count, not age, so it does not satisfy this contract alone. Any logging dependency still needs approval and a license check before adoption.

## Generation records

P1-M22 extends the typed agent detail with optional candidate index, closed style ID, requested/ready/failed/cancelled/not-started counts. Adapter lifecycle records identify the candidate index and style; the API emits one terminal generation summary with the opaque job/session IDs and counts. These fields are numbers or closed enums; source bodies, candidate text, prompts, paths and raw diagnostics cannot be attached. The log codec round-trips the additional fields and continues rejecting unknown fields/tags.

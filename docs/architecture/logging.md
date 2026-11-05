# Logging

Status: Active

This contract defines diagnostic logging for the independent server and the Tauri client: where logs live, automatic rotation, the 3-day retention period, and what may never be written. Logging is diagnostic only; it is never a source of truth for saves, jobs or recovery.

## Locations and ownership

Each process writes its own files under the workspace's runtime directory, next to its [discovery record](api-and-processes.md#launch-and-discovery):

```text
<runtime_root>/<workspace-key>/logs/server/server-YYYY-MM-DD.log
<runtime_root>/<workspace-key>/logs/client/client-YYYY-MM-DD.log
```

The server derives the path from `paths.runtime_root`. The client has no config; it derives the same directory only after validating the `--connect` discovery file and its parent. Before that path is known, or if it is unusable, the client keeps a bounded in-memory buffer (256 lines) and writes to stderr only; it never searches for another location. Directories are owner-only (0700) and files 0600, like the rest of the [runtime directory](configuration-and-exports.md#runtime-directory). Reject symlinks/nonregular files during open, rotation and pruning, checking the opened objects rather than an earlier pathname observation. An OS lock gives one writer per role directory; another client process falls back to its bounded buffer/stderr instead of competing for the same log. Logs are not workspace, source or export data and are never exported, synced or read back as application state.

The Rust bridge logs its own events. The WebView reports diagnostics through one typed `log_client_event` command (see [operations](api-and-processes.md#operations-and-tauri-commands)); the bridge accepts only an allowlisted event code, a level and a bounded redacted detail (at most 512 bytes), at most 20 events per second, and writes them to the client log. The WebView never receives a log path or file access.

## Rotation and 3-day retention

- **Daily rotation:** a new file starts at the first write after local midnight, named for its local date. Rotate before a bounded record would exceed 16 MiB, using `…-YYYY-MM-DD.1.log`, `.2.log` and so on. Allocate unused suffixes without truncating earlier logs, including after restart or a backward clock change.
- **Total bounds:** at most eight log files/128 MiB per role directory, including the active log. Size rotation alone is not a total disk bound. At the cap, close a full or past-date active file and drop diagnostic records until ordinary age pruning frees capacity; do not delete younger files to make room or fail a user operation. At most 256 records of 2 KiB each wait in the asynchronous writer queue; overflow drops diagnostics. Error/warn records are eligible regardless of level but obey the same bounds.
- **Retention:** delete rotated files whose last-modified time is more than 72 hours old. Retention is fixed at 3 days in config schema 1; it is not a config key.
- **When pruning runs:** at process start, every rotation and at least hourly from the diagnostic worker, including while capped or idle, over its own directory only. A process that runs for days therefore prunes and recovers capacity without a restart or successful write. Never delete the active file, and never touch files outside the process's own `logs/<role>/` directory or whose names do not match the pattern above.
- **Failure:** producers enqueue without waiting for disk, prune, file locks or stderr. A logging error (disk full, permission, prune failure) permits at most one redacted stderr notice per minute from the diagnostic worker. Slow/stalled stderr cannot block application work; use the same bounded drop policy. Shutdown abandons queued diagnostics after a bounded best-effort drain. Logging must not change any operation result.
- **Crash and clock changes:** a crashed process leaves its file for the next start's prune. A clock moving backwards can keep a file longer than 72 hours; moving forwards can prune early. Diagnostics tolerate both.

## Content rules

Default level is `info`; `warn` and `error` are always eligible subject to the bounds above. Each line carries timestamp with offset, level, role, instance or session ID where known, event code and short message. Operation, job and resource IDs may appear. Messages come from allowlisted codes and typed fields; a free-form WebView string cannot prove itself redacted. `log_client_event` accepts only the event schema's bounded noncontent fields, rejecting extra keys and raw exception/body strings.

Never log: the API token or any credential (including Notion), the config contents, diary or snippet body text, prompts, agent or chat output, candidate text, raw HTTP bodies, private home paths beyond what the error contract already allows in `details`, or raw exception text that may embed those. Apply the same redaction as [error bodies](api-and-processes.md#errors-and-bounds): codes and IDs, not content. A line that cannot be built without private content is dropped.

## Required evidence

Synthetic fixtures with an injected clock must show: daily rollover at local midnight including DST changes, size/total-cap behavior and restart suffix allocation, pruning at exactly 72 hours (kept just under, deleted just over), pruning on start and at rotation in a long-running process, active file never deleted, unrelated files/symlinks untouched, 0700/0600 modes, competing client log writers, bounded client buffer/async queue, event rate/size limits and schema rejection, and a scan proving seeded token and body strings never appear. A stalled writer/stderr and full queue must leave a save and a job unaffected.

`tracing` with a custom age-based pruner is the preferred candidate; `tracing-appender` rotates but prunes by file count, not age, so it does not satisfy this contract alone. Any logging dependency still needs approval and a license check before adoption.

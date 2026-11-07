# P1-M25: Export review follow-up

Status: Done (2026-11-07)

Review and harden the delivered export coordination changes before P1-M26. Scope stays within the approved export contract; real destinations, cancellation and the separately tracked P1-M24 decisions remain outside this follow-up.

## Completion checklist

- [x] R01 — Persist intent and acquire the destination reservation before observation; regression tests verify ordering and blocked destinations make no remote calls.
- [x] R02 — Concurrent duplicate confirmations return the recorded admission and schedule exactly one write; test the transaction and concurrent service path.
- [x] R03 — Paused operations cannot be confirmed against a different destination root after restart; regression test preserves both destinations.
- [x] R04 — Export regression tests, synthetic built-server export smoke and all full-gate checks pass.

## Review findings

- Destination observation preceded intent/reservation, contrary to the shared export contract, and could inspect a destination reserved by another operation.
- Confirmation transaction checked consumed tokens before duplicate steps; the service also ignored the distinction between an applied and recorded admission.
- Confirmation selected an adapter by target only, leaving the confirmation path without a check that its adapter still addresses the recorded root. Existing start-input hashing already rejects a changed root; preview and confirmation now independently enforce that binding.

## Evidence

- `cargo test --manifest-path src-tauri/Cargo.toml -p diary-infrastructure export::tests --quiet`: all 16 tests passed outside the sandbox (native macOS file coordination is denied inside it).
- New regressions: `intent_and_reservation_precede_destination_observation`, `concurrent_duplicate_confirmation_records_one_step_and_writes_once`, and `changed_root_cannot_preview_or_confirm_a_paused_operation`.
- `python3 scripts/smoke/export.py`: passed, including create/no-op/confirmation and kill/restart read-back recovery. Confirmation assertions use the returned checkpoint rather than assuming one fixed internal step count.
- Full gate: `scripts/gate.sh` passed G01 and G03–G07 (formatting, Rust tests, Prettier, ESLint, Vitest and TypeScript). G02 first identified a collapsible-if lint in the new test; corrected, formatted, and `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --quiet -- -D warnings` then passed. All required checks are green after that targeted rerun.

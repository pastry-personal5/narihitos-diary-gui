# P1-M13: Native symbols and accessible interaction work plan

Status: Done

This plan adds a native symbol route and an accessibility audit to the existing Tauri client and React shell. Scope and checklist are in the [overview](milestone-13-native-symbols-accessibility-overview.md); the contract is the [platform symbols route](../architecture/configuration-and-exports.md#version-and-platform-strategy).

## Owner decisions

Interview 2026-11-06, recorded in the [changelog](changelog.md):

1. Add direct macOS-only dependencies `objc2`, `objc2-app-kit` and `objc2-foundation`, pinned (`=`) to the versions already in `Cargo.lock` (0.6.5, 0.3.2, 0.3.2) and re-checked at implementation, under `[target.'cfg(target_os = "macos")'.dependencies]` in `src-tauri/Cargo.toml`. Only the feature flags needed for `NSImage`, `NSImageRep`, `NSBitmapImageRep`, `NSGraphicsContext` and `NSGraphics` are enabled. License notices are reviewed with the existing `scripts/collect-notices.py` flow before the dependency lands.
2. The milestone closes on automated tests and the gate. Native checks are a known gap carried to P1-M28 and are not claimed.

## Design decisions

- **Command.** `get_symbol(name: String, size: u16, scale: u8) -> SymbolResult`, where `SymbolResult` is `{ status: "ok", png_base64 }` or `{ status: "unsupported" }`. Out-of-allowlist input is a typed error, not `unsupported`.
- **Allowlist (proposed, confirmed at runtime).** Names `doc.badge.plus` (Create Empty), `command` (Command palette), `gearshape` (Settings), `sparkles` (Generate), `xmark` (Close tab), `arrow.clockwise` (Refresh); sizes 12, 14, 16, 20; scales 1, 2; no appearance parameter. A name the OS does not know returns `unsupported`.
- **Rendering.** Run on the main thread (`AppHandle::run_on_main_thread` plus a oneshot channel). Declare the command `async fn` like the other bridge commands: a sync Tauri command already runs on the main thread, so waiting on the oneshot there would deadlock until the timeout. Look up with `NSImage(systemSymbolName:accessibilityDescription:)`, draw into an `NSBitmapImageRep` of `size*scale` pixels (black template, aspect-fitted), and encode PNG. Reject output above 32 KiB.
- **Tinting.** The client uses the PNG as a CSS `mask-image` over `currentColor`, so text color, disabled state and dark mode apply without per-color requests. If the template alpha is not usable on a symbol, record it and fall back to text for that name.
- **Cache.** `Map` keyed by `name|size|scale` in memory in `src/components/Symbol.tsx`, storing the in-flight promise so simultaneous mounts (for example several Close-tab buttons) share one native call; no disk cache, no persisted data. Cache `ok` and a genuine `unsupported` only; a command error or main-thread timeout is not cached as `unsupported` and may retry on the next mount.
- **Accessibility.** `Symbol` is `aria-hidden`. The control's text label stays in the DOM (visually hidden when the symbol shows) so the accessible name never depends on the symbol. Unsupported symbols show the text label and post one Global Message per name.
- **No new capability.** One new command, one new permission entry.

## Files

| File                                                                                        | Change                                                                                                                                                                                                                                                                                    |
| ------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src-tauri/Cargo.toml`, `Cargo.lock`                                                        | macOS-only direct dependencies above; notices regenerated.                                                                                                                                                                                                                                |
| `src-tauri/src/symbols.rs`                                                                  | New: allowlist, validation, `render` (macOS) and `unsupported` stub (other targets).                                                                                                                                                                                                      |
| `src-tauri/src/main.rs`, `build.rs`, `capabilities/main.json`, `tests/architecture.test.js` | Define the `#[tauri::command] async fn get_symbol` wrapper in `main.rs` (delegating to `symbols.rs`; the architecture test scans `main.rs` for commands and handler names), add `"get_symbol"` to `COMMANDS`, add `allow-get-symbol`, and add `get_symbol` to the test's `expected` list. |
| `src/application/bridge.ts`, `src/adapters/validation.ts`                                   | `getSymbol(request)` and response validation.                                                                                                                                                                                                                                             |
| `src/components/Symbol.tsx`                                                                 | New: `Symbol({name, size})`, cache, mask rendering, `onUnsupported` callback.                                                                                                                                                                                                             |
| `src/components/App.tsx`, `AuthenticPane.tsx`, `Explorer.tsx`                               | Use `Symbol` in the listed controls; keep labels, tooltips and shortcut hints unchanged.                                                                                                                                                                                                  |
| `src/components/PaneSeparator.tsx`, `GlobalMessages.tsx`                                    | Only if the audit tests fail (they appear correct today).                                                                                                                                                                                                                                 |
| `tests/symbols.test.tsx`, `tests/accessibility.test.tsx`                                    | New; fakes extend `tests/settings-fake.ts` style.                                                                                                                                                                                                                                         |

## Sequencing and tests

Red/green per task with a commit; run only affected tests until the final gate.

1. **Dependencies and notices (S10 part).** Add the pinned crates, regenerate notices, confirm `cargo tree` shows no new transitive crates beyond the lock. Check: `cargo test` still builds on macOS.
2. **Rust command (S01, S05).** Tests: `rejects_unlisted_name`, `rejects_unlisted_size_or_scale`, `png_has_signature_and_cap` (macOS only, `cfg(target_os = "macos")`), `unknown_symbol_is_unsupported`, `non_macos_is_unsupported`, and extend the existing `tests/architecture.test.js` manifest/handler/permission test with `get_symbol` instead of adding a second agreement test.
3. **Bridge and validation (S01).** Tests: `accepts_ok_and_unsupported`, `rejects_malformed_png_field`, `rejects_oversized_base64`.
4. **Symbol component (S02, S03, S04).** Tests: `renders_aria_hidden`, `cache_serves_repeat_without_second_call`, `unsupported_shows_label_and_posts_once`, `command_error_shows_label`, `disabled_control_keeps_name_and_tooltip`.
5. **Wire controls (S03).** Tests: `toolbar_controls_keep_names_and_hints` for Create Empty, Command palette, Settings, Generate, Close tab, Refresh (App.tsx and Explorer.tsx).
6. **Accessibility audit (S06–S09).** Tests in `tests/accessibility.test.tsx`: `every_interactive_has_name`, `tab_order_follows_layout`, `dialogs_and_palette_restore_focus`, `editor_tab_change_keeps_focus`, `inclusion_does_not_move_snippet_focus`, `separator_arrow_resize_clamps`, `separator_ignores_composition`, `messages_region_is_polite_log`, `save_waits_for_compositionend`. Fix only what fails.
7. **Docs and gate (S10).** Update the platform-symbols paragraph with the implemented allowlist and limits, handoff E9/E12, phase plan, roadmap, README, AGENTS.md decision entry and changelog; run `scripts/gate.sh`.

## Review focus

Inputs the contract implies but the tasks would not otherwise pin. Each has a test in the named task.

- A symbol name the OS removed or renamed must not blank the button (task 4, `unsupported_shows_label_and_posts_once`).
- A webview request with a huge size or negative scale must be rejected before any AppKit call (task 2, `rejects_unlisted_size_or_scale`).
- Rapid repeated renders of the same icon must make one native call (task 4, `cache_serves_repeat_without_second_call`).
- A disabled button must keep its accessible name and tooltip with the symbol showing (task 4, `disabled_control_keeps_name_and_tooltip`).
- Arrow keys during Korean composition must not resize a pane (task 6, `separator_ignores_composition`).

## Risks and limits

- AppKit calls off the main thread crash; every call goes through `run_on_main_thread`, with a timeout that returns `unsupported`.
- Symbol names and rendering are verified only by unit tests and, on macOS CI-less local runs, the PNG test; real Retina output is unverified until P1-M28; native VoiceOver and Japanese IME proof are not required in Phase 1.
- Using Apple's symbols is not license clearance; the owner proceeds despite that concern, as already recorded in AGENTS.md.

## Plan review findings and corrections

A review of this plan found seven issues. Six were corrected in the plan: the command must be `async` (a sync command runs on the main thread and the oneshot would deadlock); the wrapper lives in `main.rs` and `tests/architecture.test.js` lists `get_symbol`; the roadmap no longer says no dependency is approved; the phase goal matches the owner decision; the cache stores in-flight promises and does not cache errors; the P1-M12 phase status was stale. The seventh (drop `appearance`, wire Refresh) was resolved by the owner interview. Each maps to a test: `png_has_signature_and_cap` and the `async` wrapper (S01), the architecture test (S05), `shares one native call` and `falls back on a command error and retries` (S04).

# P1-M12: Command palette and keyboard shortcuts work plan

Status: Planned

This plan implements the [keyboard shortcuts contract](../architecture/keyboard-shortcuts.md) in the existing Rust user-settings store and React shell. It adds no dependency; scope and checklist are in the [overview](milestone-12-command-palette-shortcuts-overview.md).

## Design decisions

- **One registry file.** `src/application/actions/registry.json` holds action ids, labels, info, scopes, defaults, the reserved-key lists and canonicalization fixtures. TypeScript imports it (`resolveJsonModule` is on by default under `moduleResolution: Bundler`; confirm with `tsc`). Rust embeds it with `include_str!` and parses it with the existing `serde_json`. A Rust test and a Vitest test assert both sides see the same ids, scopes and defaults.
- **Canonical form.** `Control+Option+Shift+Command+<Key>`, in that order, key names per the contract (`P`, `S`, `Backspace`, `ForwardDelete`, arrows, `F1`…). Parsing is case-insensitive for modifiers and rejects duplicates, empty or multi-key strings. Rust is the authority on disk; TypeScript mirrors it, proved by shared fixtures.
- **Key capture and matching.** Derive the key from `event.code` (not `event.key`, which Option and Shift alter); map `Delete` to `ForwardDelete`. Known limit: layouts where `code` differs from the printed key (JIS, Korean) match by physical key; record it in the K09 evidence, do not claim layout parity.
- **IME guard.** Ignore a keydown when `event.isComposing || event.keyCode === 229`. WebKit may deliver the composition-ending keydown with `isComposing === false`, so the 229 check is mandatory. The same guard covers capture mode and Enter in the palette field.
- **Scope-aware reserved keys.** The reserved list is evaluated per scope: Command/Option+Backspace and +ForwardDelete are text-editing keys for `app` but valid `snippetTree` defaults. Reserved also includes Command+C/X/V/Z/A, Shift+Command+Z, plain and modifier+arrow keys, plain typing, and Command+Q/H/M/W/Tab.
- **Unknown entries survive.** The stored `keybindings` object is `{ id: [shortcuts] }`. Rust loads each entry independently: invalid entries are inactive and retained verbatim, with an issue `{id, code}` in the snapshot. A one-action edit (`update_user_settings` section `keybindings`, value `{ "set": {id: [..]} }`) changes only that key and keeps every other entry byte-for-byte; `{ "reset_all": true }` is the only operation that discards unknown or invalid entries. Corrupt structure (not an object) keeps today's `section_invalid` warning and defaults.
- **Effective merge.** defaults ← valid overrides; then detect scope-overlap conflicts (`app` overlaps every scope). An override always beats a default, so a default that collides with an override is deactivated and reported. Two overrides that collide (for example a hand-edited file) are both deactivated and both reported; the Settings editor itself never creates this state because it requires Reassign. Palette access is unaffected.
- **Snapshot change.** The user-settings snapshot gains `effective_keybindings: {id: [shortcut]}` and `keybinding_issues: [{id, code}]` (`unknown_action`, `invalid_shortcut`, `reserved`, `conflict`). Touches `src/application/bridge.ts`, `src/adapters/validation.ts` and the `update` union in `useSettings.ts`. No new Tauri command, event or capability.
- **Dispatch policy.** A single `keydown` listener in the capture phase on `window`: skip if suspended (capture mode, source-guard, recovery or Reset-all dialog), composing, or reserved; else resolve shortcut+scope → action. `snippetTree` resolves only when `document.activeElement` is inside the Snippet tree. A matched shortcut calls `preventDefault` and runs the handler; a disabled action posts its reason to Global Messages instead.
- **Handlers.** `ActionHandlers` map built in `App.tsx` from existing calls (`model.resources.createEmpty`, flush/`switchTab`, `model.refreshActiveSource`, settings toggle, palette). `editor.save` flushes the active Authentic buffer; for a source it reports "Source saving is not available yet" (P1-M14). Future actions return a reason string; none performs work.
- **Main process.** There is no Tauri menu or accelerator today (`git grep` found none), so no key is swallowed before the WebView. Do not add one.

## Review focus

Inputs the contract implies but the task tests above would not otherwise pin, most likely first. Each has a test in the named task.

- Backspace/ForwardDelete in the Tabbed Editor while the Snippet tree is not focused must still delete text (task 3, `editor_delete_unaffected_outside_tree`).
- A Korean/Japanese composition-ending keydown with `isComposing === false` must not trigger Command+S or a palette action (task 3, `ignores_composition_and_229`).
- A settings file already holding an unknown action id or a malformed shortcut must survive editing a different action (task 2).
- Rapid Command+S while an Authentic save is in flight must flush through the existing save queue, not start a second write (task 4, `save_flushes_queue_once`).
- Pressing a shortcut while the unsaved-source dialog, recovery dialog or Reset-all confirmation is open must do nothing (task 3, `suspended_during_capture_and_dialogs`).

## Files

| File                                         | Change                                                                                           |
| -------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| `src/application/actions/registry.json`      | New: data above.                                                                                 |
| `src/application/actions/shortcut.ts`        | New: `canonicalize(s: string): string \| null`, `fromEvent(e: KeyboardEvent): string \| null`.   |
| `src/application/actions/bindings.ts`        | New: `effectiveBindings(overrides): {bindings, issues}`, `conflictsFor(id, shortcut, bindings)`. |
| `src/application/actions/dispatcher.ts`      | New: `createDispatcher(handlers, state)` and the `keydown` policy.                               |
| `src/components/CommandPalette.tsx`          | New: dialog, label filter, listbox, focus restore.                                               |
| `src/components/KeyboardShortcuts.tsx`       | New: Settings table and capture; `SettingsView.tsx` gains a third section.                       |
| `src/components/App.tsx`, `Explorer.tsx`     | Route toolbar buttons through handlers; mark the Snippet tree region for scope resolution.       |
| `src-tauri/src/user_settings/keybindings.rs` | New: parse registry, `canonical`, `merge`, per-entry validation (replaces `valid_keybindings`).  |
| `src-tauri/src/user_settings.rs`, `main.rs`  | Use the module; extend the snapshot; typed `set` / `reset_all` changes.                          |

## Sequencing and tests

Each task is red/green with a commit; run only the affected tests until the final gate.

1. **Registry and canonicalization (K01, K02).** Tests: `registry_parity` (Rust and Vitest list the same 12 ids, scopes, defaults), `canonical_fixtures_match` (`Command+Shift+P` and `shift+command+p` both become `Shift+Command+P`; `Command+Command+P`, `Shift+P` in `app`, empty and two-key strings are rejected), `factory_defaults_validate` (all defaults pass the shipped validator, including the four `source.trash` shortcuts).
2. **Rust validation, merge, update (K03, K04).** Tests: `unknown_and_invalid_survive_single_edit` (stored unknown id plus invalid string; edit one action; both remain on disk), `reset_all_discards`, `new_default_does_not_steal_override`, `app_snippet_overlap_conflict_reported`, `reserved_is_scope_aware`, `empty_list_is_unbound`, `corrupt_section_falls_back`, `stale_revision_conflicts`, `newer_schema_rejects`. Snapshot contains no path or token.
3. **Pure bindings and dispatcher (K05, K08).** Tests: `ignores_composition_and_229`, `ignores_reserved`, `snippet_scope_requires_tree_focus`, `editor_delete_unaffected_outside_tree`, `suspended_during_capture_and_dialogs`, `disabled_action_posts_reason`, `save_on_source_never_succeeds`.
4. **Handlers and toolbar (K07).** The toolbar gains a Command palette button; there is no toolbar Save button, so Save stays reachable through the palette. Tests: `toolbar_buttons_call_registry`, `unbound_actions_remain_reachable`, `palette_button_opens_palette`, `save_flushes_queue_once`.
5. **Command palette (K06).** Tests: `filters_by_label`, `shows_shortcuts_and_disabled_reason`, `enter_runs_selected`, `enter_during_composition_does_nothing`, `escape_restores_invoker_focus`.
6. **Settings table (K09).** Tests: `columns_and_modified_marker`, `capture_records_code_not_key`, `escape_cancels`, `reassign_requires_confirmation`, `reset_and_reset_all_confirm`, `shortcut_exposed_as_text`, `read_only_disables_edit`.
7. **Docs and gate (K10).** Update the shortcut contract's evidence paragraph, handoff row E12, phase plan, roadmap, README and changelog; run `scripts/gate.sh`.

## Owner decisions

Interview 2026-11-06, recorded in the [changelog](changelog.md):

1. `app`-scope shortcuts also reserve Option-only printable keys and Control+letter text-editing keys (A, E, K, F, B, N, P). The [contract](../architecture/keyboard-shortcuts.md#editing-rules) is updated; reserved-list tests cover both.
2. A shortcut bound to a registered but unavailable action is consumed and its reason posted to Global Messages.
3. Command+S on a source tab posts "Source saving is not available yet" and never reports success.
4. The Keyboard Shortcuts table has no search field in this milestone.

## Native evidence

M12 owns unit and UI tests only. `native_smoke.rs` already counts trusted keydowns and can be extended later to check Shift+Command+P, Command+S, ForwardDelete outside the tree, and composition; that run belongs to P1-M13/P1-M28 with E12. Do not describe mock events as native key proof.

## Plan-review findings and corrections

Reviewed 2026-11-06 against the contract, `user_settings.rs` (`valid_keybindings` rejects the whole section on one bad entry), `App.tsx`, `Explorer.tsx`, `SettingsView.tsx`, `native_smoke.rs` and `tsconfig.json`.

| #   | Finding                                                                                                                       | Correction                                                                               | Item |
| --- | ----------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------- | ---- |
| F1  | Contract wants unknown/invalid entries retained, but the current validator drops the whole section on one bad entry.          | Per-entry validation; `set` edits one key, `reset_all` is the only discard.              | K04  |
| F2  | Command/Option+Backspace/ForwardDelete are `snippetTree` defaults yet text-editing keys under `app`.                          | Reserved list is scope-aware; defaults must validate under the shipped validator.        | K02  |
| F3  | Rust must validate and TypeScript must dispatch the same registry; two hand-kept copies would drift.                          | One JSON file read by both, with a parity test.                                          | K01  |
| F4  | `event.key` changes under Option/Shift and IME can end a composition with `isComposing === false`.                            | Match on `event.code`; guard `isComposing \|\| keyCode === 229`.                         | K05  |
| F5  | No toolbar palette or Save control exists, so an unbound action could become unreachable.                                     | Add a palette button; Save stays in the palette; toolbar buttons call registry handlers. | K07  |
| F6  | Disabled future actions with bound keys would otherwise pass through silently or run.                                         | Consume and post the reason to Global Messages; `source.trash` opens nothing.            | K08  |
| F7  | No menu accelerators exist, so no key reaches the WebView late; native key proof is still absent.                             | State the limit; defer to P1-M13/P1-M28.                                                 | K10  |
| F8  | Option-only printable and Control+letter shortcuts conflict with text input, but restricting them changes a decided contract. | Recorded as the owner question; not applied.                                             | —    |

Not planned, by design: the table search field (the contract says "may"), menu-bar accelerators, new dependencies.

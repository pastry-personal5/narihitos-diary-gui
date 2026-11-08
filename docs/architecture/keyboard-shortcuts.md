# Keyboard shortcuts

Status: Active

This contract defines GUI actions, their factory-default keyboard shortcuts, how an end user edits the bindings, and where they are stored. One action registry serves the toolbar, command palette and shortcut dispatch, as required by [UX GUI](../ux-gui.md#input-and-accessibility).

## Actions and bindings

An action is a stable `id` (lower-case dotted, such as `palette.open`), a label, an info sentence, a scope, and a list of default shortcuts (possibly empty). A binding pairs one action with zero or more shortcuts. Several shortcuts may trigger one action; one shortcut triggers at most one action per scope.

Scopes: `app` fires wherever focus is in the window; `snippetTree` fires only while focus is on a snippet in the Explorer's Snippet tab, so it never competes with text editing in the Tabbed Editor.

Factory defaults. Shortcuts use macOS names; the stored key names are `Command`, `Shift`, `Option`, `Control` plus a key such as `P`, `S`, `Backspace` or `ForwardDelete` (the Delete key that removes forward).

| Action ID                | Action                                                                                 | Scope         | Default shortcuts                                                              |
| ------------------------ | -------------------------------------------------------------------------------------- | ------------- | ------------------------------------------------------------------------------ |
| `palette.open`           | Open command palette                                                                   | `app`         | Shift+Command+P                                                                |
| `editor.save`            | Explicitly save the active source or flush the active Authentic editor                 | `app`         | Command+S                                                                      |
| `authentic.create_empty` | Create Empty for the selected date; immediately save an empty Authentic file if absent | `app`         | none                                                                           |
| `source.trash`           | Move a source diary snippet and related files to Trash                                 | `snippetTree` | Shift+Backspace, Shift+ForwardDelete, Command+Backspace, Command+ForwardDelete |
| `source.refresh`         | Refresh imported snippets                                                              | `app`         | none                                                                           |
| `generate.start`         | Generate candidates                                                                    | `app`         | none                                                                           |
| `ai.activity.open`       | Open AI Activities from the Main Toolbar Strip                                         | `app`         | none                                                                           |
| `ai.run.cancel`          | Cancel the active AI job; keep completed candidates                                    | `app`         | none                                                                           |
| `ai.results.open`        | View the selected job's results after normal date/save guards                          | `app`         | none                                                                           |
| `ai.results.discard`     | Explicitly discard completed transient results after review guards                     | `app`         | none                                                                           |
| `export.start`           | Start export of the saved revision                                                     | `app`         | none                                                                           |
| `settings.open`          | Open Settings                                                                          | `app`         | none                                                                           |

`source.trash` only opens the existing [delete preview](sources-and-dates.md#related-audio-and-trash); the confirmation step is unchanged, so no shortcut deletes without it. Actions without a default stay unbound until the user assigns one. This registry lists the GUI actions defined so far; adding an action means adding a row here and a registry entry, never a hard-coded key handler.

## Editing rules

- A shortcut is a modifier set plus one non-modifier key. `app` shortcuts require Command, Control or Option, since Shift alone types text. `snippetTree` shortcuts may use Shift alone with Backspace or ForwardDelete, as in the defaults.
- Reserved and not rebindable: text-editing and clipboard keys (Command+C/X/V/Z/A, Shift+Command+Z, arrows, Option+arrow, plain typing, Option with a printable key alone, Control+A/E/K/F/B/N/P). In `app` scope, Command/Option+Backspace and +ForwardDelete are also reserved; `snippetTree` allows them as defaults, macOS system keys (Command+Q/H/M/W/Tab), and any key during IME composition, which is never intercepted.
- A shortcut already used by another action in an overlapping scope (`app` overlaps every scope) is a conflict. The editor names the other action and requires an explicit Reassign, which removes it from that action; it never silently steals a binding.
- Removing every shortcut of an action is allowed; the action stays reachable from the command palette and toolbar. The palette and toolbar tooltips show an action's current shortcuts.

## Settings: Keyboard Shortcuts

Settings has a sidebar with status, writing language and **Keyboard Shortcuts**. The Keyboard Shortcuts section shows a table with one row per action:

| Column             | Content                                                                                     |
| ------------------ | ------------------------------------------------------------------------------------------- |
| Action             | Visible action label                                                                        |
| Original shortcuts | Factory defaults, read-only, or "None"                                                      |
| Current shortcuts  | Effective bindings; changed ones are marked "Modified"                                      |
| Info               | Scope and what the action does, plus a conflict or reserved-key message for the row in edit |

The row has Edit and Reset controls; a Reset all button restores every default after confirmation. Edit enters capture mode ("Press a shortcut"); a keypress is recorded, Escape cancels, and a chip list lets the user add another shortcut or remove one. Capture respects IME composition. The table is keyboard-operable with text names for every control, and shortcuts are exposed as text to VoiceOver (for example "Shift plus Command plus P"). A search field may filter rows; it adds no other behavior.

## Storage and API

Bindings are a presentation preference kept in the `keybindings` section of [user settings](user-settings.md), not in the server config or workspace. The section maps action ID to a shortcut list (an empty list means unbound) and stores only overrides, so new defaults and actions reach users who have overrides. Unknown action IDs and invalid shortcut strings are inactive and reported in the table, not fatal; retain them until that section is explicitly replaced. A missing or corrupt section means factory defaults with a visible message. Validate the complete effective registry after merging defaults and overrides: a newly added default must not silently steal an existing override. Deactivate conflicting shortcuts, report both actions, and preserve palette/toolbar access until explicit reassignment. `update_user_settings` validates the section against the registry and reserved list in Rust and returns the effective bindings; the server never sees bindings.

## Required evidence

Unit tests for registry defaults, the four `source.trash` defaults, override merge with new actions, reserved keys, `app` versus `snippetTree` overlap conflicts, reassign, empty list and invalid or corrupt file. UI tests for the table columns, modified marker, capture, Escape and reset. Actual WKWebView checks that Shift+Command+P, the four trash shortcuts (including ForwardDelete) and Command+S reach the registry, that editor text deletion is unaffected when the Snippet tree is not focused, and that IME composition is never intercepted. Mocks cannot establish the native key events; P1-M12 delivered only the unit and UI tests, so the WKWebView checks remain open for P1-M13/P1-M32.

## Command palette overlay

The Main Toolbar Strip button toggles the Command palette; its shortcut opens it. The non-modal overlay sits below the toolbar, focuses an empty search field, and leaves the toggle reachable. Search updates recommendations immediately. Matching uses normalized labels: exact, label prefix, ordered word prefixes, then substring. Within a match tier, available commands precede unavailable ones, then labels and stable action IDs break ties. Empty search uses availability, label and ID. Scores and rank numbers are never displayed.

Arrow keys select commands while focus stays in the search combobox; Enter or a pointer click runs the selected command once. Unavailable commands retain their reason and show the existing warning if attempted. Execution rechecks current availability and preserves save guards, previews and confirmations. IME composition never runs, navigates or dismisses commands; held Enter does not repeat execution. Escape restores invoker focus; outside pointer/focus dismisses without stealing the destination. Commands hand focus to Settings or their dialog. Another modal or shortcut capture closes the palette, and app shortcuts remain suspended while it is open. Reopening resets search and selection.

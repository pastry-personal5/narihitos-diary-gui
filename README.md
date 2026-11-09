# narihitos-diary-gui

Narihito's Diary, the GUI: a personal asset management app on your Mac. A Rust server and a Tauri 2 client (TypeScript + React, CodeMirror 6) for editing diary entries, drafting them with AI from source snippets, reviewing changes paragraph by paragraph, and exporting to iCloud or Notion.

Narihito mobile recording snippets (schema 1) and text-only manual snippets (schema 2) support guarded body-only Source Save. Text-only snippets need no audio; their Trash action remains unavailable.

With a Draft open in the Tabbed Editor, place the caret inside a paragraph and click **Paragraph Review** in the main toolbar. A ready Paragraph Review profile and Writing Language are required. The AI suggestion appears below the original with Accept and Reject at the top right. Accept replaces the paragraph through normal autosave; Reject keeps the original. Changes to the draft invalidate acceptance. Paragraph review has its own transient AI session and preserves your AI Chat transcript.

Server connection status and the Connect button are in **Settings > Server**. The app connects and reconnects automatically. Explorer views use icon buttons with tooltips, without a tab-name heading below the icons. The calendar uses custom bitmap chevron buttons for Previous month and Next month, with tooltips and accessible names.

- [User guide](docs/user-guide.md): what the app does today
- [Setup and development](docs/contribution-guide.md#setup-and-development)
- [Roadmap](docs/roadmap.md) and [all docs](docs/README.md)

## License

Apache-2.0. See [LICENSE](LICENSE) and the [third-party notices](third-party-notices.md).

## Contributing

See [AGENTS.md](AGENTS.md) and the [contribution guide](docs/contribution-guide.md).

Generate uses the selected eligible diary snippets even when the Entry tab is closed. Generation settings, blocked reasons, adjacent-date warnings and preparation/submission errors appear in Global Messages with recovery guidance. Draft and Entry use distinct custom bitmap buttons with tooltips and selected states; Jump to latest in Global Messages uses a matching bitmap button. The editor has no visible “Tabbed Editor” title; passive editor and save notices appear in Global Messages. The toolbar has no save-status text. Source snippet details have a custom bitmap Close button at the top right of the overlay, with a tooltip and Escape dismissal.

Settings → Server holds configuration status and Reload configuration. Settings → Sources lists configured sources and reveals the active server configuration folder in macOS Finder. Settings → AI Agents holds the Agent profiles table and separate Chat, Generation and Paragraph Review profile cards. Profile choices apply to new AI runs. Each profile has a saved **Show progress using Global Messages** toggle, on by default; enabled profiles report ongoing request/response turn progress every two seconds.

Open **Trash Recovery** in the Main Toolbar Strip to check interrupted moves, inspect each file’s saved result and review the next steps. Its dialog explains partial and uncertain outcomes; retrying remaining files requires a new preview and confirmation.

The Main Toolbar Strip uses distinct custom bitmap icon buttons, with tooltips for action names and keyboard shortcuts. Thin pane dividers highlight on hover, keyboard focus and drag; drag them or use arrow keys to resize.

Use **AI Activities** in the Main Toolbar Strip to inspect AI work, cancel a run or open its results in a spacious modal. Its header and guidance cards match Trash Recovery. Bitmap info buttons explain progress and temporary results. Closing the modal keeps work running; the bottom status line shows a brief activity summary.

The Command palette toolbar button toggles a search overlay with ranked command recommendations, current shortcut hints and unavailable reasons. Use arrows and Enter or click a command; Escape and outside interaction close it. See the [keyboard contract](docs/architecture/keyboard-shortcuts.md#command-palette-overlay).

Closing the main window of an app launched with `run-gui.sh` waits for save/session cleanup, exits the client and stops that launch’s development processes. The independently started diary server keeps running.

In AI Chat, send `/clear` to clear the conversation context and start a fresh chat. Wait for an active reply to finish or cancel it first. Enter sends a message; Shift+Enter adds a new line.

AI Chat labels replies **AI Agent**, using **AI Agent (provisional)** while a reply streams. UI and accessibility labels follow the [canonical UX terms](docs/ux-terms.md).

The AI Chat composer shows the next message’s document context in a thin-bordered box with a bitmap icon at the top left.

AI Chat’s Send button is a thin white up arrow inside a blue circle.

The empty AI Chat input shows a dimmed “Chat with AI” placeholder that disappears when typing.

AI Chat uses a very dark gray background with a darker input area.

The server connection starts automatically without a startup delay after frontend reload. Failed attempts retry within one second; returning to the window or recovering the network prompts an immediate retry.

Generate, Chat and Review report their current request/response stage in Global Messages every two seconds when the running profile’s progress toggle is on, including preparation and unchanged waits. Interrupted observations report status unavailable until refreshed. Completion, warnings and errors remain visible when progress is off.

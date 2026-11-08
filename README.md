# narihitos-diary-gui

Narihito's Diary, the GUI: a personal asset management app on your Mac. A Rust server and a Tauri 2 client (TypeScript + React, CodeMirror 6) for editing diary entries, drafting them with AI from source snippets, reviewing changes paragraph by paragraph, and exporting to iCloud or Notion.

Server connection status and the Connect button are in **Settings > Server**. The app connects and reconnects automatically. Explorer views use icon buttons with tooltips, without a tab-name heading below the icons.

- [User guide](docs/user-guide.md): what the app does today
- [Setup and development](docs/contribution-guide.md#setup-and-development)
- [Roadmap](docs/roadmap.md) and [all docs](docs/README.md)

## License

Apache-2.0. See [LICENSE](LICENSE) and the [third-party notices](third-party-notices.md).

## Contributing

See [AGENTS.md](AGENTS.md) and the [contribution guide](docs/contribution-guide.md).

Generate uses the selected eligible diary snippets even when the Entry tab is closed. Preparation and submission errors appear in Global Messages with recovery guidance.

Settings → Server holds configuration status and Reload configuration. Settings → Sources lists configured sources and reveals the active server configuration folder in macOS Finder. Settings → AI Agents holds the Agent profiles table and separate Chat profile and Generation profile cards. Profile choices apply to new AI runs.

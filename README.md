# narihitos-diary-gui

Narihito's Diary, the GUI: a personal asset management app on your Mac.

The app runs a separately launched Rust server and Tauri client, with saved Authentic editing, Create Empty, autosave and Settings. AI and exports remain unavailable. Setup and independent launch commands are in the [contribution guide](docs/contribution-guide.md#setup-and-development). Feature delivery follows the [roadmap](docs/roadmap.md).

Read-only Ayumi/Narihito date imports and timestamp-resolution choices are available through the server and typed bridge commands. Imports search source trees within shared scan limits and warn about misplaced timestamped files, including files outside the expected date directories. Listing pages stay fixed across file changes and server restarts; import again to refresh sources or apply timestamp choices. Narihito mobile snippets can be saved from the Source tab (explicit Save only; the title is left unchanged; Ayumi sources stay read-only). Verified against the mobile app's own file format on this Mac, not against the phone app or iCloud. See the [P1-M10 acceptance record](docs/archive/milestones/p1-m10/milestone-10-civil-date-imports-overview.md) and [snapshot hardening evidence](docs/archive/milestones/p1-m10/milestone-10-import-hardening-overview.md).

The planned AI agents are Codex, Claude Code, Qwen Code, and Apple's on-device Foundation Models. Agent integration is not enabled yet: Settings → Status shows whether each configured Codex profile is installed, a supported version and signed in (Check runs no AI), but nothing can start a run. Prompt templates live in [assets/prompts/](assets/prompts/README.md).

Writing language is selected in Settings, initially uses the Mac's preferred language, and is remembered across sessions. Planned generation requires selected snippets and continues for its original date when you navigate away. **Create Empty** saves a blank draft for the selected date, including without snippets.

Press Shift+Command+P (or use the toolbar) for the command palette; Command+S saves the active Authentic file or explicitly saves a supported Narihito source. Shortcuts are editable in Settings → Keyboard Shortcuts, and every action stays reachable from the palette. To move a supported Narihito snippet and its owned audio to macOS Trash, use Trash in the Explorer or command palette, review the exact files and confirm. Global Messages retains results and offers a fresh confirmation for safe partial retries. Directories stay in place; the mobile producer can show an empty recovery row for a retained assets directory. Ayumi source saves, generation and export remain unavailable. Toolbar and tab controls show macOS SF Symbols when the system provides them and fall back to text labels otherwise.

The [planned background AI experience](docs/architecture/ai-job-experience.md) keeps editing available with visible progress, cancellation and in-app completion feedback.

## License

Licensed under the Apache-2.0 license. See [LICENSE](LICENSE) and the [third-party notices](third-party-notices.md).

## Contributing

Contributors and AI agents: see [AGENTS.md](AGENTS.md) and [docs/contribution-guide.md](docs/contribution-guide.md).

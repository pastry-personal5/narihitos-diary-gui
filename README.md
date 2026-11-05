# narihitos-diary-gui

Narihito's Diary, the GUI: a personal asset management app on your Mac.

The P1-M3 scaffold now builds a separately launched Rust server and Tauri client with an in-memory Markdown scratch editor. Storage, imports, AI and exports are unavailable; scratch edits are never saved. Setup and independent launch commands are in the [contribution guide](docs/contribution-guide.md#setup-and-development). Milestone acceptance remains in progress.

The planned AI agents are Codex, Claude Code, Qwen Code, and Apple's on-device Foundation Models. Agent integration is not implemented yet. Prompt templates live in [assets/prompts/](assets/prompts/README.md).

Planned writing language is selected in Settings, initially uses the Mac's preferred language, and is remembered across sessions. Generation requires selected snippets and continues for its original date when you navigate away. **Create Empty** saves a blank draft for the selected date, including without snippets.

The [planned background AI experience](docs/architecture/ai-job-experience.md) keeps editing available with visible progress, cancellation and in-app completion feedback.

## License

Licensed under the Apache-2.0 license. See [LICENSE](LICENSE) and the [third-party notices](third-party-notices.md).

## Contributing

Contributors and AI agents: see [AGENTS.md](AGENTS.md) and [docs/contribution-guide.md](docs/contribution-guide.md).

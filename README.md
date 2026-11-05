# narihitos-diary-gui

Narihito's Diary, the GUI: a personal asset management app on your Mac.

The planned AI agents are Codex, Claude Code, Qwen Code, and Apple's on-device Foundation Models. Agent integration is not implemented yet. Prompt templates live in [assets/prompts/](assets/prompts/README.md).

Planned writing language is selected in Settings, initially uses the Mac's preferred language, and is remembered across sessions. Generation requires selected snippets and continues for its original date when you navigate away. **Create Empty** saves a blank draft for the selected date, including without snippets.

The [planned background AI experience](docs/architecture/ai-job-experience.md) keeps editing available with visible progress, cancellation and in-app completion feedback.

## License

Licensed under the Apache-2.0 license. See [LICENSE](LICENSE).

## Contributing

Contributors and AI agents: see [AGENTS.md](AGENTS.md) and [docs/contribution-guide.md](docs/contribution-guide.md).

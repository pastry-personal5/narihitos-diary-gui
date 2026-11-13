# User guide

Status: Active

Practical guidance for using Narihito's Diary, including setup, everyday
workflows, recovery and current limitations. The root [README](../README.md)
is a short product introduction.

The app runs a separately launched Rust server and Tauri client, with saved Authentic editing, Create Empty, autosave, Settings, a transient AI Chat, generated candidates and paragraph-by-paragraph review. iCloud export is available when an iCloud folder is configured; Notion export is available when a shared Notion token and a root for the required diary kind are configured. Setup and independent launch commands are in the [contribution guide](contribution-guide.md#setup-and-development). Feature delivery follows the [roadmap](roadmap.md).

Open **Settings → Server** to see the connection status or use **Connect**. Automatic connection and reconnection run in the background. Configuration reload is in **Settings → Server**, source diagnostics are in **Settings → Sources**, and profile diagnostics are in **Settings → AI Agents**.

Read-only Ayumi/Narihito date imports and timestamp-resolution choices are available through the server and typed bridge commands. Imports search source trees within shared scan limits and warn about misplaced timestamped files, including files outside the expected date directories. Listing pages stay fixed across file changes and server restarts; import again to refresh sources or apply timestamp choices. Narihito mobile recording snippets (schema 1) and manual text-only snippets (schema 2) can be saved from the Source tab (explicit Save only; the title is left unchanged; Ayumi sources stay read-only). Verified against the mobile app's own file format on this Mac, not against the phone app or iCloud. See the [P1-M10 acceptance record](archive/milestones/p1-m10/milestone-10-civil-date-imports-overview.md) and [snapshot hardening evidence](archive/milestones/p1-m10/milestone-10-import-hardening-overview.md).

Supported AI profiles are Codex, Claude Code, Qwen Code, and Apple's on-device Foundation Models. AI Chat is enabled: pick a ready Chat profile in Settings → AI Agents and send messages; each message carries the active source or Authentic buffer (including unsaved edits, shown as a context chip); while a Candidate tab is active, the Authentic buffer supplies context, the reply streams in as provisional text and Cancel stops it. The conversation is kept only in memory for the session. Settings → AI Agents shows provider-specific availability and supported-version or sign-in issues where detectable (Check runs no AI). To generate, include eligible snippets in the Explorer, select a Generation profile in Settings → AI Agents (readiness is checked in the background), then use Generate. Dirty selected sources require Save, Exclude or Cancel; the saved Entry is flushed before capture. Settings → Generation saves the candidate count (1–5, default 3). Prompt templates live in [assets/prompts/](../assets/prompts/README.md).

**AI Activities** in the Main Toolbar Strip opens a spacious modal showing runs across dates, captured profile and
language, progress and retained result indicators. View results navigates explicitly
through the usual save guards; dismissing an indicator retains the result.
Temporary connection loss marks activity stale. Reconnecting within the same live
lease discovers retained work without generating again; a server restart or expired
lease loses transient results. Packaged macOS recovery and responsiveness evidence
is still pending.

Writing language is selected in Settings, initially uses the Mac's preferred language, and is remembered across sessions. Generation requires selected snippets and continues for its original date when you navigate away. Candidate Editor tabs show provisional text until validation, stay read-only, and are kept only for the live session. The Draft tab lists their styles with Info and Open in Tabbed Editor actions. Cancel keeps completed candidates; Discard results frees their transient storage. Editing the saved Entry marks older candidates stale and requires regeneration before review. Use **Review candidate** on a finished candidate to review it against the saved Entry paragraph by paragraph: removed text is struck through in red, added text appears in green, and each change has Accept and Reject buttons. Accepting saves that paragraph at once; rejecting writes nothing; editing the Entry is paused until you choose **End review** (changing the date or regenerating ends it). If the Entry changes elsewhere, the remaining suggestions are dropped, accepted text stays, and you regenerate to review again. **Create Empty** saves a blank draft for the selected date, including without snippets.

Press Shift+Command+P (or use the toolbar) for the command palette; Command+S saves the active Authentic file or explicitly saves a supported Narihito source. Shortcuts are editable in Settings → Keyboard Shortcuts, and the palette lists commands with their availability. Type to search and order recommendations, use arrows and Enter to choose, or click a command. The toolbar button toggles the palette; Escape closes it and restores focus. Unavailable commands explain why they cannot run. To move a supported Narihito snippet and its owned audio to macOS Trash, use Trash in the Explorer or command palette, review the exact files and confirm. Use Trash Recovery in the Main Toolbar Strip to inspect saved results, check unfinished moves and review guidance for partial or uncertain outcomes. Inspect opens the file results and offers a fresh confirmation for safe partial retries; closing the result returns to Trash Recovery. The spacious dialog has a top-right Close button and supports Escape. Directories stay in place; the mobile producer can show an empty recovery row for a retained assets directory. Text-only manual snippets have no related audio and their Trash action remains unavailable. Ayumi source saves remain unavailable. Export in the toolbar saves pending Authentic edits first, then writes the acknowledged entry to `YYYY/MM/YYYY-MM-DD.md` under the configured iCloud folder, asks before replacing a file changed outside the app, and lets you resolve an export whose outcome is unknown. Retained exports can be resumed after reload; confirmation dialogs identify their original entry date. Toolbar and Explorer controls use app-owned bitmap icons with accessible names and tooltips; other controls use SF Symbols where supported.

The [background AI experience](architecture/ai-job-experience.md) keeps editing available with visible progress, cancellation and in-app completion feedback.

## Interface and feedback

The app connects to its separately launched server automatically; **Settings →
Server** shows connection status and offers **Connect** and **Reload
configuration**. Failed connections retry promptly, and returning to the app or
recovering network access prompts another attempt. **Settings → Sources** lists
configured sources and can reveal the active server configuration folder in
Finder.

The Main Toolbar Strip provides **AI Activities**, **Trash Recovery** and the
command palette. AI Activities lets you inspect or cancel work without stopping
it when the window closes; Trash Recovery explains and records interrupted
moves. The command palette shows shortcut hints and unavailable reasons; use
arrow keys and Enter, click a command, or press Escape to close it. Pane
dividers can be dragged or resized with arrow keys. Controls provide tooltips
and accessible names.

Global Messages show save, preparation, warning and recovery information. For a
profile with **Show progress using Global Messages** enabled (the default),
Generate, Chat and Review also report ongoing request and response stages every
two seconds. Completion, warnings and errors remain visible when it is off.

In AI Chat, Enter sends, Shift+Enter inserts a line break, and `/clear` starts a
fresh in-memory conversation after the active response finishes or is cancelled.
Replies are labeled **AI Agent** while complete and **AI Agent (provisional)**
while streaming. Chat context is shown before sending and is not retained after
the session ends.

Settings → Server holds configuration status and Reload configuration. Settings → Sources lists configured sources and reveals the active server configuration folder in macOS Finder. Settings → AI Agents holds the Agent profiles table and separate Chat, Generation and Paragraph Review profile cards. Profile choices apply to new AI runs. Each profile has a **Show progress using Global Messages** checkbox, on by default and remembered after restart. Enabled profiles report ongoing job and request/response turn stages every two seconds. Turn it off to stop periodic progress; completion, warnings and errors still appear.

The AI Activities window has a top-right Close button and supports Escape. Bitmap (i) buttons open further explanations. Close an explanation to return to its info button. Closing AI Activities keeps work running, and the bottom status line shows what is running or waiting. Use Reconnect when observations are out of date. Profile choices remain in Settings → AI Agents.

Generate, Chat and Review report their current request/response stage in Global Messages every two seconds when the running profile’s progress toggle is on, including preparation and unchanged waits. Interrupted observations report status unavailable until refreshed. Completion, warnings and errors remain visible when progress is off.

## Personal release layout and independent launch

Keep the standalone `diary-server`, `Narihito's Diary.app`,
`third-party-notices.md` and the accompanying `assets/third-party` license files
in the installation folder. Use ordinary release artifacts qualified by the
[release ledger](phase-1/milestone-32-release-evidence.md); development and
fault-injection builds are separate test tooling.

Start the server in one shell using absolute paths:

```sh
"/absolute/Diary Install/diary-server" --config "/absolute/private/config.yaml"
```

To write that configuration file, copy the [example configuration](global_config_example.yaml)
to a private location, replace every placeholder and set its mode to 0600.

Start the client from a second shell, using the discovery file published by
that server under its configured private runtime directory:

```sh
"/absolute/Diary Install/Narihito's Diary.app/Contents/MacOS/diary-client" --connect "/absolute/private/runtime/WORKSPACE_KEY/discovery.json"
```

Use real paths without symlink components. Keep configuration and runtime
credentials private; do not copy a discovery token between installations.
The client does not start or own the server. Closing the client waits for
save/session cleanup; stop the server separately with Ctrl+C after the client
has finished. Resolve failed or uncertain saves before closing; retry preserves
the operation identity so an acknowledged write is not repeated. On recovery,
use the displayed saved state and recovery controls rather than deleting the
workspace ledger. AI results and chat are session memory and do not survive a
server restart. Missing AI tools disable their actions while editing remains
available. Local export read-back does not establish remote iCloud upload;
text-only snippet Trash remains unavailable.

## Local image engine configuration

The optional private YAML `image_engines` map configures local PNG generators. See the [configuration and command contract](architecture/image-engines.md). Settings > AI Agents > Image engines shows configured engines and their effective Illustrated selection. Readiness is checked automatically in the background on connection and when opening AI Agents. Problems appear in Global Messages. Use Check to retry a prerequisite check; it does not generate a photo. Engine selection, prompts and the photo cap remain in private server configuration, and Illustrated configuration changes need a server restart. Stopping the server with SIGTERM or Ctrl+C also stops its active image engines and waits for cleanup.

## Illustrated diaries

1. Save an Authentic diary for the working date. In Settings > AI Agents, choose a ready Generation profile and wait for the automatic image engine check to finish. Choose a Writing Language if it is unresolved.
2. Choose **Create Illustrated Diary** in the toolbar or Command palette. Pending Authentic saves finish before capture. The action’s tooltip explains why creation is unavailable.
3. Follow text preparation and photo progress in **AI Activities**. Use Cancel AI run there to request cancellation; the same dialog can be closed while work continues. Ordinary Authentic editing and autosave keep working.
4. On success, a separate rendered, read-only Illustrated tab opens if you are still on that day. Otherwise Global Messages tells you where to open it. The **Illustrated** Explorer tab lists all published Illustrated dates, including dates whose Authentic file is absent.

Creating again silently replaces the previous version only when the complete new version is published. Failure or cancellation before publication preserves the previous version. Cancellation can arrive too late to prevent publication; the displayed result follows the final server outcome. A quiet Global Message tells you when the saved Authentic changes after creation; nothing regenerates automatically.

The viewer shows text and generated photos in order. Photos load as needed; an unavailable photo shows its description and Retry. A failed refresh keeps the previous text and releases its photos; reconnect or retry the refresh before loading them again. The viewer is read-only: no save or Paragraph Review. Use the separate selected-date Upload Illustrated Diary to Notion action for a published entry. AI Chat uses the working day’s open Authentic buffer when an Illustrated tab is active; if that buffer is closed, it shows No document. Reading links can be selected/copied; the viewer does not open them automatically.

To delete, open an Illustrated tab and choose **Delete Illustrated diary**. The dialog starts on Cancel. Confirm to remove the displayed Markdown and its photo directory; Authentic is kept. If the version changed, refresh and confirm the current version again. Close an Illustrated tab to release its loaded photos without deleting the saved diary. Closing stays available while a refresh is waiting for the server.

If admission is uncertain, use **Retry Illustrated admission**. It checks the original request identity; starting a new request is held until that outcome or a new server session is reconciled. No live AI or GPU call is needed for the repository’s synthetic smoke tests.

## Upload an Illustrated diary to Notion

Select the published diary's date and use **Upload Illustrated Diary to Notion** in the toolbar or command palette. Any editor tab may be active. Upload needs a connection, writable storage and a configured Illustrated Notion root; it needs no saved Authentic diary, open viewer, AI profile or image engine.

Private configuration uses `notion.token` with `notion.pages`, a nonempty list of `type: normal` and/or `type: illustrated` entries, each with `root_page_id`. The roots must be distinct. `normal` retains Authentic exports. Legacy `notion.root_page_id` is rejected with a migration message naming `notion.pages`; edit the private config explicitly using the [example](global_config_example.yaml), then restart the server. Reload does not replace active Notion adapters.

The upload contains rendered text and photos, with no prompts, seeds or front matter. Notion pages are ordered as `YYYY / YYYY-MM / YYYY-MM-DD (WED)`. Upload copies are JPEGs, exactly 480 pixels high with proportional width, quality 85 and white transparency background. Original PNGs and Markdown remain unchanged. Preparation is limited to 99 photos, 5 MiB per JPEG and 64 MiB total; invalid or oversized preparation stops before remote writes.

An unchanged upload performs no writes or photo uploads. Changed local content replaces the same day page and keeps its URL. The app reads and hashes hosted image bytes to detect replacements even when Notion's edit timestamp does not change. Externally changed, deleted or unowned destinations require a fresh confirmation. Duplicate/disordered dated pages or unsupported foreign blocks require repair in Notion before retrying.

Global Messages identify the captured date and upload steps. You may navigate while it runs; completion does not change your date or open Notion. **Stop upload** stops before another admitted request; it does not roll back created pages, uploaded files or partial content. New content is verified before old body blocks move to Notion trash. Replacement is not atomic, and an edit racing a Notion request cannot be prevented completely. Unknown outcomes retain their reservation until resolved; recovery observes without replaying writes. Confirmed abandonment stops tracking and does not undo remote effects. Deleting a local Illustrated diary never deletes its Notion page.

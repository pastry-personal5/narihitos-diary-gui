# UX terms

Status: Active

This is the canonical vocabulary for visible user interface components, and their code-facing names. Use the exact component terms in UI text and accessibility-visible names; use the mapped lower-camel or snake-case names in implementation identifiers when a component is represented there.

## Canonical components

| Visible term        | Code-facing name     | Meaning                                                                                                                                                                                |
| ------------------- | -------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Standard Layout     | `standardLayout`     | The whole application window arrangement                                                                                                                                               |
| Main Toolbar Strip  | `mainToolbarStrip`   | App title, distinct custom bitmap icon buttons (named with tooltips)                                                                                                                   |
| Main Content        | `mainContent`        | Region below the Main Toolbar Strip: Icon Bar, Explorer, Tabbed Editor and the AI column                                                                                               |
| Icon Bar            | `iconBar`            | Left strip of flat icon-only buttons (no visible label), including Settings                                                                                                            |
| Explorer            | `explorer`           | Left pane containing the four navigation tabs                                                                                                                                          |
| Date tab            | `dateTab`            | Civil-date selection                                                                                                                                                                   |
| Snippet tab         | `snippetTab`         | Source-grouped snippet hierarchy and inclusion checkboxes                                                                                                                              |
| Draft tab           | `draftTab`           | Authentic access and the transient generated candidates for the selected date; choosing one activates its Editor tab                                                                   |
| Entry tab           | `entryTab`           | Saved Authentic files by date                                                                                                                                                          |
| Tabbed Editor       | `tabbedEditor`       | Body-only Markdown editor, read-only Candidate tabs and review views                                                                                                                   |
| Global Messages     | `globalMessages`     | Pane below the Tabbed Editor listing notices and global messages to the user, oldest first, newest at the bottom                                                                       |
| Message severity    | `messageSeverity`    | Visual level of a global message: info, warning or error, shown by color plus an icon and text label                                                                                   |
| Editor tab          | `editorTab`          | One open buffer (source, Authentic or read-only Candidate) in the Tabbed Editor; distinct from the Explorer tabs                                                                       |
| AI Agents           | `aiAgents`           | Configured AI providers used by AI Chat, Generate and Paragraph Review; also the Settings category for their profiles                                                                  |
| AI Agent            | `aiAgent`            | One configured AI provider, such as Codex, Claude Code, Qwen Code or Apple Foundation Models                                                                                           |
| AI Chat             | `aiChat`             | Transient conversation and composer; fills the AI column                                                                                                                               |
| Settings            | `settings`           | Redacted configuration/capability status in cards; server config remains private YAML                                                                                                  |
| Settings sidebar    | `settingsSidebar`    | Category list inside Settings: Server, Status, Sources, AI Agents, Writing Language, Generation, Status Line and Keyboard Shortcuts, under a Go back button                            |
| Writing Language    | `writingLanguage`    | Persistent AI prose locale, initially the Mac's preferred language                                                                                                                     |
| Keyboard Shortcuts  | `keyboardShortcuts`  | Settings category: table of Action, Original shortcuts, Current shortcuts, Info with editing                                                                                           |
| Command palette     | `commandPalette`     | Search overlay with ranked commands, toggled by the toolbar button or opened by its shortcut                                                                                           |
| Authentic           | `authentic`          | The one autosaved managed Diary file for a date, shared by Draft/Entry                                                                                                                 |
| Candidate           | `candidate`          | A transient suggestion shown in a read-only Editor tab that can be reviewed against Authentic                                                                                          |
| Paragraph review    | `paragraphReview`    | Candidate review of paragraph or indivisible Markdown structure changes against saved Authentic                                                                                        |
| Review controls     | `reviewControls`     | Per-unit Accept/Reject controls shown during Paragraph review                                                                                                                          |
| Create Empty        | `createEmpty`        | Explicitly save an empty Authentic file for the selected date if it does not exist                                                                                                     |
| AI Activities       | `aiActivity`         | Toolbar button and modal dialog: run details, guidance, Cancel and result access                                                                                                       |
| Connection status   | `connectionStatus`   | Local server connection indicator in Settings > Server                                                                                                                                 |
| Save status         | `saveStatus`         | Dirty/saving/saved/failed/conflict feedback in Global Messages, based on the acknowledged revision                                                                                     |
| Profile selector    | `profileSelector`    | Selection control inside the Chat profile, Generation profile or Paragraph Review profile card in Settings > AI Agents                                                                 |
| Metadata warning    | `metadataWarning`    | Per-snippet notice of parse, availability or timestamp problems, with the explicit header/filename authority choice                                                                    |
| Snippet checkbox    | `snippetCheckbox`    | Generation-inclusion toggle in the Snippet tab; independent of snippet focus                                                                                                           |
| Pane separator      | `paneSeparator`      | Resizable, keyboard-operable boundary between Explorer, Tabbed Editor, AI Chat and Global Messages (the last resizes height)                                                           |
| Trash Recovery      | `trashRecovery`      | Main Toolbar Strip button and modal dialog for saved Trash results, unfinished moves and recovery guidance; separate from Global Messages                                              |
| Completion message  | `completionMessage`  | One polite in-app message per AI completion, with View results; posted to Global Messages                                                                                              |
| Results badge       | `resultsBadge`       | Marker in the AI Activities dialog and bottom status line that completed AI results await, until opened or dismissed                                                                   |
| Confirmation dialog | `confirmationDialog` | Explicit confirmation for moving a snippet to Trash, discarding edits or replacing export output                                                                                       |
| Settings card       | `settingsCard`       | Titled group in a Settings category (Server, Sources, Agent profiles, Chat profile, Generation profile, Paragraph Review profile, Advanced, Generation, Writing Language, Status Line) |
| Profile table       | `profileTable`       | Agent profiles with status SF Symbol, reason and per-row Check; refreshed by background checks                                                                                         |
| Advanced card       | `advancedCard`       | Settings card listing what currently blocks Chat, Generate or saving                                                                                                                   |
| Export preview      | `exportPreview`      | Fresh preview shown before replacing externally changed iCloud/Notion output                                                                                                           |

Casing: named regions use Title Case (Standard Layout, Icon Bar, Tabbed Editor, Main Content, AI Activities); sub-parts and controls use sentence case (Date tab, Editor tab, Settings sidebar, Settings card, Command palette). "Tab" alone is ambiguous: say Explorer tab (Date, Snippet, Draft, Entry) or Editor tab.

Draft and Entry are navigation terms, not two persisted asset kinds. Use these names for control labels and accessibility; use ordinary action verbs such as Save, Generate, Export, Accept and Reject for commands.

AI Chat context (`aiChatContext`) is the thin-bordered box at the top left of the AI Chat composer: a decorative bitmap icon followed by the document label, including unsaved-edit guidance or No document. It identifies the context attached to the next message.

**Paragraph Review** (`paragraphReview`) is the toolbar action for an AI suggestion on the caret paragraph in Authentic. Its suggestion uses Accept/Reject and normal editor undo/autosave. **Paragraph review** (`paragraphReview`) names the existing Candidate review mode against a saved Authentic revision. These are distinct workflows; see [GUI behavior](ux-gui.md#caret-paragraph-review).

**Status line** (`statusLine`) is the persistent footer for AI activity, unread results and optional readiness labels. **Status Line** is the Settings category controlling those labels.

## AI terminology

Use **AI Agents** for the configured providers collectively and for the Settings category; use **AI Agent** for one provider. Do not use “assistant” or “AI assistant” as user-facing names for them. AI Chat names the conversation surface, AI Activities names the activity dialog, and Chat profile, Generation profile, Paragraph Review profile and Agent profiles retain their distinct card names. AI Chat labels completed replies **AI Agent** and streaming replies **AI Agent (provisional)**; reply announcements also use AI Agent. Code/API transcript roles such as `assistant` are technical schema values, not canonical visible component names.

## Trash Recovery controls

Trash Recovery names both the toolbar action and the dialog it opens. Use the exact control names below; their behavior is defined in the [Trash Recovery contract](ux-gui.md#trash-recovery).

| Visible term            | Code-facing name        | Meaning                                                                                        |
| ----------------------- | ----------------------- | ---------------------------------------------------------------------------------------------- |
| Current situation       | `recoveryStatus`        | Section explaining connection, discovery and available saved moves                             |
| Check unfinished moves  | `checkUnfinishedMoves`  | Refresh saved unfinished operations without initiating a move                                  |
| Next unfinished moves   | `nextUnfinishedMoves`   | Load the next page of saved unfinished operations                                              |
| Inspect                 | `inspectTrashMove`      | Open a saved move's file-by-file result                                                        |
| Check again             | `checkTrashMoveAgain`   | Retry a failed saved-result request                                                            |
| Check saved result      | `checkSavedTrashResult` | Refresh the operation shown in the result dialog                                               |
| Preview remaining files | `previewRemainingFiles` | Obtain a fresh preview before explicitly confirming eligible remaining moves                   |
| Dismiss                 | `dismissCompletedMove`  | Remove a completed move from the visible list without restoring files                          |
| Close Trash Recovery    | `closeTrashRecovery`    | Accessible name and tooltip of the top-right Close button; returns focus to the toolbar action |

“No unfinished moves.” is an empty-state message, not an action. “Move to Trash” labels the operation list within Trash Recovery. Recovery controls do not belong in Global Messages.

## Settings categories and cards

Use these exact category names in the Settings sidebar, in this order. A category contains cards; it is distinct from an Explorer tab or an Editor tab.

| Category           | Code-facing name     | Contents                                                                            |
| ------------------ | -------------------- | ----------------------------------------------------------------------------------- |
| Server             | `serverSettings`     | Connection status, Connect and the Server configuration status/reload card          |
| Status             | `statusSettings`     | Advanced card; initial page when Settings opens                                     |
| Sources            | `sourcesSettings`    | Sources availability and Server configuration folder reveal cards                   |
| AI Agents          | `aiAgentsSettings`   | Chat profile, Generation profile, Paragraph Review profile and Agent profiles cards |
| Writing Language   | `writingLanguage`    | Saved language and language editing                                                 |
| Generation         | `generationSettings` | Candidates per generation                                                           |
| Status Line        | `statusLine`         | Saved visibility toggles for Generate ready, Review ready and Chat ready            |
| Keyboard Shortcuts | `keyboardShortcuts`  | Shortcut table and binding editing                                                  |

| Visible term                          | Code-facing name            | Meaning                                                                                                                 |
| ------------------------------------- | --------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| Chat profile                          | `chatProfile`               | Card in AI Agents containing the profile selector for AI Chat                                                           |
| Generation profile                    | `generationProfile`         | Card in AI Agents containing the profile selector for Generate; distinct from the Generation category's candidate count |
| Paragraph Review profile              | `paragraphReviewProfile`    | Independent profile selector for caret paragraph review                                                                 |
| Agent profiles                        | `agentProfiles`             | Card in AI Agents containing the Profile table of configured agents and their availability                              |
| Server configuration                  | `serverConfiguration`       | Card in Sources containing the action to reveal the active configuration folder                                         |
| Reveal configuration folder in Finder | `revealConfigurationFolder` | Open the folder containing the active server configuration file in macOS Finder                                         |

Profile choices are configured in Settings > AI Agents. AI Activities in the Main Toolbar Strip opens run status and result access.

## Candidate style terms

Each AI-generated Candidate has a **Style label** and **Style description** pair describing the writing style requested by its generation prompt.

| Visible term      | Code-facing name   | Meaning                                                                                      |
| ----------------- | ------------------ | -------------------------------------------------------------------------------------------- |
| Style label       | `styleLabel`       | A few words naming the requested writing style or approach, such as concise or chronological |
| Style description | `styleDescription` | A short explanation of how the prompt asks the AI Agent to write                             |

Example pair: (**reflective**, **dwell on feelings and thoughts the facts imply, without adding events.**)

These terms describe the prompt's requested style, rather than the Candidate's subject or a summary of its content. Use **Style label** and **Style description** as the canonical names instead of title, tagline or request summary for this pair. This records terminology only; it does not change generation behavior or define uniqueness enforcement.

## Implementation map

Views are grouped under `src/components/{standard-layout,explorer,tabbed-editor,
paragraph-review,global-messages,ai-chat,ai-activities,ai-agents,settings,
trash-recovery,export}`. Matching state/use cases live under `src/application`;
bridge validation remains under `src/adapters`, and CodeMirror integration under
`src/editor`. Main Content, Tabbed Editor and AI Chat use the matching
`main-content`, `tabbed-editor` and `ai-chat` CSS selectors. Editor tabs and
Explorer tabs name their accessible groups; Review controls names the React
control group. The technical `cm-review-bar` is a decorative gutter marker.
Transport jobs, HTTP sessions, transcript role `assistant`, source identities,
provider kinds and generation styles retain their distinct technical meanings.

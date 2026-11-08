# UX terms

Status: Active

This is the canonical vocabulary for visible user interface components, and their code-facing names. Use the exact component terms in UI text and accessibility-visible names; use the mapped lower-camel or snake-case names in implementation identifiers when a component is represented there.

## Canonical components

| Visible term        | Code-facing name     | Meaning                                                                                                                                         |
| ------------------- | -------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| Standard Layout     | `standardLayout`     | The whole application window arrangement                                                                                                        |
| Main Toolbar Strip  | `mainToolbarStrip`   | App title, actions and Save status                                                                                                              |
| Main Content        | `mainContent`        | Region below the Main Toolbar Strip: Icon Bar, Explorer, Tabbed Editor and the AI column                                                        |
| Icon Bar            | `iconBar`            | Left strip of flat icon-only buttons (no visible label), including Settings                                                                     |
| Explorer            | `explorer`           | Left pane containing the four navigation tabs                                                                                                   |
| Date tab            | `dateTab`            | Civil-date selection                                                                                                                            |
| Snippet tab         | `snippetTab`         | Source-grouped snippet hierarchy and inclusion checkboxes                                                                                       |
| Draft tab           | `draftTab`           | Authentic access and the transient generated candidates for the selected date; choosing one activates its Editor tab                            |
| Entry tab           | `entryTab`           | Saved Authentic files by date                                                                                                                   |
| Tabbed Editor       | `tabbedEditor`       | Body-only Markdown editor, read-only Candidate tabs and review views                                                                            |
| Global Messages     | `globalMessages`     | Pane below the Tabbed Editor listing notices and global messages to the user, oldest first, newest at the bottom                                |
| Message severity    | `messageSeverity`    | Visual level of a global message: info, success, warning or error, shown by color plus an icon and text label                                   |
| Editor tab          | `editorTab`          | One open buffer (source, Authentic or read-only Candidate) in the Tabbed Editor; distinct from the Explorer tabs                                |
| AI Chat             | `aiChat`             | Transient conversation and composer; sits below the AI Activity widget                                                                          |
| Settings            | `settings`           | Redacted configuration/capability status in cards; server config remains private YAML                                                           |
| Settings sidebar    | `settingsSidebar`    | Category list inside Settings: Status, Server, Sources, AI Agents, Writing Language, Keyboard Shortcuts and Generation, under a Go back button  |
| Writing Language    | `writingLanguage`    | Persistent AI prose locale, initially the Mac's preferred language                                                                              |
| Keyboard Shortcuts  | `keyboardShortcuts`  | Settings category: table of Action, Original shortcuts, Current shortcuts, Info with editing                                                    |
| Command palette     | `commandPalette`     | Searchable list of actions, opened by its shortcut                                                                                              |
| Authentic           | `authentic`          | The one autosaved managed Diary file for a date, shared by Draft/Entry                                                                          |
| Candidate           | `candidate`          | A transient suggestion shown in a read-only Editor tab that can be reviewed against Authentic                                                   |
| Paragraph review    | `paragraphReview`    | Review mode for paragraph or indivisible Markdown structure changes                                                                             |
| Review controls     | `reviewControls`     | Per-unit Accept/Reject controls shown during Paragraph review                                                                                   |
| Create Empty        | `createEmpty`        | Explicitly save an empty Authentic file for the selected date if it does not exist                                                              |
| AI Activity         | `aiActivity`         | Widget above AI Chat: run summary, expandable run details, Cancel and result access                                                             |
| Connection status   | `connectionStatus`   | Local server connection indicator in Settings > Server                                                                                          |
| Save status         | `saveStatus`         | Dirty/saving/saved/failed/conflict indicator based on the acknowledged revision                                                                 |
| Profile selector    | `profileSelector`    | Selection control inside the Chat profile or Generation profile card in Settings > AI Agents                                                    |
| Metadata warning    | `metadataWarning`    | Per-snippet notice of parse, availability or timestamp problems, with the explicit header/filename authority choice                             |
| Snippet checkbox    | `snippetCheckbox`    | Generation-inclusion toggle in the Snippet tab; independent of snippet focus                                                                    |
| Pane separator      | `paneSeparator`      | Resizable, keyboard-operable boundary between Explorer, Tabbed Editor, AI Chat and Global Messages (the last resizes height)                    |
| Completion message  | `completionMessage`  | One polite in-app message per AI completion, with View results; posted to Global Messages                                                       |
| Results badge       | `resultsBadge`       | Marker in the AI Activity widget that completed AI results await, until opened or dismissed                                                     |
| Confirmation dialog | `confirmationDialog` | Explicit confirmation for snippet delete, discard and export replacement                                                                        |
| Settings card       | `settingsCard`       | Titled group in a Settings category (Server, Sources, Agent profiles, Chat profile, Generation profile, Advanced, Generation, Writing Language) |
| Profile table       | `profileTable`       | Agent profiles with status SF Symbol, reason and per-row Check; refreshed by background checks                                                  |
| Advanced card       | `advancedCard`       | Settings card listing what currently blocks Chat, Generate or saving                                                                            |
| Export preview      | `exportPreview`      | Fresh preview shown before replacing externally changed iCloud/Notion output                                                                    |

Casing: named regions use Title Case (Standard Layout, Icon Bar, Tabbed Editor, Main Content, AI Activity); sub-parts and controls use sentence case (Date tab, Editor tab, Settings sidebar, Settings card, Command palette). "Tab" alone is ambiguous: say Explorer tab (Date, Snippet, Draft, Entry) or Editor tab.

Draft and Entry are navigation terms, not two persisted asset kinds. Use these names for control labels and accessibility; use ordinary action verbs such as Save, Generate, Export, Accept and Reject for commands.

## Settings categories and cards

Use these exact category names in the Settings sidebar, in this order. A category contains cards; it is distinct from an Explorer tab or an Editor tab.

| Category           | Code-facing name     | Contents                                                                              |
| ------------------ | -------------------- | ------------------------------------------------------------------------------------- |
| Status             | `statusSettings`     | Advanced card                                                                         |
| Server             | `serverSettings`     | Connection status, Connect and the Server card with configuration status/reload       |
| Sources            | `sourcesSettings`    | Sources availability card and Server configuration card with the Finder reveal action |
| AI Agents          | `aiAgentsSettings`   | Chat profile, Generation profile and Agent profiles cards                             |
| Writing Language   | `writingLanguage`    | Saved language and language editing                                                   |
| Keyboard Shortcuts | `keyboardShortcuts`  | Shortcut table and binding editing                                                    |
| Generation         | `generationSettings` | Candidates per generation                                                             |

| Visible term                          | Code-facing name            | Meaning                                                                                                                 |
| ------------------------------------- | --------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| Chat profile                          | `chatProfile`               | Card in AI Agents containing the profile selector for AI Chat                                                           |
| Generation profile                    | `generationProfile`         | Card in AI Agents containing the profile selector for Generate; distinct from the Generation category's candidate count |
| Agent profiles                        | `agentProfiles`             | Card in AI Agents containing the Profile table of configured agents and their availability                              |
| Server configuration                  | `serverConfiguration`       | Card in Sources containing the action to reveal the active configuration folder                                         |
| Reveal configuration folder in Finder | `revealConfigurationFolder` | Open the folder containing the active server configuration file in macOS Finder                                         |

Profile choices are configured in Settings > AI Agents. AI Activity in Main contains run status and result access.

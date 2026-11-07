# UX terms

Status: Active

This is the canonical vocabulary for visible user interface components, and their code-facing names. Use the exact component terms in UI text and accessibility-visible names; use the mapped lower-camel or snake-case names in implementation identifiers when a component is represented there.

## Canonical components

| Visible term        | Code-facing name     | Meaning                                                                                                                      |
| ------------------- | -------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| Standard Layout     | `standardLayout`     | The whole application window arrangement                                                                                     |
| Main Toolbar Strip  | `mainToolbarStrip`   | App title, actions, Save status and Connection status                                                                        |
| Main Content        | `mainContent`        | Region below the Main Toolbar Strip: Icon Bar, Explorer, Tabbed Editor and the AI column                                     |
| Icon Bar            | `iconBar`            | Left strip of flat icon-only buttons (no visible label), including Settings                                                  |
| Explorer            | `explorer`           | Left pane containing the four navigation tabs                                                                                |
| Date tab            | `dateTab`            | Civil-date selection                                                                                                         |
| Snippet tab         | `snippetTab`         | Source-grouped snippet hierarchy and inclusion checkboxes                                                                    |
| Draft tab           | `draftTab`           | Authentic access and the transient generated candidates for the selected date; choosing one activates its Editor tab         |
| Entry tab           | `entryTab`           | Saved Authentic files by date                                                                                                |
| Tabbed Editor       | `tabbedEditor`       | Body-only Markdown editor, read-only Candidate tabs and review views                                                         |
| Global Messages     | `globalMessages`     | Pane below the Tabbed Editor listing notices and global messages to the user, oldest first, newest at the bottom             |
| Message severity    | `messageSeverity`    | Visual level of a global message: info, success, warning or error, shown by color plus an icon and text label                |
| Editor tab          | `editorTab`          | One open buffer (source, Authentic or read-only Candidate) in the Tabbed Editor; distinct from the Explorer tabs             |
| AI Chat             | `aiChat`             | Transient conversation and composer; sits below the AI Activity widget                                                       |
| Settings            | `settings`           | Redacted configuration/capability status in cards; server config remains private YAML                                        |
| Settings sidebar    | `settingsSidebar`    | Section list inside Settings: Status, Writing Language, Keyboard Shortcuts and Generation, under a Go back button            |
| Writing Language    | `writingLanguage`    | Persistent AI prose locale, initially the Mac's preferred language                                                           |
| Keyboard Shortcuts  | `keyboardShortcuts`  | Settings section: table of Action, Original shortcuts, Current shortcuts, Info with editing                                  |
| Command palette     | `commandPalette`     | Searchable list of actions, opened by its shortcut                                                                           |
| Authentic           | `authentic`          | The one autosaved managed Diary file for a date, shared by Draft/Entry                                                       |
| Candidate           | `candidate`          | A transient suggestion shown in a read-only Editor tab that can be reviewed against Authentic                                |
| Paragraph review    | `paragraphReview`    | Review mode for paragraph or indivisible Markdown structure changes                                                          |
| Review controls     | `reviewControls`     | Per-unit Accept/Reject controls shown during Paragraph review                                                                |
| Create Empty        | `createEmpty`        | Explicitly save an empty Authentic file for the selected date if it does not exist                                           |
| AI Activity         | `aiActivity`         | Widget above AI Chat: Profile selector, run summary, expandable run details, Cancel and result access                        |
| Connection status   | `connectionStatus`   | Local server connection indicator in the Main Toolbar Strip                                                                  |
| Save status         | `saveStatus`         | Dirty/saving/saved/failed/conflict indicator based on the acknowledged revision                                              |
| Profile selector    | `profileSelector`    | Controls at the top of the AI Activity widget for choosing the agent profile for Generate and AI Chat                        |
| Metadata warning    | `metadataWarning`    | Per-snippet notice of parse, availability or timestamp problems, with the explicit header/filename authority choice          |
| Snippet checkbox    | `snippetCheckbox`    | Generation-inclusion toggle in the Snippet tab; independent of snippet focus                                                 |
| Pane separator      | `paneSeparator`      | Resizable, keyboard-operable boundary between Explorer, Tabbed Editor, AI Chat and Global Messages (the last resizes height) |
| Completion message  | `completionMessage`  | One polite in-app message per AI completion, with View results; posted to Global Messages                                    |
| Results badge       | `resultsBadge`       | Marker in the AI Activity widget that completed AI results await, until opened or dismissed                                  |
| Confirmation dialog | `confirmationDialog` | Explicit confirmation for snippet delete, discard and export replacement                                                     |
| Settings card       | `settingsCard`       | Titled group in a Settings section (Server, Sources, Agent profiles, Advanced, Generation, Writing Language)                 |
| Profile table       | `profileTable`       | Agent profiles with status SF Symbol, reason and per-row Check; refreshed by background checks                               |
| Advanced card       | `advancedCard`       | Settings card listing what currently blocks Chat, Generate or saving                                                         |
| Export preview      | `exportPreview`      | Fresh preview shown before replacing externally changed iCloud/Notion output                                                 |

Casing: named regions use Title Case (Standard Layout, Icon Bar, Tabbed Editor, Main Content, AI Activity); sub-parts and controls use sentence case (Date tab, Editor tab, Settings sidebar, Settings card, Command palette). "Tab" alone is ambiguous: say Explorer tab (Date, Snippet, Draft, Entry) or Editor tab.

Draft and Entry are navigation terms, not two persisted asset kinds. Use these names for control labels and accessibility; use ordinary action verbs such as Save, Generate, Export, Accept and Reject for commands.

# UX terms

Status: Active

This is the canonical vocabulary for visible user interface components, and their code-facing names. Use the exact component terms in UI text and accessibility-visible names; use the mapped lower-camel or snake-case names in implementation identifiers when a component is represented there.

## Canonical components

| Visible term        | Code-facing name     | Meaning                                                                                                |
| ------------------- | -------------------- | ------------------------------------------------------------------------------------------------------ |
| Standard Layout     | `standardLayout`     | The whole application window arrangement                                                               |
| Main Toolbar Strip  | `mainToolbarStrip`   | Date, actions, Connection status, Save status and AI activity                                          |
| Main Content        | `mainContent`        | Region below the Main Toolbar Strip: Icon Bar, Explorer, Tabbed Editor and AI Chat                     |
| Icon Bar            | `iconBar`            | Left navigation strip, including Settings                                                              |
| Explorer            | `explorer`           | Left pane containing the four navigation tabs                                                          |
| Date tab            | `dateTab`            | Civil-date selection                                                                                   |
| Snippet tab         | `snippetTab`         | Source-grouped snippet hierarchy and inclusion checkboxes                                              |
| Draft tab           | `draftTab`           | Authentic access and transient generated candidates for the selected date                              |
| Entry tab           | `entryTab`           | Saved Authentic files by date                                                                          |
| Tabbed Editor       | `tabbedEditor`       | Body-only Markdown editor and candidate/review views                                                   |
| Editor tab          | `editorTab`          | One open buffer (source, Authentic or Candidate) in the Tabbed Editor; distinct from the Explorer tabs |
| AI Chat             | `aiChat`             | Session/profile selection, transient conversation and run status                                       |
| Settings            | `settings`           | Redacted configuration/capability status; server config remains private YAML                           |
| Settings sidebar    | `settingsSidebar`    | Section list inside Settings: status, Writing Language and Keyboard Shortcuts                          |
| Writing Language    | `writingLanguage`    | Persistent AI prose locale, initially the Mac's preferred language                                     |
| Keyboard Shortcuts  | `keyboardShortcuts`  | Settings section: table of Action, Original shortcuts, Current shortcuts, Info with editing            |
| Command palette     | `commandPalette`     | Searchable list of actions, opened by its shortcut                                                     |
| Authentic           | `authentic`          | The one autosaved managed Diary file for a date, shared by Draft/Entry                                 |
| Candidate           | `candidate`          | A transient suggestion that can be reviewed against Authentic                                          |
| Paragraph review    | `paragraphReview`    | Review mode for paragraph or indivisible Markdown structure changes                                    |
| Review controls     | `reviewControls`     | Per-unit Accept/Reject controls shown during Paragraph review                                          |
| Create Empty        | `createEmpty`        | Explicitly save an empty Authentic file for the selected date if it does not exist                     |
| AI activity         | `aiActivity`         | Persistent Main Toolbar Strip summary, expandable run details, Cancel and result access                |
| Connection status   | `connectionStatus`   | Local server connection indicator in the Main Toolbar Strip                                            |
| Save status         | `saveStatus`         | Dirty/saving/saved/failed/conflict indicator based on the acknowledged revision                        |
| Profile selector    | `profileSelector`    | Control for choosing a configured agent profile for Generate and AI Chat                               |
| Snippet checkbox    | `snippetCheckbox`    | Generation-inclusion toggle in the Snippet tab; independent of snippet focus                           |
| Pane separator      | `paneSeparator`      | Resizable, keyboard-operable boundary between Explorer, Tabbed Editor and AI Chat                      |
| Completion message  | `completionMessage`  | One polite in-app message per AI completion, with View results                                         |
| Results badge       | `resultsBadge`       | Marker that completed AI results await, until opened or dismissed                                      |
| Confirmation dialog | `confirmationDialog` | Explicit confirmation for snippet delete, discard and export replacement                               |
| Export preview      | `exportPreview`      | Fresh preview shown before replacing externally changed iCloud/Notion output                           |

Casing: named regions use Title Case (Standard Layout, Icon Bar, Tabbed Editor, Main Content); sub-parts and controls use sentence case (Date tab, Editor tab, Settings sidebar, Command palette). "Tab" alone is ambiguous: say Explorer tab (Date, Snippet, Draft, Entry) or Editor tab.

Draft and Entry are navigation terms, not two persisted asset kinds. Use these names for control labels and accessibility; use ordinary action verbs such as Save, Generate, Export, Accept and Reject for commands.

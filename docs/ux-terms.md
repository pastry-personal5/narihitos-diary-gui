# UX terms

Status: Active

This is the canonical vocabulary for visible user interface components, and their code-facing names. Use the exact component terms in UI text and accessibility-visible names; use the mapped lower-camel or snake-case names in implementation identifiers when a component is represented there.

## Canonical components

| Visible term | Code-facing name | Meaning |
|---|---|---|
| Standard Layout | `standardLayout` | The whole application window arrangement |
| Main Toolbar Strip | `mainToolbarStrip` | Date, actions and connection/save/job status |
| Icon Bar | `iconBar` | Left navigation strip, including Settings |
| Explorer | `explorer` | Left pane containing the four navigation tabs |
| Date tab | `dateTab` | Civil-date selection |
| Snippet tab | `snippetTab` | Source-grouped snippet hierarchy and inclusion checkboxes |
| Draft tab | `draftTab` | Authentic access and transient generated candidates for the selected date |
| Entry tab | `entryTab` | Saved Authentic files by date |
| Tabbed Editor | `tabbedEditor` | Body-only Markdown editor and candidate/review views |
| AI Chat | `aiChat` | Session/profile selection, transient conversation and run status |
| Settings | `settings` | Redacted configuration/capability status; server config remains private YAML |
| Settings sidebar | `settingsSidebar` | Section list inside Settings: status, Writing Language and Keyboard Shortcuts |
| Writing Language | `writingLanguage` | Persistent AI prose locale, initially the Mac's preferred language |
| Keyboard Shortcuts | `keyboardShortcuts` | Settings section: table of Action, Original shortcuts, Current shortcuts, Info with editing |
| Command palette | `commandPalette` | Searchable list of actions, opened by its shortcut |
| Authentic | `authentic` | The one autosaved managed Diary file for a date, shared by Draft/Entry |
| Candidate | `candidate` | A transient suggestion that can be reviewed against Authentic |
| Paragraph review | `paragraphReview` | Accept/reject controls for paragraph or indivisible Markdown structure changes |
| Create Empty | `createEmpty` | Explicitly save an empty Authentic file for the selected date if it does not exist |
| AI activity | `aiActivity` | Persistent Main Toolbar Strip summary, expandable run details, Cancel and result access |

Draft and Entry are navigation terms, not two persisted asset kinds. Use these names for control labels and accessibility; use ordinary action verbs such as Save, Generate, Export, Accept and Reject for commands.

# Frontend and licensing findings

Status: Active

Research date: 2026-11-05. Covers R7.1–R7.8 and R10.1–R10.3 as implementation options and evidence gaps. The stack stays React/TypeScript + CodeMirror 6; no component, testing, or icon dependency is adopted here.

## Editor state (R7.1)

The CodeMirror guide/examples endpoints returned 403 during this research, so the fallback evidence is upstream [EditorState source](https://github.com/codemirror/state/blob/main/src/state.ts), [EditorView source](https://github.com/codemirror/view/blob/main/src/editorview.ts), and [Markdown language package](https://github.com/codemirror/lang-markdown), fetched on the research date from their moving default branches. Pin chosen package versions in P1-M3; these links do not establish a tested version set.

Recommendation: a dedicated editor module owns extensions, keymaps, Markdown language support, and a view lifecycle. Keep an independent EditorState per asset tab so selection/undo survive switching. Compare one view with state switching against retained views per tab: the former limits active DOM work, the latter can preserve view-specific state but consumes resources. Destroy removed views and handle React mount/cleanup correctly. Use transactions for edits instead of recreating a view or replacing the whole document on each React render. Track document revision vs. persisted revision, not merely whether any edit event occurred. Authentic draft and Entry share one autosaved file/revision stream. Opening the same file from different Explorer tabs must not create competing editor/save buffers; source Snippet write-back needs its own contract.

## Layout and controls (R7.2–R7.5)

The owner describes Main Toolbar Strip, icon sidebar with Settings, Explorer tabs (Date, Snippet, Draft, Entry), Tabbed Editor, and AI Chat. The canonical UX/vocabulary/containment docs are still `TBD`; record these as owner-idea terms until their canonical definitions are approved. No visual design is settled by calling the app VS Code-like.

| Area             | Compare                                                                                                   | Recommendation for P1-M2                                                                                                                                                                                          |
| ---------------- | --------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Resizable panes  | CSS Grid + custom separators; [react-resizable-panels](https://github.com/bvaughn/react-resizable-panels) | Library can reduce interaction work; check current API and keyboard resize behavior before approval. Custom CSS still needs pointer/keyboard/focus handling.                                                      |
| Command palette  | React dialog/listbox implementation; existing headless primitive library                                  | Use one command registry shared with toolbar/shortcuts. Palette focus returns to its caller; Escape closes; do not add a library solely because the design resembles VS Code.                                     |
| Date calendar    | Custom civil-date grid; [React DayPicker accessibility](https://daypicker.dev/guides/accessibility)       | Calendar library offers keyboard/focus conventions; custom grid avoids a dependency but carries substantial accessibility/date testing work. Ensure selected dates do not shift through implicit UTC conversions. |
| Snippet explorer | Grouped lists with native checkboxes; full ARIA tree                                                      | Use tree semantics only if hierarchical keyboard interactions are implemented. Focus opens a snippet without toggling inclusion; checkbox state remains independent.                                              |
| Shared state     | React reducer/context; external store                                                                     | Start comparison with a reducer and explicit asset/job IDs; do not use CodeMirror state or streamed text as a global React keystroke mirror. Add a store only for demonstrated cross-pane/subscription needs.     |

The [WAI tree pattern](https://www.w3.org/WAI/ARIA/apg/patterns/treeview/) distinguishes focus from multiselection and specifies navigation semantics. Source grouping alone does not require the complexity of a true tree widget. Native buttons/checkboxes with labels may serve the v1 hierarchy more simply; owner UX confirmation remains necessary.

Cmd+Shift+P is an app-local palette shortcut; compare DOM/keymap handling with a native menu accelerator. A global shortcut plugin is only needed for an OS-wide shortcut, which the owner did not request. Resolve shortcut ownership centrally so CodeMirror and the app do not both act on the same key. Respect IME composition and text-editing shortcuts.

## Validation and gaps (R7.6–R7.8)

Use Vitest for selection-vs-focus, command dispatch, tab state preservation, autosave acknowledgements/errors, candidate adoption guards, and stale stream cancellation. DOM mocks cannot prove layout, native IME, clipboard, VoiceOver, or WKWebView behavior. Test those in the packaged app with keyboard-only navigation, visible focus, labeled icon buttons, pane resize, high contrast/text scaling, and focus restoration. DayPicker's own guidance recommends screen-reader testing; library adoption is not proof of accessibility.

Proposed workload scenarios: many snippets across multiple sources, a long Markdown document, many open tabs, concurrent agent deltas, and rapid date switches during autosave. Record render/save/stream latency and memory before setting thresholds; no benchmark was run. Virtualization is an option if measurement justifies it, with focus/selection persistence as acceptance criteria.

R7.7 result: the ideas cannot be checked against substantive canonical UX docs yet. P1-M2 must resolve navigation, dirty/close rules, adoption behavior, and Authentic-file autosave failure behavior and then fill the canonical docs from owner decisions; this findings doc does not replace them.

## Icons and dependency licensing (R10)

[Apple's SF Symbols page](https://developer.apple.com/sf-symbols/) links to the [official disk image](https://devimages-cdn.apple.com/design/resources/download/SF-Symbols-27.dmg?2=). Downloaded it to temporary storage, mounted it read-only without auto-open, expanded `SFSymbols.pkg` without installing, and converted `Resources/en.lproj/License.rtf` with `textutil` for inspection. No app/installer was run and no license acceptance was submitted.

Evidence fingerprints: disk image SHA-256 `bcf900f9efa96f61c3d470fdf7cb6d5a80b49bf645da44919eb9c1835f02046f`; English RTF SHA-256 `12fb2f79eea41813a3658f8eff47c34b769b1c296eaf232be7958be52e2f86d2`. The image was 445,151,232 bytes, downloaded on the research date; package metadata identified app version `27.2` build `150`, and the SF Symbols agreement was `EA1662`, dated `9/6/2019`. Reproduce with `curl` from that Apple URL, `hdiutil attach -readonly -nobrowse -noautoopen`, `pkgutil --expand` on the package, and `textutil -convert txt` on the English license. Inspecting the installer resources does not select a license for all OS-provided runtime symbols.

License findings: section 2A limits the symbols grant to Apple-platform UI mock-ups; 2B(iv) permits distribution of modified templates through Xcode; 2D limits other distribution and repackaging; 2E restricts logos/app icons/trademark uses. **Inference for this project:** these provisions do not establish permission for the proposed exported-bitmap bundle in Tauri's web assets or an Apache-2.0 asset repository. Bitmap conversion does not remove restrictions. Investigate OS-provided native symbol rendering or obtain clearer permission before bundling; neither alternative is approved here. Individual-symbol restrictions still require inspection after selecting a permitted route.

Proposed export workflow after clearance: choose only unrestricted permitted symbols, keep symbol/version/provenance records, render matching light/dark or template-style assets at the UI's logical size and required pixel scales, inspect contrast and Retina sharpness, and retain accessible text labels in the UI. App icon artwork is a separate deliverable and must not be inferred from toolbar symbols. No render/export is performed here.

R10.2 fallback: if terms do not allow the intended distribution, propose original repo-native icons or a permissively licensed set for owner approval before changing the SF Symbols requirement. No fallback has been chosen. R10.3 process: inspect exact-version SPDX expressions/license texts and transitive native/assets notices; record required attribution. [cargo-deny license checks](https://embarkstudios.github.io/cargo-deny/checks/licenses/index.html) can automate Rust policy checks but do not exhaustively identify every embedded license and do not cover npm/assets. Adding that tool requires approval.

Initial candidates have upstream MIT licenses: [CodeMirror view](https://github.com/codemirror/view/blob/main/LICENSE), [react-resizable-panels](https://github.com/bvaughn/react-resizable-panels/blob/main/LICENSE.md), and [React DayPicker](https://github.com/gpbl/react-day-picker/blob/main/LICENSE). Preserve their notices if adopted; verify the exact resolved dependency graph rather than treating this short inventory as blanket legal clearance.

P1-M2 implications: choose the tab/editor ownership model and minimal layout/calendar/state dependencies from accessibility and workload evidence. The inspected SF Symbols terms do not clear the requested bitmap workflow, so choose a permitted route with the owner before bundling; an unbranded scaffold can use temporary text labels. Product/UX decisions and packaged keyboard/IME/VoiceOver checks remain open.

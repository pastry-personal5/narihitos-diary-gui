# P1-M35: Illustrated diary GUI work plan

Status: Planned

Work plan for the [P1-M35 overview](milestone-35-illustrated-diary-gui-overview.md): a thin GUI over the P1-M34 routes that reuses the existing action, activity, tab and settings patterns.

## Design

- **Controller (`src/application/illustrated/`):** catalog of dates, create/replace/delete commands, snapshot for components; follows the `authentic` and `generation` controllers.
- **Action:** one entry in the shared action decisions (`src/application/actions`) so the toolbar, palette and disabled reasons come from one place.
- **Progress:** no new surface; creation is one more activity kind in AI Activities with the existing Cancel.
- **Viewer:** a read-only resource kind in the tabbed editor. Rendering per overview Q2; photos through a bridge command returning bytes, shown as object URLs (overview Q3).
- **Settings:** one card modelled on the AI Agents profile table (`ProfileTable`, check delay logic reused).

## Existing code to reuse

- `src/application/actions`, `navigation` (`EditorResource`, `ResourceCoordinator`), `activity`, `ai-activities`.
- `src/components/explorer` tables, `tabbed-editor`, `ModalDialog`, `Bitmap`, `settings`, `global-messages`.
- Tauri bridge command pattern from P1-M21 (`bridge/chat.rs`).

## Stages and tasks

### Stage A - controller and Explorer (T01-T03)

- T01 - Bridge commands and validated adapters.
- T02 - Controller and catalog.
- T03 - Explorer indicator and open.

### Stage B - create (T04-T06)

- T04 - Action, disabled reasons, palette entry and shortcut-table entry.
- T05 - Activity kind with step progress and cancel.
- T06 - Replace confirmation and failure messages.

### Stage C - view and delete (T07-T09)

- T07 - Read-only resource kind and viewer.
- T08 - Photo loading and cleanup.
- T09 - Delete with confirmation.

### Stage D - settings, docs and proof (T10-T13)

- T10 - Image engines Settings card.
- T11 - UX terms and product docs.
- T12 - Native smoke leg.
- T13 - Full gate.

## Risks and notes

- A Markdown renderer would be a new dependency and a new HTML-injection surface; the no-dependency CodeMirror route avoids both.
- Several multi-megabyte photos per tab: load lazily and revoke object URLs, and count them against the existing session budget if it applies.
- P1-M32 release evidence was collected before this feature; P1-M32 must re-run the affected automated checks after P1-M35 and its release note needs a line for the feature.

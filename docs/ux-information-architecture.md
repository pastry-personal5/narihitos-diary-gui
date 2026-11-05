# UX information architecture

Status: Active

The Standard Layout is the conceptual containment model for the app. It records structure only; visual and interaction rules are in [ux-gui.md](ux-gui.md).

```mermaid
flowchart TB
  W[Standard Layout] --> T[Main Toolbar Strip]
  T --> CS[Connection status]
  T --> SS[Save status]
  T --> AI[AI activity]
  W --> B[Main Content]
  B --> I[Icon Bar]
  B --> E[Explorer]
  B --> C[Tabbed Editor]
  B --> A[AI Chat]
  C --> ET[Editor tab]
  C -. Pane separator .- G[Global Messages]
  A --> PS[Profile selector]
  B -. between panes .- SP[Pane separator]
  I --> S[Settings]
  N --> MW[Metadata warning]
  E --> D[Date tab]
  E --> N[Snippet tab]
  E --> F[Draft tab]
  E --> R[Entry tab]
```

The Main Toolbar Strip spans the top; Main Content below it runs left to right: Icon Bar, Explorer, Tabbed Editor, AI Chat. The toolbar includes Create Empty for manual creation on the selected date and an AI activity summary with expandable details/results access. This detail surface adds no permanent pane or blocking progress window. Global Messages sits below the Tabbed Editor, in the editor column only, with a horizontal Pane separator between them that resizes its height. Pane separators sit between Explorer, Tabbed Editor and AI Chat. Settings opens a labeled panel within the same local window, with a sidebar of sections (status, Writing Language, Keyboard Shortcuts); it does not expose private config contents.

Date selects the working civil date; Snippet groups views of originals by source and local date. Draft lists Authentic access and transient candidates for that date, including results completed in the background (flagged by the Results badge) after navigating away; Entry lists saved Authentic dates, including explicitly created empty files. Both open the same Authentic resource/buffer in the Tabbed Editor. Opening a snippet does not change generation inclusion. AI Chat uses a snapshot of the active editor on each send. The containment model introduces no preview pane, financial workflow, separate saved draft, or history view.

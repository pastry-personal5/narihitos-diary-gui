import type {
  ParagraphReviewer,
  ParagraphReviewContext,
  ParagraphReviewState,
} from "./contracts";

const EMPTY_CONTEXT: ParagraphReviewContext = {
  editor: null,
  profile: null,
  locale: null,
  blocked: true,
  context: async () => [],
};
/** Session-owned use case. No React, bridge/chat DTOs, timers, DOM or CodeMirror. */
export class ParagraphReviewController {
  private context = EMPTY_CONTEXT;
  private listeners = new Set<() => void>();
  private operation: AbortController | null = null;
  private disposed = false;
  state: ParagraphReviewState = {
    phase: "idle",
    reason: "Focus a Draft in the Tabbed Editor.",
  };
  constructor(
    private reviewer: ParagraphReviewer,
    private notify: (
      text: string,
      severity: "info" | "warning" | "error",
    ) => void,
  ) {}
  subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  };
  getSnapshot = () => this.state;
  private publish(phase = this.state.phase) {
    const { editor, profile, locale, blocked } = this.context;
    const reason = !editor
      ? "Focus a Draft in the Tabbed Editor."
      : blocked
        ? "Wait for the active AI request or review to finish."
        : !profile || !locale
          ? "Choose a ready Paragraph Review profile and Writing Language in Settings."
          : !this.reviewer.available
            ? "Paragraph review is unavailable."
            : phase === "reviewing"
              ? "Reviewing paragraph…"
              : phase === "suggestion"
                ? "Accept or reject the current suggestion first."
                : null;
    if (phase === this.state.phase && reason === this.state.reason) return;
    this.state = { phase, reason };
    for (const listener of this.listeners) listener();
  }
  configure(context: ParagraphReviewContext) {
    if (this.disposed) return;
    if (
      context.editor !== this.context.editor ||
      context.blocked ||
      context.profile?.alias !== this.context.profile?.alias ||
      context.profile?.type !== this.context.profile?.type ||
      context.locale !== this.context.locale
    )
      this.cancel();
    this.context = context;
    this.publish();
  }
  cancel() {
    this.operation?.abort();
    this.operation = null;
    this.context.editor?.clearParagraph();
    this.publish("idle");
  }
  async start(): Promise<void> {
    if (this.disposed || this.state.reason) return;
    const { editor, profile, locale, context } = this.context;
    if (!editor || !profile || !locale) return;
    const snapshot = editor.paragraph();
    if (!snapshot) {
      this.notify(
        "Place the caret inside a non-empty Draft paragraph.",
        "warning",
      );
      return;
    }
    if (new TextEncoder().encode(snapshot.text).length > 60 * 1024) {
      this.notify(
        "This paragraph is too large to review. Split it into smaller paragraphs first.",
        "warning",
      );
      return;
    }
    const operation = new AbortController();
    this.operation = operation;
    this.publish("reviewing");
    try {
      const snippets = context?.() ?? [];
      const text = (
        await this.reviewer.review(
          { text: snapshot.text, context: snippets, profile, locale },
          operation.signal,
        )
      ).trim();
      if (this.operation !== operation || operation.signal.aborted) return;
      if (!text || new TextEncoder().encode(text).length > 64 * 1024)
        throw new Error("invalid_reply");
      if (editor.getText() !== snapshot.body) {
        this.notify(
          "The Draft changed during review. Review the paragraph again.",
          "warning",
        );
        return;
      }
      if (text === snapshot.text) {
        this.notify("AI suggested no changes to this paragraph.", "info");
        return;
      }
      editor.suggestParagraph(snapshot, text, (accept) => {
        if (this.operation !== operation || this.state.phase !== "suggestion")
          return;
        if (accept && !editor.acceptParagraph(snapshot, text))
          this.notify(
            "The Draft changed. Review the paragraph again before accepting.",
            "warning",
          );
        this.cancel();
        editor.focus();
      });
      this.publish("suggestion");
      this.notify(
        "Paragraph suggestion ready. Accept or Reject below the original paragraph.",
        "info",
      );
    } catch {
      if (this.operation === operation && !operation.signal.aborted)
        this.notify(
          "Paragraph review could not finish. Your Draft is unchanged; try again.",
          "error",
        );
    } finally {
      if (this.operation === operation && this.state.phase !== "suggestion") {
        this.operation = null;
        this.publish("idle");
      }
    }
  }
  dispose() {
    this.cancel();
    this.disposed = true;
    this.listeners.clear();
  }
}

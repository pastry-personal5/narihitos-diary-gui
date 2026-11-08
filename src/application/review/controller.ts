import { AuthenticError, type Bridge, type Revision } from "../bridge";
import type { ReviewView } from "./contracts";

export interface ReviewEditor {
  /** Settles the date's Authentic queue; true when nothing is pending or failed. */
  flush(date: string): Promise<boolean>;
  /** Opens the date's Authentic tab and shows the review in it, or clears it with null. */
  show(
    date: string,
    view: ReviewView | null,
    decide: (hunk: string, action: "accept" | "reject") => void,
  ): Promise<boolean>;
  /** Asks the Authentic session to rescan and adopt the saved text. */
  reload(date: string): void;
}
export interface ReviewState {
  view: ReviewView | null;
  busy: boolean;
  invalidated: boolean;
  uncertain: boolean;
  error: string | null;
  announcement: string;
}
export const EMPTY_REVIEW_STATE: ReviewState = {
  view: null,
  busy: false,
  invalidated: false,
  uncertain: false,
  error: null,
  announcement: "",
};
export const REVIEW_MESSAGE: Record<string, string> = {
  stale_baseline: "Entry changed — regenerate to review.",
  review_invalidated: "Entry changed — regenerate to review.",
  review_unsupported:
    "This text cannot be split into reviewable paragraphs. Use the candidate text directly.",
  review_limit: "This entry is too large to review paragraph by paragraph.",
  review_active: "Finish the review of this date first.",
  authentic_save: "Resolve the Entry save problem before reviewing.",
  busy: "Another change is being saved. Try again in a moment.",
  outcome_uncertain:
    "The result of the last change is unknown. Retry the same change to confirm it.",
  decided: "That change was already decided.",
  unavailable: "Review is unavailable.",
  failed: "The change could not be saved. Your saved text is unchanged.",
};
const INVALIDATING = new Set([
  "review_invalidated",
  "revision_conflict",
  "external_change",
  "stale_baseline",
  "not_found",
]);
const same = (a: Revision | null, b: Revision | null) =>
  a === b || (!!a && !!b && a.sequence === b.sequence && a.sha256 === b.sha256);
const code = (e: unknown) =>
  e instanceof AuthenticError ? e.code : "unavailable";

/** One paragraph review at a time. Accepted text is saved by the server before it is shown as
 * accepted; the editor shows exactly the server's `body` and never computes offsets itself. */
export class ReviewController {
  state: ReviewState = { ...EMPTY_REVIEW_STATE };
  private listeners = new Set<() => void>();
  private op: { hunk: string; action: string; id: string } | null = null;
  private recheck = false;
  private disposed = false;
  constructor(
    private bridge: Bridge,
    private editor: ReviewEditor,
    private newId = () => crypto.randomUUID(),
  ) {}
  subscribe = (fn: () => void) => {
    this.listeners.add(fn);
    return () => {
      this.listeners.delete(fn);
    };
  };
  getSnapshot = () => this.state;
  private set(patch: Partial<ReviewState>) {
    this.state = { ...this.state, ...patch };
    if (!this.disposed) for (const l of this.listeners) l();
  }
  get active() {
    return this.state.view !== null;
  }
  get date() {
    return this.state.view?.date ?? null;
  }
  private ready() {
    return !!(
      this.bridge.startReview &&
      this.bridge.decideReview &&
      this.bridge.endReview
    );
  }
  async start(date: string, candidateId: string): Promise<boolean> {
    if (this.state.busy || this.state.view || !this.ready()) return false;
    this.set({ busy: true, invalidated: false, error: null, announcement: "" });
    try {
      if (!(await this.editor.flush(date))) return this.fail("authentic_save");
      const view = await this.bridge.startReview!({
        review_id: this.newId(),
        date,
        candidate_id: candidateId,
      });
      if (!(await this.editor.show(date, view, this.pick))) {
        await this.bridge.endReview!(view.review_id).catch(() => {});
        return this.fail("unavailable");
      }
      this.op = null;
      this.set({
        view,
        announcement: view.hunks.length
          ? `Review started. ${view.hunks.length} changes to review.`
          : "Review started. The candidate matches the saved entry.",
      });
      return true;
    } catch (e) {
      return this.fail(code(e));
    } finally {
      this.set({ busy: false });
      this.settleChanges();
    }
  }
  private fail(error: string) {
    this.set({ error, announcement: REVIEW_MESSAGE[error] ?? "" });
    return false;
  }
  private pick = (hunk: string, action: "accept" | "reject") => {
    void this.decide(hunk, action);
  };
  async decide(hunk: string, action: "accept" | "reject"): Promise<boolean> {
    const view = this.state.view;
    if (!view || this.state.busy || this.state.invalidated) return false;
    // A retry of an uncertain accept keeps its operation ID.
    const op =
      this.op && this.op.hunk === hunk && this.op.action === action
        ? this.op
        : { hunk, action, id: this.newId() };
    this.op = op;
    this.set({ busy: true, error: null });
    try {
      const d = await this.bridge.decideReview!({
        review_id: view.review_id,
        operation_id: op.id,
        hunk_id: hunk,
        action,
        expected_revision: view.revision,
        expected_set_digest: view.set_digest,
      });
      this.op = null;
      if (!d.view) {
        // Saved, but the server no longer holds the review: leave review mode.
        await this.editor.show(view.date, null, this.pick);
        this.editor.reload(view.date);
        this.set({
          view: null,
          uncertain: false,
          announcement: "Change saved. The review has ended.",
        });
        return true;
      }
      await this.editor.show(view.date, d.view, this.pick);
      const left = d.view.hunks.length;
      this.set({
        view: d.view,
        uncertain: false,
        announcement:
          `Change ${action === "accept" ? "accepted" : "rejected"}. ` +
          (left
            ? `${left} left.`
            : "All changes decided. End the review to resume editing."),
      });
      return true;
    } catch (e) {
      const c = code(e);
      if (c === "outcome_uncertain") {
        this.set({ uncertain: true });
        return this.fail(c);
      }
      // A retryable failure may have lost the reply of a committed accept: keep its ID.
      if (!(e instanceof AuthenticError && e.retryable)) this.op = null;
      if (INVALIDATING.has(c)) {
        await this.invalidate(view);
        return false;
      }
      return this.fail(c === "unavailable" ? "failed" : c);
    } finally {
      this.set({ busy: false });
      this.settleChanges();
    }
  }
  /** Remaining suggestions are dropped; accepted text stays saved. */
  private async invalidate(view: ReviewView) {
    await this.bridge.endReview!(view.review_id).catch(() => {});
    await this.editor.show(view.date, null, this.pick);
    this.editor.reload(view.date);
    this.set({
      view: null,
      invalidated: true,
      uncertain: false,
      error: null,
      announcement: REVIEW_MESSAGE.review_invalidated,
    });
  }
  async end(): Promise<boolean> {
    const view = this.state.view;
    if (!view) return true;
    if (this.state.busy || this.state.uncertain) {
      this.fail(this.state.busy ? "busy" : "outcome_uncertain");
      return false;
    }
    this.set({ busy: true });
    try {
      await this.bridge.endReview!(view.review_id);
      await this.editor.show(view.date, null, this.pick);
      this.editor.reload(view.date);
      this.op = null;
      this.set({
        view: null,
        error: null,
        announcement: "Review ended. Editing resumed.",
      });
      return true;
    } catch (e) {
      return this.fail(code(e));
    } finally {
      this.set({ busy: false });
    }
  }
  dismissInvalidated() {
    this.set({ invalidated: false, announcement: "" });
  }
  /** The saved Authentic changed somewhere; our own accepts keep the revision in step. */
  async onAuthenticChanged(): Promise<void> {
    const view = this.state.view;
    if (!view) return;
    if (this.state.busy) {
      this.recheck = true;
      return;
    }
    try {
      const file = await this.bridge.loadAuthentic(view.date);
      const now = this.state.view;
      if (this.state.busy) {
        this.recheck = true;
      } else if (now && !same(file.revision, now.revision)) {
        await this.invalidate(now);
      }
    } catch {
      /* The next decision is checked by the server anyway. */
    }
  }
  private settleChanges() {
    if (this.recheck && !this.state.busy) {
      this.recheck = false;
      void this.onAuthenticChanged();
    }
  }
  dispose() {
    this.disposed = true;
    this.listeners.clear();
  }
}

import type { Revision } from "../bridge";

export type HunkKind = "replace" | "insert" | "delete";
export interface ReviewHunk {
  id: string;
  kind: HunkKind;
  /** Structural label such as "heading"; plain paragraphs have none. */
  label: string | null;
  /** UTF-16 offsets into `body` of the text this change would remove or replace. */
  from: number;
  to: number;
  /** Text this change would add. */
  text: string;
}
export interface ReviewView {
  review_id: string;
  date: string;
  candidate_id: string;
  revision: Revision | null;
  set_digest: string;
  /** The saved text: baseline plus accepted changes. */
  body: string;
  hunks: ReviewHunk[];
}
export interface ReviewReceipt {
  hunk: string;
  revision: Revision;
  before: string;
  after: string;
}
export interface ReviewDecision {
  receipt: ReviewReceipt | null;
  /** Null when the review no longer exists on the server (a replay after restart). */
  view: ReviewView | null;
}
export interface ReviewBridge {
  startReview?(input: {
    review_id: string;
    date: string;
    candidate_id: string;
  }): Promise<ReviewView>;
  decideReview?(input: {
    review_id: string;
    operation_id: string;
    hunk_id: string;
    action: "accept" | "reject";
    expected_revision: Revision | null;
    expected_set_digest: string;
  }): Promise<ReviewDecision>;
  endReview?(reviewId: string): Promise<void>;
}

import type { IllustratedEntry } from "../illustrated/contracts";
import type { BufferPort } from "../authentic/session";
import type { ReviewView } from "../review/contracts";
import type { ParagraphReviewEditor } from "../paragraph-review/contracts";

export type ReviewDecide = (hunk: string, action: "accept" | "reject") => void;

/** Application needs only retained text and editing policy, never a DOM or CodeMirror state. */
export interface ResourceBufferPort extends BufferPort {
  readonly composing: boolean;
  readonly reviewing: boolean;
  readonly paragraphReview?: ParagraphReviewEditor;
  setEditable(editable: boolean): void;
  setIllustrated?(entry: IllustratedEntry, online: boolean): void;
  setReview(view: ReviewView | null, decide: ReviewDecide): void;
  focus(): void;
  dispose(): void;
}
export interface ResourceEditorHandlers {
  edit(): void;
  composition(start: boolean): void;
  save(): void;
  limit(): void;
}
export interface ResourceBuffers {
  create(
    key: string,
    label: string,
    handlers: ResourceEditorHandlers,
  ): ResourceBufferPort;
  /** Select the retained surface; attaching it to a host is adapter-owned. */
  activate(buffer: ResourceBufferPort | null): void;
}

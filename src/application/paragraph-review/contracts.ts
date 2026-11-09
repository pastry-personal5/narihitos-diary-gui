/** A captured target, not a saved revision. Offsets are UTF-16 into body. */
export interface ParagraphSnapshot {
  body: string;
  from: number;
  to: number;
  text: string;
}

/** Complete editor capability: adapters must implement every operation. */
export interface ParagraphReviewEditor {
  paragraph(): ParagraphSnapshot | null;
  getText(): string;
  suggestParagraph(
    snapshot: ParagraphSnapshot,
    text: string,
    decide: (accept: boolean) => void,
  ): void;
  clearParagraph(): void;
  acceptParagraph(snapshot: ParagraphSnapshot, text: string): boolean;
  focus(): void;
}
export interface ParagraphReviewRequest {
  text: string;
  context: ParagraphReviewSnippet[] | Promise<ParagraphReviewSnippet[]>;
  profile: { alias: string; type: string };
  locale: string;
  onProgress?: (progress: ParagraphReviewObservation) => void;
}
export interface ParagraphReviewSnippet {
  date: string;
  selected: boolean;
  text: string;
  creation_time: {
    utc: string;
    precision: "second" | "milli";
    diary_local: { datetime: string; offset_seconds: number } | null;
    conflicts_with_filename: boolean;
  } | null;
}
/** AI implementation owns protocol, prompt loading, polling, timeout and remote cleanup. */
export interface ParagraphReviewer {
  readonly available: boolean;
  review(request: ParagraphReviewRequest, signal: AbortSignal): Promise<string>;
}
export interface ParagraphReviewContext {
  editor: ParagraphReviewEditor | null;
  profile: ParagraphReviewRequest["profile"] | null;
  locale: string | null;
  blocked: boolean;
  context?: () => Promise<ParagraphReviewSnippet[]>;
}
export type ParagraphReviewPhase = "idle" | "reviewing" | "suggestion";
/** Content-free observations; execution adapters never expose chat DTOs here. */
export interface ParagraphReviewObservation {
  requestId: string | null;
  jobId: string | null;
  stage: string | null;
}
export interface ParagraphReviewProgress extends ParagraphReviewObservation {
  profile: string;
}
export interface ParagraphReviewState {
  phase: ParagraphReviewPhase;
  progress: ParagraphReviewProgress | null;
  reason: string | null;
}

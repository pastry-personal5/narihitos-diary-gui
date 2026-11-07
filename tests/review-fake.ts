import { vi } from "vitest";
import type { Bridge, Revision } from "../src/application/bridge";
import type { ReviewEditor } from "../src/application/review/controller";
import type {
  ReviewDecision,
  ReviewView,
} from "../src/application/review/contracts";

export const rev = (n: number): Revision => ({
  sequence: n,
  sha256: String(n).padStart(64, "0"),
});
export const digest = (c: string) => c.repeat(64);
export function view(over: Partial<ReviewView> = {}): ReviewView {
  return {
    review_id: "11111111-1111-4111-8111-111111111111",
    date: "2026-11-07",
    candidate_id: "job-1",
    revision: rev(1),
    set_digest: digest("a"),
    body: "A\n\nB\n\n",
    hunks: [
      {
        id: "h1",
        kind: "replace",
        label: null,
        from: 3,
        to: 6,
        text: "B2\n\n",
      },
      {
        id: "h2",
        kind: "insert",
        label: "heading",
        from: 6,
        to: 6,
        text: "# X\n",
      },
    ],
    ...over,
  };
}
export function reviewFake() {
  const bridge = {
    startReview: vi.fn(async () => view()),
    decideReview: vi.fn(async (): Promise<ReviewDecision> => ({
      receipt: null,
      view: view({ revision: rev(2), set_digest: digest("b"), hunks: [] }),
    })),
    endReview: vi.fn(async () => {}),
    loadAuthentic: vi.fn(async (date: string) => ({
      date,
      exists: true,
      revision: rev(1),
      body: "A\n\nB\n\n",
    })),
  };
  const editor = {
    flush: vi.fn(async () => true),
    show: vi.fn(async () => true),
    reload: vi.fn(),
  } satisfies ReviewEditor;
  return { bridge: bridge as unknown as Bridge, calls: bridge, editor };
}

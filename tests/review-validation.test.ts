import { describe, expect, it } from "vitest";
import {
  reviewDecisionDto,
  reviewViewDto,
} from "../src/adapters/review-validation";
import { view } from "./review-fake";

describe("review response validation", () => {
  it("accepts a sound view and rejects out-of-range or misordered offsets", () => {
    expect(reviewViewDto(view()).hunks).toHaveLength(2);
    const bad = (mutate: (v: ReturnType<typeof view>) => void) => {
      const v = structuredClone(view());
      mutate(v);
      expect(() => reviewViewDto(v)).toThrow();
    };
    bad((v) => (v.hunks[0].to = 99));
    bad((v) => (v.hunks[1].from = 1));
    bad((v) => (v.hunks[0].kind = "merge" as never));
    bad((v) => (v.hunks[1].to = 7));
    bad((v) => (v.set_digest = "short"));
    bad((v) => (v.hunks[0].id = "a/b"));
  });
  it("requires a receipt or a view in a decision", () => {
    expect(() => reviewDecisionDto({ receipt: null, view: null })).toThrow();
    expect(
      reviewDecisionDto({ receipt: null, view: view() }).view,
    ).not.toBeNull();
  });
});

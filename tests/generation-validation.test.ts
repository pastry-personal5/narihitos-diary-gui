import { describe, expect, it } from "vitest";
import { candidateDto, generationDto } from "../src/adapters/validation";
import { generationView, candidateView } from "./generation-fixtures";
import { streamEvent } from "../src/application/events";
describe("generation response validation", () => {
  it("rebuilds bounded DTOs and strips private additive fields", () => {
    const raw = {
      ...generationView(),
      path: "PRIVATE_PATH",
      progress: { ...generationView().progress, debug: "PRIVATE_DEBUG" },
    };
    expect(generationDto(raw)).toEqual(generationView());
    expect(JSON.stringify(generationDto(raw))).not.toContain("PRIVATE");
    expect(candidateDto({ ...candidateView(), extra: "PRIVATE" })).toEqual(
      candidateView(),
    );
  });
  it("rejects inconsistent identity, counts, status, code and byte bounds", () => {
    for (const progress of [
      { phase: "PRIVATE" },
      { ready_count: 255 },
      { ready_count: 0 },
      { candidate_ids: ["other"] },
      { origin_date: "2026-11-07" },
      { job_id: "other" },
      { error_code: "PRIVATE" },
      { candidate_index: 3 },
    ])
      expect(() =>
        generationDto({
          ...generationView(),
          progress: { ...generationView().progress, ...progress },
        }),
      ).toThrow();
    expect(() =>
      generationDto({ ...generationView(), provisional: "한".repeat(400000) }),
    ).toThrow();
    expect(() =>
      candidateDto({ ...candidateView(), draft_markdown: "한".repeat(400000) }),
    ).toThrow();
    expect(() => candidateDto({ ...candidateView(), id: "other" })).toThrow();
    expect(() =>
      candidateDto({
        ...candidateView(),
        claim_support: [{ claim: "x", snippet_ids: [] }],
      }),
    ).toThrow();
    expect(() =>
      candidateDto({
        ...candidateView(),
        claim_support: [{ claim: "x", snippet_ids: ["path/private"] }],
      }),
    ).toThrow();
  });
  it("preserves candidate-ready event identity without diary content", () => {
    const event = {
      kind: "candidate_ready",
      instance_id: "i",
      session_id: "s",
      event_id: 1,
      job_id: "j",
      job_sequence: 3,
      payload: { id: "j", sequence: 3, state: "running" },
      raw: "PRIVATE",
    };
    expect(streamEvent(event).kind).toBe("candidate_ready");
    expect(JSON.stringify(streamEvent(event))).not.toContain("PRIVATE");
  });
});

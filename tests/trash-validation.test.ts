import { describe, it, expect } from "vitest";
import {
  trashAckDto,
  trashPageDto,
  trashPreviewDto,
  trashResultDto,
} from "../src/adapters/trash-validation";
import { operationStateDto } from "../src/adapters/validation";
const op = "00000000-0000-4000-8000-000000000001";
const step = "00000000-0000-4000-8000-000000000002";
function members() {
  return [
    {
      ordinal: 0,
      relative_label: "2024/02/29/source.md",
      state: "moved",
      reason: null,
      retry_eligible: false,
    },
    {
      ordinal: 1,
      relative_label: "2024/02/29/source.assets/audio.m4a",
      state: "failed",
      reason: "unavailable",
      retry_eligible: true,
    },
  ];
}
function result() {
  return {
    kind: "source_trash",
    operation_id: op,
    source_id: "a".repeat(32),
    plan_hash: "b".repeat(64),
    checkpoint: 1,
    state: "needs_confirmation",
    members: members(),
  };
}
describe("Trash response validation", () => {
  it("decodes partial members and strips private additive data", () => {
    const value = {
      ...result(),
      native_root: "PRIVATE",
      receipt: "PRIVATE",
      members: members().map((m) => ({ ...m, native_path: "PRIVATE" })),
    };
    const decoded = trashResultDto(value);
    expect(decoded.members[0].state).toBe("moved");
    expect(decoded.members[1].retry_eligible).toBe(true);
    expect(JSON.stringify(decoded)).not.toContain("PRIVATE");
  });
  it("rejects absolute, traversing, oversized or control-bearing labels", () => {
    for (const label of [
      "/Users/private/audio.m4a",
      "../a",
      "a/../b",
      "a//b",
      "a\\b",
      "a\n.m4a",
      "あ".repeat(342),
    ]) {
      const value = result();
      value.members[1].relative_label = label;
      expect(() => trashResultDto(value)).toThrow();
    }
  });
  it("rejects contradictory or unbounded member outcomes", () => {
    const mutations = [
      (v: ReturnType<typeof result>) => {
        v.members[0].retry_eligible = true;
      },
      (v: ReturnType<typeof result>) => {
        v.members[1].ordinal = 0;
      },
      (v: ReturnType<typeof result>) => {
        v.members[0].state = "missing";
      },
      (v: ReturnType<typeof result>) => {
        v.members[1].reason = "private_exception";
      },
      (v: ReturnType<typeof result>) => {
        v.members.push({ ...v.members[1] });
      },
      (v: ReturnType<typeof result>) => {
        v.state = "succeeded";
      },
      (v: ReturnType<typeof result>) => {
        v.state = "uncertain";
      },
      (v: ReturnType<typeof result>) => {
        v.checkpoint = -1;
      },
      (v: ReturnType<typeof result>) => {
        v.source_id = "path";
      },
    ];
    for (const mutate of mutations) {
      const value = result();
      mutate(value);
      expect(() => trashResultDto(value)).toThrow();
    }
  });
  it("validates preview lifetime, ordinal set and acknowledgement IDs", () => {
    const preview = {
      token: "transient",
      plan_hash: "a".repeat(64),
      expected_checkpoint: 1,
      expires_in_ms: 60_000,
      ordinals: [1],
      members: members(),
    };
    expect(trashPreviewDto(preview).ordinals).toEqual([1]);
    for (const value of [
      { ...preview, expires_in_ms: 120_000 },
      { ...preview, ordinals: [1, 0] },
      { ...preview, ordinals: [] },
      { ...preview, token: "a".repeat(129) },
    ])
      expect(() => trashPreviewDto(value)).toThrow();
    expect(
      trashAckDto({ operation_id: op, step_id: step, admitted_checkpoint: 2 })
        .step_id,
    ).toBe(step);
    expect(() =>
      trashAckDto({
        operation_id: op,
        step_id: "arbitrary",
        admitted_checkpoint: 2,
      }),
    ).toThrow();
  });
  it("validates bounded unfinished discovery pages", () => {
    expect(
      trashPageDto({ operations: [result()], next_cursor: "abc.def" })
        .operations,
    ).toHaveLength(1);
    for (const value of [
      { operations: [result(), result()], next_cursor: null },
      { operations: Array(101).fill(result()), next_cursor: null },
      { operations: [result()], next_cursor: "/private?token=x" },
    ])
      expect(() => trashPageDto(value)).toThrow();
  });
  it("preserves existing save polling and validates tagged Trash lookup", () => {
    const legacy = {
      operation_id: "save-op",
      state: "succeeded",
      terminal: true,
      succeeded: true,
      result: "digest:12",
    };
    expect(operationStateDto(legacy)).toEqual(legacy);
    const value = {
      operation_id: op,
      kind: "source_trash",
      checkpoint: 1,
      state: "paused",
      terminal: false,
      succeeded: false,
      result: null,
      trash: result(),
    };
    expect(operationStateDto(value).trash?.state).toBe("needs_confirmation");
    for (const invalid of [
      { ...value, state: "needs_confirmation" },
      { ...value, terminal: true },
      { ...value, operation_id: step },
    ])
      expect(() => operationStateDto(invalid)).toThrow();
  });
});

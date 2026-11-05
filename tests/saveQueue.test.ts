import { describe, expect, it } from "vitest";
import {
  DEBOUNCE_MS,
  type Action,
  type Effect,
  type QueueState,
  guard,
  initialQueue,
  step,
} from "../src/application/authentic/saveQueue";

const rev = (n: number) => ({
  sequence: n,
  sha256: String(n).repeat(64).slice(0, 64),
});
let counter = 0;
const newId = () => `op-${++counter}`;
function run(state: QueueState, ...actions: Action[]) {
  const effects: Effect[] = [];
  for (const action of actions) {
    const r = step(state, action, newId);
    state = r.state;
    effects.push(...r.effects);
  }
  return { state, effects };
}
const sends = (effects: Effect[]) => effects.filter((e) => e.kind === "send");
const edits = (n: number): Action[] =>
  Array.from({ length: n }, () => ({ kind: "edit" }));

describe("save queue", () => {
  it("coalesces many edits into one send per tick and builds on the acknowledged revision", () => {
    counter = 0;
    let r = run(initialQueue(rev(1)), ...edits(20), { kind: "tick" });
    expect(sends(r.effects)).toEqual([
      {
        kind: "send",
        operationId: "op-1",
        generation: 20,
        expected: rev(1),
        resend: false,
      },
    ]);
    // Edits while saving send nothing; the ack of generation 20 leaves generation 23 dirty.
    r = run(r.state, ...edits(3), { kind: "tick" });
    expect(sends(r.effects)).toHaveLength(0);
    r = run(r.state, {
      kind: "ack",
      operationId: "op-1",
      generation: 20,
      revision: rev(2),
    });
    expect(r.state.state).toBe("dirty");
    expect(r.effects).toEqual([{ kind: "schedule_tick", delayMs: 300 }]);
    r = run(r.state, { kind: "tick" });
    expect(sends(r.effects)).toEqual([
      {
        kind: "send",
        operationId: "op-2",
        generation: 23,
        expected: rev(2),
        resend: false,
      },
    ]);
  });

  it("ignores old, duplicate and unknown acknowledgements", () => {
    counter = 0;
    let r = run(initialQueue(null), ...edits(1), { kind: "tick" });
    const ack: Action = {
      kind: "ack",
      operationId: "op-1",
      generation: 1,
      revision: rev(1),
    };
    r = run(r.state, ack);
    expect(r.state.state).toBe("clean");
    const before = r.state;
    r = run(r.state, ack, {
      kind: "ack",
      operationId: "other",
      generation: 9,
      revision: rev(9),
    });
    expect(r.state).toEqual(before);
  });

  it("does not save during composition and saves after it ends", () => {
    counter = 0;
    let r = run(
      initialQueue(null),
      { kind: "composition_start" },
      ...edits(2),
      { kind: "tick" },
    );
    expect(sends(r.effects)).toHaveLength(0);
    r = run(r.state, { kind: "composition_end" });
    expect(r.effects).toEqual([{ kind: "schedule_tick", delayMs: 300 }]);
    r = run(r.state, { kind: "tick" });
    expect(sends(r.effects)).toHaveLength(1);
  });

  it("flush sends immediately, waits for composition and for an in-flight save", () => {
    counter = 0;
    let r = run(initialQueue(null), ...edits(1), { kind: "flush" });
    expect(sends(r.effects)).toHaveLength(1);
    r = run(r.state, ...edits(1), { kind: "flush" });
    expect(sends(r.effects)).toHaveLength(0);
    r = run(r.state, {
      kind: "ack",
      operationId: "op-1",
      generation: 1,
      revision: rev(1),
    });
    expect(sends(r.effects)).toHaveLength(1); // the held flush goes out at once
    const c = run(
      initialQueue(null),
      { kind: "composition_start" },
      ...edits(1),
      { kind: "flush" },
    );
    expect(sends(c.effects)).toHaveLength(0);
    expect(
      sends(run(c.state, { kind: "composition_end" }).effects),
    ).toHaveLength(1);
  });

  it("queries the original operation after a timeout and never mints a new ID", () => {
    counter = 0;
    let r = run(
      initialQueue(null),
      ...edits(1),
      { kind: "tick" },
      {
        kind: "failed",
        operationId: "op-1",
        code: "server_unavailable",
        retryable: true,
      },
    );
    expect(r.effects.at(-1)).toEqual({
      kind: "get_operation",
      operationId: "op-1",
      delayMs: 0,
    });
    r = run(r.state, {
      kind: "operation",
      operationId: "op-1",
      found: true,
      terminal: true,
      succeeded: true,
      result: `4:${"b".repeat(64)}`,
    });
    expect(r.state.state).toBe("clean");
    expect(r.state.acknowledged).toEqual({
      sequence: 4,
      sha256: "b".repeat(64),
    });
    expect(counter).toBe(1);
  });

  it("resends the same ID when the server never saw the operation", () => {
    counter = 0;
    const r = run(
      initialQueue(rev(1)),
      ...edits(1),
      { kind: "tick" },
      {
        kind: "failed",
        operationId: "op-1",
        code: "server_unavailable",
        retryable: true,
      },
      {
        kind: "operation",
        operationId: "op-1",
        found: false,
        terminal: false,
        succeeded: false,
        result: null,
      },
    );
    expect(r.effects.at(-1)).toEqual({
      kind: "send",
      operationId: "op-1",
      generation: 1,
      expected: rev(1),
      resend: true,
    });
  });

  it("gives up after repeated unknown outcomes and retry asks about the same operation", () => {
    counter = 0;
    let r = run(
      initialQueue(null),
      ...edits(1),
      { kind: "tick" },
      {
        kind: "failed",
        operationId: "op-1",
        code: "outcome_uncertain",
        retryable: true,
      },
    );
    for (let i = 0; i < 5; i++)
      r = run(r.state, {
        kind: "operation",
        operationId: "op-1",
        found: true,
        terminal: false,
        succeeded: false,
        result: null,
      });
    expect(r.state.state).toBe("save_failed");
    r = run(r.state, { kind: "retry" });
    expect(r.effects).toEqual([
      { kind: "get_operation", operationId: "op-1", delayMs: 0 },
    ]);
  });

  it("retains the original operation when a success receipt is malformed", () => {
    counter = 0;
    const r = run(
      initialQueue(null),
      ...edits(1),
      { kind: "tick" },
      {
        kind: "failed",
        operationId: "op-1",
        code: "outcome_uncertain",
        retryable: true,
      },
      {
        kind: "operation",
        operationId: "op-1",
        found: true,
        terminal: true,
        succeeded: true,
        result: "bad",
      },
    );
    expect(r.state.state).toBe("save_failed");
    expect(r.state.inFlight?.operationId).toBe("op-1");
    expect(run(r.state, { kind: "retry" }).effects).toEqual([
      { kind: "get_operation", operationId: "op-1", delayMs: 0 },
    ]);
  });
  it("never discards an admitted save whose outcome remains uncertain", () => {
    counter = 0;
    let r = run(
      initialQueue(rev(1)),
      ...edits(1),
      { kind: "tick" },
      {
        kind: "failed",
        operationId: "op-1",
        code: "outcome_uncertain",
        retryable: true,
      },
    );
    for (let i = 0; i < 5; i++)
      r = run(r.state, {
        kind: "operation",
        operationId: "op-1",
        found: true,
        terminal: false,
        succeeded: false,
        result: null,
      });
    const before = r.state;
    r = run(before, { kind: "discard" });
    expect(r.state).toEqual(before);
    expect(r.effects).toEqual([]);
    expect(r.state.inFlight?.operationId).toBe("op-1");
  });

  it("a definite failure keeps the buffer and retry sends a fresh operation", () => {
    counter = 0;
    let r = run(
      initialQueue(null),
      ...edits(2),
      { kind: "tick" },
      {
        kind: "failed",
        operationId: "op-1",
        code: "storage_error",
        retryable: false,
      },
    );
    expect(r.state.state).toBe("save_failed");
    expect(guard(r.state)).toBe("blocked");
    r = run(r.state, ...edits(1), { kind: "tick" });
    expect(sends(r.effects)).toHaveLength(0); // blocked states never autosave
    r = run(r.state, { kind: "retry" });
    expect(sends(r.effects)).toEqual([
      {
        kind: "send",
        operationId: "op-2",
        generation: 3,
        expected: null,
        resend: false,
      },
    ]);
  });

  it("an invalidation reloads a clean buffer and conflicts a dirty one", () => {
    counter = 0;
    const r = run(initialQueue(rev(1)), { kind: "invalidated" });
    expect(r.effects).toEqual([{ kind: "reload" }]);
    const clean = run(r.state, {
      kind: "loaded",
      revision: rev(2),
      body: "disk",
    });
    expect(clean.state.acknowledged).toEqual(rev(2));
    expect(clean.effects).toEqual([{ kind: "replace_body", body: "disk" }]);
    const dirty = run(initialQueue(rev(1)), ...edits(1), {
      kind: "loaded",
      revision: rev(2),
      body: "disk",
    });
    expect(dirty.state.state).toBe("external_conflict");
    expect(dirty.effects.map((e) => e.kind)).not.toContain("replace_body");
  });

  it("reloading our own acknowledged revision changes nothing", () => {
    const s = initialQueue(rev(3));
    expect(run(s, { kind: "loaded", revision: rev(3), body: "x" })).toEqual({
      state: s,
      effects: [],
    });
  });

  it("an invalidation during a save is held until the save settles", () => {
    counter = 0;
    let r = run(
      initialQueue(rev(1)),
      ...edits(1),
      { kind: "tick" },
      { kind: "loaded", revision: rev(2), body: "ours" },
    );
    expect(r.state.state).toBe("saving");
    r = run(r.state, {
      kind: "ack",
      operationId: "op-1",
      generation: 1,
      revision: rev(2),
    });
    expect(r.state.state).toBe("clean");
    expect(r.effects).toEqual([{ kind: "reload" }]);
  });

  it("a stale save conflicts, keep_mine builds on the server revision only after reload", () => {
    counter = 0;
    let r = run(
      initialQueue(rev(1)),
      ...edits(1),
      { kind: "tick" },
      {
        kind: "failed",
        operationId: "op-1",
        code: "revision_conflict",
        retryable: false,
      },
    );
    expect(r.state.state).toBe("external_conflict");
    expect(r.effects.at(-1)).toEqual({ kind: "reload" });
    expect(sends(run(r.state, { kind: "keep_mine" }).effects)).toHaveLength(0);
    expect(sends(run(r.state, { kind: "retry" }).effects)).toHaveLength(0);
    r = run(r.state, { kind: "loaded", revision: rev(5), body: "disk" });
    r = run(r.state, { kind: "keep_mine" });
    expect(sends(r.effects)).toEqual([
      {
        kind: "send",
        operationId: "op-2",
        generation: 1,
        expected: rev(5),
        resend: false,
      },
    ]);
  });

  it("discard reloads and adopts the file, leaving the buffer clean", () => {
    counter = 0;
    let r = run(
      initialQueue(rev(1)),
      ...edits(2),
      { kind: "tick" },
      {
        kind: "failed",
        operationId: "op-1",
        code: "storage_error",
        retryable: false,
      },
      { kind: "discard" },
    );
    expect(r.effects.at(-1)).toEqual({ kind: "reload" });
    r = run(r.state, { kind: "loaded", revision: rev(1), body: "disk" });
    expect(r.state.state).toBe("clean");
    expect(r.effects).toEqual([{ kind: "replace_body", body: "disk" }]);
    expect(guard(r.state)).toBe("proceed");
  });

  it("still saves text typed during a save when an invalidation was held back", () => {
    counter = 0;
    let r = run(initialQueue(rev(1)), ...edits(1), { kind: "tick" });
    r = run(r.state, ...edits(2), { kind: "invalidated" });
    r = run(r.state, { kind: "loaded", revision: rev(1), body: "same" });
    expect(r.state.recheck).toBe(true);
    r = run(r.state, {
      kind: "ack",
      operationId: "op-1",
      generation: 1,
      revision: rev(2),
    });
    expect(r.effects).toEqual([{ kind: "reload" }]);
    r = run(r.state, { kind: "loaded", revision: rev(2), body: "same" });
    expect(r.effects).toEqual([
      { kind: "schedule_tick", delayMs: DEBOUNCE_MS },
    ]);
  });

  it("guards transitions", () => {
    expect(guard(initialQueue(null))).toBe("proceed");
    expect(guard(run(initialQueue(null), ...edits(1)).state)).toBe("wait");
  });
});

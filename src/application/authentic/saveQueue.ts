import type { Revision } from "../bridge";

/** Pure autosave state machine. Timers and I/O are effects run by the caller (see `AuthenticSession`). */
export const DEBOUNCE_MS = 300;
const RECOVERY_DELAY_MS = 1000;
const MAX_ATTEMPTS = 5;

export type SaveState =
  "clean" | "dirty" | "saving" | "save_failed" | "external_conflict";

export interface QueueState {
  state: SaveState;
  /** Disconnect/reconciliation suspends all new writes. */
  paused: boolean;
  /** Increases with every user edit; programmatic body replacement never counts. */
  generation: number;
  /** Highest generation covered by an acknowledgement. */
  saved: number;
  /** Last revision the server acknowledged or the client loaded; null means the file is absent. */
  acknowledged: Revision | null;
  inFlight: {
    operationId: string;
    generation: number;
    recovering: boolean;
    attempts: number;
  } | null;
  composing: boolean;
  flushPending: boolean;
  /** An invalidation arrived while a save was in flight; reload once it settles. */
  recheck: boolean;
  /** The server revision seen while in conflict; adopted by `keep_mine`. */
  /** undefined until the reload after a conflict has answered; null means the file is absent. */
  external: Revision | null | undefined;
  discarding: boolean;
  error: string | null;
}

export type Action =
  | { kind: "pause" }
  | { kind: "resume" }
  | { kind: "edit" }
  | { kind: "tick" }
  | { kind: "composition_start" }
  | { kind: "composition_end" }
  | { kind: "flush" }
  | {
      kind: "ack";
      operationId: string;
      generation: number;
      revision: Revision;
    }
  | { kind: "failed"; operationId: string; code: string; retryable: boolean }
  | {
      kind: "operation";
      operationId: string;
      found: boolean;
      terminal: boolean;
      succeeded: boolean;
      result: string | null;
    }
  | { kind: "loaded"; revision: Revision | null; body: string }
  | { kind: "invalidated" }
  | { kind: "retry" }
  | { kind: "keep_mine" }
  | { kind: "discard" };

export type Effect =
  | { kind: "schedule_tick"; delayMs: number }
  | {
      kind: "send";
      operationId: string;
      generation: number;
      expected: Revision | null;
      /** Resend the body stored for this operation instead of the current text. */
      resend: boolean;
    }
  | { kind: "get_operation"; operationId: string; delayMs: number }
  | { kind: "reload" }
  | { kind: "replace_body"; body: string };

export const initialQueue = (acknowledged: Revision | null): QueueState => ({
  state: "clean",
  paused: false,
  generation: 0,
  saved: 0,
  acknowledged,
  inFlight: null,
  composing: false,
  flushPending: false,
  recheck: false,
  external: undefined,
  discarding: false,
  error: null,
});

const dirty = (s: QueueState) => s.generation > s.saved;
const same = (a: Revision | null, b: Revision | null) =>
  a === b || (!!a && !!b && a.sequence === b.sequence && a.sha256 === b.sha256);
const CONFLICT = ["revision_conflict", "external_change"];

type Result = { state: QueueState; effects: Effect[] };
const out = (state: QueueState, ...effects: Effect[]): Result => ({
  state,
  effects,
});

function send(s: QueueState, newId: () => string): Result {
  if (s.paused) return out({ ...s, state: "dirty", flushPending: true });
  const operationId = newId();
  return out(
    {
      ...s,
      state: "saving",
      flushPending: false,
      error: null,
      inFlight: {
        operationId,
        generation: s.generation,
        recovering: false,
        attempts: 0,
      },
    },
    {
      kind: "send",
      operationId,
      generation: s.generation,
      expected: s.acknowledged,
      resend: false,
    },
  );
}

function settle(s: QueueState, newId: () => string): Result {
  // After an acknowledgement: reload if an invalidation was held back (newer text goes out
  // once `loaded` has checked the revision), else send newer text.
  if (s.recheck)
    return out(
      { ...s, recheck: false, state: dirty(s) ? "dirty" : "clean" },
      { kind: "reload" },
    );
  if (!dirty(s)) return out({ ...s, state: "clean" });
  if (s.flushPending && !s.composing) return send(s, newId);
  return out(
    { ...s, state: "dirty" },
    {
      kind: "schedule_tick",
      delayMs: DEBOUNCE_MS,
    },
  );
}

function fail(s: QueueState, error: string): Result {
  return out({ ...s, state: "save_failed", error });
}

function conflict(s: QueueState): Result {
  return out(
    { ...s, state: "external_conflict", inFlight: null, error: null },
    { kind: "reload" },
  );
}

export function step(s: QueueState, a: Action, newId: () => string): Result {
  const blocked = s.state === "save_failed" || s.state === "external_conflict";
  switch (a.kind) {
    case "pause":
      return out({ ...s, paused: true });
    case "resume": {
      const next = { ...s, paused: false };
      if (s.inFlight)
        return out(
          {
            ...next,
            state: "saving",
            inFlight: { ...s.inFlight, recovering: true, attempts: 0 },
          },
          {
            kind: "get_operation",
            operationId: s.inFlight.operationId,
            delayMs: 0,
          },
        );
      if (blocked) return out(next);
      return settle({ ...next, flushPending: dirty(next) }, newId);
    }
    case "edit": {
      const next = {
        ...s,
        generation: s.generation + 1,
        state: blocked ? s.state : s.inFlight ? "saving" : "dirty",
      } as QueueState;
      return blocked || s.inFlight
        ? out(next)
        : out(next, { kind: "schedule_tick", delayMs: DEBOUNCE_MS });
    }
    case "tick":
      if (s.paused || blocked || s.composing || s.inFlight || !dirty(s))
        return out(s);
      return send(s, newId);
    case "composition_start":
      return out({ ...s, composing: true });
    case "composition_end": {
      const next = { ...s, composing: false };
      if (s.recheck && !s.inFlight)
        return out({ ...next, recheck: false }, { kind: "reload" });
      if (blocked || s.inFlight || !dirty(next)) return out(next);
      return next.flushPending
        ? send(next, newId)
        : out(next, { kind: "schedule_tick", delayMs: DEBOUNCE_MS });
    }
    case "flush":
      if (blocked || !dirty(s)) return out(s);
      if (s.composing || s.inFlight) return out({ ...s, flushPending: true });
      return send(s, newId);
    case "ack": {
      if (s.inFlight?.operationId !== a.operationId) return out(s);
      return settle(
        {
          ...s,
          inFlight: null,
          acknowledged: a.revision,
          saved: Math.max(s.saved, a.generation),
          error: null,
        },
        newId,
      );
    }
    case "failed": {
      const f = s.inFlight;
      if (f?.operationId !== a.operationId) return out(s);
      if (f.recovering) {
        // The status query itself failed.
        if (f.attempts + 1 >= MAX_ATTEMPTS)
          return fail(
            { ...s, inFlight: { ...f, attempts: f.attempts + 1 } },
            a.code,
          );
        return out(
          { ...s, inFlight: { ...f, attempts: f.attempts + 1 } },
          {
            kind: "get_operation",
            operationId: f.operationId,
            delayMs: RECOVERY_DELAY_MS,
          },
        );
      }
      if (CONFLICT.includes(a.code)) return conflict(s);
      if (a.retryable)
        return out(
          { ...s, inFlight: { ...f, recovering: true } },
          { kind: "get_operation", operationId: f.operationId, delayMs: 0 },
        );
      return fail({ ...s, inFlight: null }, a.code);
    }
    case "operation": {
      const f = s.inFlight;
      if (f?.operationId !== a.operationId || !f.recovering) return out(s);
      const attempts = f.attempts + 1;
      if (!a.found || !a.terminal) {
        if (attempts >= MAX_ATTEMPTS)
          return fail(
            { ...s, inFlight: { ...f, attempts } },
            "outcome_uncertain",
          );
        const next = { ...s, inFlight: { ...f, attempts } };
        return a.found
          ? out(next, {
              kind: "get_operation",
              operationId: f.operationId,
              delayMs: RECOVERY_DELAY_MS,
            })
          : // Unknown to the server: the original ID is safe to send again (the server deduplicates).
            out(
              { ...next, inFlight: { ...next.inFlight, recovering: false } },
              {
                kind: "send",
                operationId: f.operationId,
                generation: f.generation,
                expected: s.acknowledged,
                resend: true,
              },
            );
      }
      if (a.succeeded) {
        const m = /^(\d+):([0-9a-f]{64})$/i.exec(a.result ?? "");
        if (!m) return fail(s, "invalid_response");
        const sequence = Number(m[1]);
        if (!Number.isSafeInteger(sequence) || sequence < 1)
          return fail(s, "invalid_response");
        return step(
          s,
          {
            kind: "ack",
            operationId: f.operationId,
            generation: f.generation,
            revision: { sequence, sha256: m[2] },
          },
          newId,
        );
      }
      return /conflict|exists/.test(a.result ?? "")
        ? conflict(s)
        : fail({ ...s, inFlight: null }, "operation_failed");
    }
    case "invalidated":
      return out(s, { kind: "reload" });
    case "loaded": {
      if (s.composing) return out({ ...s, recheck: true });
      if (s.discarding) {
        return out(
          {
            ...initialQueue(a.revision),
            generation: s.generation,
            saved: s.generation,
            composing: s.composing,
            paused: s.paused,
          },
          { kind: "replace_body", body: a.body },
        );
      }
      if (s.inFlight) return out({ ...s, recheck: true });
      if (same(a.revision, s.acknowledged) && s.state !== "external_conflict") {
        // Text typed during the save or composition was held back for this check.
        if (!dirty(s) || blocked || s.paused || s.composing) return out(s);
        return s.flushPending
          ? send(s, newId)
          : out(s, { kind: "schedule_tick", delayMs: DEBOUNCE_MS });
      }
      if (s.state === "external_conflict")
        return out({ ...s, external: a.revision });
      if (dirty(s) || blocked)
        return out({ ...s, state: "external_conflict", external: a.revision });
      return out(
        { ...s, acknowledged: a.revision, state: "clean" },
        { kind: "replace_body", body: a.body },
      );
    }
    case "retry":
      if (s.state === "save_failed") {
        if (s.inFlight)
          return out(
            {
              ...s,
              state: "saving",
              inFlight: { ...s.inFlight, recovering: true, attempts: 0 },
              error: null,
            },
            {
              kind: "get_operation",
              operationId: s.inFlight.operationId,
              delayMs: 0,
            },
          );
        return send({ ...s, state: "dirty" }, newId);
      }
      return out(s);
    case "keep_mine":
      // Reconcile: the buffer now builds on the server's current revision.
      if (s.state !== "external_conflict" || s.external === undefined)
        return out(s);
      return send(
        { ...s, acknowledged: s.external, external: undefined },
        newId,
      );
    case "discard":
      // An admitted operation with an unknown outcome must be reconciled first.
      if (s.inFlight) return out(s);
      return out(
        { ...s, inFlight: null, discarding: true, flushPending: false },
        { kind: "reload" },
      );
  }
}

export type Transition = "proceed" | "wait" | "blocked";
/** Whether a date change or close may go ahead. */
export function guard(s: QueueState): Transition {
  if (
    s.state === "save_failed" ||
    s.state === "external_conflict" ||
    (s.paused && (dirty(s) || s.inFlight))
  )
    return "blocked";
  return s.state === "clean" && !s.inFlight && !s.composing
    ? "proceed"
    : "wait";
}

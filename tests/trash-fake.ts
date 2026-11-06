import { vi } from "vitest";
import { AuthenticError, type Bridge } from "../src/application/bridge";
import type {
  ImportPage,
  Source,
} from "../src/application/navigation/contracts";
import type {
  TrashConfirmRequest,
  TrashPreviewRequest,
  TrashResult,
} from "../src/application/trash/contracts";
import { navigationFake } from "./navigation-fake";
export const day = "2024-02-29",
  sourceId = "a".repeat(32),
  planHash = "c".repeat(64);
export const uuid = (n: number) =>
  `00000000-0000-4000-8000-${String(n).padStart(12, "0")}`;
export function deferred<T>() {
  let resolve!: (v: T) => void, reject!: (e: unknown) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}
export function snippet(id = sourceId, importId = "import-1"): Source {
  const local = { date: day, datetime: `${day}T00:00:00`, offset_seconds: 0 };
  return {
    id,
    relative_key: `2024/02/29/${id}.md`,
    digest: "a".repeat(64),
    import_id: importId,
    dating: {
      state: "agreed",
      header: { unix_ms: 0, precision: "milli" },
      filename: { unix_ms: 0, precision: "milli" },
    },
    duplicate_producer_id: false,
    header_invalid: null,
    filename_invalid: null,
    eligible: true,
    navigation: {
      group_id: "group",
      header: local,
      filename: local,
      included: local,
      authority: null,
    },
    capabilities: {
      read: true,
      save: true,
      trash: true,
      reason: "ready",
      trash_reason: "ready",
    },
  };
}
export function result(
  id = uuid(1),
  state: TrashResult["state"] = "succeeded",
  checkpoint = 1,
): TrashResult {
  return {
    kind: "source_trash",
    operation_id: id,
    source_id: sourceId,
    plan_hash: planHash,
    checkpoint,
    state,
    members: [
      {
        ordinal: 0,
        relative_label: snippet().relative_key,
        state:
          state === "running"
            ? "pending"
            : state === "uncertain"
              ? "uncertain"
              : "moved",
        reason: state === "uncertain" ? "uncertain" : null,
        retry_eligible: false,
      },
      {
        ordinal: 1,
        relative_label: `2024/02/29/${sourceId}.assets/${sourceId}.m4a`,
        state:
          state === "succeeded"
            ? "moved"
            : state === "needs_confirmation"
              ? "failed"
              : "pending",
        reason: state === "needs_confirmation" ? "unavailable" : null,
        retry_eligible: state === "needs_confirmation",
      },
    ],
  };
}
export function trashFake() {
  const operations = new Map<string, TrashResult>(),
    dates = new Map<string, string>();
  let imports = 0;
  const bridge = {
    ...navigationFake(),
    loadAuthentic: vi.fn(async (date: string) => ({
      date,
      exists: true,
      revision: { sequence: 1, sha256: "f".repeat(64) },
      body: `Authentic ${date}`,
    })),
    saveAuthentic: vi.fn(async (r: Parameters<Bridge["saveAuthentic"]>[0]) => ({
      operation_id: r.operationId,
      revision: { sequence: 2, sha256: "f".repeat(64) },
      buffer_generation: r.bufferGeneration,
    })),
    importDate: vi.fn(async (date: string) => {
      const id = `import-${++imports}`;
      dates.set(id, date);
      return {
        import_id: id,
        job: { id, sequence: 1, state: "succeeded" as const },
      };
    }),
    listSources: vi.fn(async (importId: string): Promise<ImportPage> => ({
      import_id: importId,
      date: dates.get(importId) ?? day,
      zone: "UTC",
      timezone_database: "test",
      config_generation: 1,
      completed_at: 1,
      items: [
        snippet(sourceId, importId),
        snippet("b".repeat(32), importId),
      ].map((source) => ({
        kind: "included" as const,
        source,
        instant:
          source.dating.state === "agreed"
            ? source.dating.header
            : { unix_ms: 0, precision: "milli" },
        misplaced: false,
      })),
      next_cursor: null,
      incomplete: false,
      groups: [{ id: "group", alias: "Diary", type: "narihito" }],
      reimport_required: false,
    })),
    readSource: vi.fn(async (id: string, snapshot: string) => ({
      source: snippet(id, snapshot),
      snapshot_id: snapshot,
      body: `Source ${id}`,
      capabilities: snippet().capabilities,
    })),
    saveSource: vi.fn(
      async (
        _id: string,
        operation: string,
        _digest: string,
        body: string,
      ) => ({
        operation_id: operation,
        digest: "b".repeat(64),
        size: body.length,
        unchanged: false,
      }),
    ),
    previewSourceDelete: vi.fn(
      async (_id: string, request: TrashPreviewRequest) => ({
        token: "transient",
        plan_hash: planHash,
        expected_checkpoint: request.expected_checkpoint,
        expires_in_ms: 60_000,
        ordinals: request.kind === "initial" ? [0, 1] : [1],
        members: result(
          uuid(1),
          request.kind === "initial" ? "running" : "needs_confirmation",
        ).members.map((m) => ({
          ...m,
          retry_eligible:
            request.kind === "initial"
              ? m.state === "pending"
              : m.retry_eligible,
        })),
      }),
    ),
    confirmSourceDelete: vi.fn(
      async (id: string, request: TrashConfirmRequest) => {
        const r = result(
          request.operation_id,
          "succeeded",
          request.expected_checkpoint + 1,
        );
        r.source_id = id;
        operations.set(request.operation_id, r);
        return {
          operation_id: request.operation_id,
          step_id: request.step_id,
          admitted_checkpoint: request.expected_checkpoint + 1,
        };
      },
    ),
    getOperation: vi.fn(async (id: string) => {
      const r = operations.get(id);
      if (!r) throw new AuthenticError("not_found", false);
      return {
        operation_id: id,
        kind: "source_trash",
        checkpoint: r.checkpoint,
        state: r.state === "needs_confirmation" ? "paused" : r.state,
        terminal: r.state === "succeeded",
        succeeded: r.state === "succeeded",
        result: null,
        trash: r,
      };
    }),
    listSourceDeletions: vi.fn(async () => ({
      operations: [...operations.values()].filter(
        (r) => r.state !== "succeeded",
      ),
      next_cursor: null,
    })),
  };
  return { bridge, operations };
}

import { describe, expect, it, vi } from "vitest";
import { AuthenticError, type Bridge } from "../src/application/bridge";
import type {
  ExportResult,
  StartExportOutcome,
} from "../src/application/export/contracts";
import { ExportController } from "../src/application/export/controller";

const OP = "00000000-0000-4000-8000-000000000001";
const result = (state: ExportResult["state"], reason: string | null = null) =>
  ({
    kind: "export",
    operation_id: OP,
    target: "icloud",
    date: "2026-11-05",
    revision: { sequence: 1, sha256: "a".repeat(64) },
    state,
    checkpoint: 2,
    reason,
  }) as ExportResult;
const preview = (
  reason: "changed" | "uncertain" = "changed",
): StartExportOutcome => ({
  outcome: "needs_confirmation",
  preview: {
    operation_id: OP,
    expected_checkpoint: 1,
    token: "t".repeat(64),
    reason,
    write: "replace",
    expires_in_ms: 60000,
  },
});
function setup(over: Record<string, unknown> = {}, flush = true) {
  const notes: string[] = [];
  let n = 0;
  const bridge = {
    loadAuthentic: vi.fn(async () => ({
      date: "2026-11-05",
      exists: true,
      body: "x",
      revision: { sequence: 1, sha256: "a".repeat(64) },
    })),
    startExport: vi.fn(async () => ({
      outcome: "result" as const,
      result: result("succeeded"),
    })),
    confirmExport: vi.fn(async () => ({ admitted_checkpoint: 2 })),
    abandonExport: vi.fn(),
    confirmAbandonExport: vi.fn(),
    listExports: vi.fn(async () => []),
    cancelJob: vi.fn(async () => ({})),
    getOperation: vi.fn(async () => ({
      operation_id: OP,
      state: "succeeded",
      terminal: true,
      succeeded: true,
      result: "export",
      export: result("succeeded"),
    })),
    ...over,
  } as unknown as Bridge;
  const c = new ExportController(bridge, {
    flush: async () => flush,
    notify: (t) => notes.push(t),
    newId: () => `00000000-0000-4000-8000-${String(++n).padStart(12, "0")}`,
    sleep: async () => {},
  });
  return { c, bridge, notes };
}
const uncertainPath = (extra: Record<string, unknown> = {}) => ({
  startExport: vi.fn(async () => {
    throw new AuthenticError("outcome_uncertain", false);
  }),
  listExports: vi.fn(async () => [result("uncertain")]),
  abandonExport: vi.fn(async () => preview("uncertain")),
  ...extra,
});

describe("export controller", () => {
  it("exports the acknowledged revision after flushing", async () => {
    const { c, bridge, notes } = setup();
    await c.start("2026-11-05");
    expect(bridge.startExport).toHaveBeenCalledWith(
      expect.objectContaining({
        expected_revision: { sequence: 1, sha256: "a".repeat(64) },
        target: "icloud",
      }),
    );
    expect(notes).toEqual(["Exported 2026-11-05 to iCloud."]);
    expect(c.getSnapshot().phase).toBe("idle");
  });
  it("does not export when the buffer cannot be saved", async () => {
    const { c, bridge, notes } = setup({}, false);
    await c.start("2026-11-05");
    expect(bridge.startExport).not.toHaveBeenCalled();
    expect(notes[0]).toMatch(/could not be saved/u);
  });
  it("needs an explicit confirmation to replace a changed file", async () => {
    const { c, bridge } = setup({ startExport: vi.fn(async () => preview()) });
    await c.start("2026-11-05");
    expect(c.getSnapshot().phase).toBe("preview");
    expect(bridge.confirmExport).not.toHaveBeenCalled();
    await c.confirm();
    expect(bridge.confirmExport).toHaveBeenCalledWith(
      OP,
      expect.objectContaining({ expected_checkpoint: 1 }),
    );
    expect(c.getSnapshot().phase).toBe("idle");
  });
  it("declining cancels the export and never confirms", async () => {
    const { c, bridge, notes } = setup({
      startExport: vi.fn(async () => preview()),
      getOperation: vi.fn(async () => ({ export: result("cancelled") })),
    });
    await c.start("2026-11-05");
    await c.decline();
    expect(bridge.cancelJob).toHaveBeenCalledWith(OP);
    expect(bridge.confirmExport).not.toHaveBeenCalled();
    expect(notes[0]).toMatch(/not changed/u);
  });
  it("maps typed failures to app-authored text", async () => {
    const { c, notes } = setup({
      startExport: vi.fn(async () => {
        throw new AuthenticError("permission_denied", false);
      }),
    });
    await c.start("2026-11-05");
    expect(notes[0]).toMatch(/denied access/u);
  });
  it("finds an unknown earlier export and abandons it after confirmation", async () => {
    const { c, bridge, notes } = setup(
      uncertainPath({
        confirmAbandonExport: vi.fn(async () =>
          result("cancelled", "abandoned"),
        ),
      }),
    );
    await c.start("2026-11-05");
    expect(c.getSnapshot().phase).toBe("uncertain");
    await c.abandon();
    expect(c.getSnapshot().phase).toBe("preview");
    expect(c.getSnapshot().preview?.abandon).toBe(true);
    await c.confirm();
    expect(bridge.confirmAbandonExport).toHaveBeenCalled();
    expect(bridge.confirmExport).not.toHaveBeenCalled();
    expect(notes.at(-1)).toMatch(/abandoned/u);
    expect(c.getSnapshot().phase).toBe("idle");
  });
  it("declining an abandon preview leaves the export unresolved", async () => {
    const { c, bridge } = setup(uncertainPath());
    await c.start("2026-11-05");
    await c.abandon();
    await c.decline();
    expect(bridge.cancelJob).not.toHaveBeenCalled();
    expect(c.getSnapshot().phase).toBe("idle");
  });
  it("explains why export cannot run", () => {
    const { c } = setup();
    expect(c.reason(null, true)).toMatch(/Select a date/u);
    expect(c.reason("2026-11-05", false)).toMatch(/Open the Authentic/u);
    expect(c.reason("2026-11-05", true)).toBeNull();
  });
});

describe("export recovery and truthful status", () => {
  it("does not announce cancellation when the cancel request fails", async () => {
    const { c, notes } = setup({
      startExport: vi.fn(async () => preview()),
      cancelJob: vi.fn(async () => {
        throw new Error("lost response");
      }),
    });
    await c.start("2026-11-05");
    await c.decline();
    expect(notes.some((text) => text.includes("cancelled"))).toBe(false);
    expect(notes.at(-1)).toMatch(/unavailable/u);
  });
  it("reports the real outcome when cancellation loses a race to confirmation", async () => {
    const { c, notes } = setup({ startExport: vi.fn(async () => preview()) });
    await c.start("2026-11-05");
    await c.decline();
    expect(notes.at(-1)).toMatch(/Exported/u);
    expect(notes.some((text) => text.includes("not changed"))).toBe(false);
  });
  it("re-previews the same operation after concurrent drift pauses it", async () => {
    const startExport = vi
      .fn()
      .mockResolvedValueOnce({ outcome: "result", result: result("running") })
      .mockResolvedValueOnce(preview());
    const { c } = setup({
      startExport,
      getOperation: vi.fn(async () => ({
        export: result("needs_confirmation"),
      })),
    });
    await c.start("2026-11-05");
    expect(c.getSnapshot().phase).toBe("preview");
    expect(startExport).toHaveBeenLastCalledWith(
      expect.objectContaining({
        operation_id: OP,
        expected_revision: result("running").revision,
      }),
    );
  });
  it("continues tracking exports beyond one minute", async () => {
    let polls = 0;
    const { c, notes } = setup({
      startExport: vi.fn(async () => ({
        outcome: "result",
        result: result("running"),
      })),
      getOperation: vi.fn(async () => ({
        export: result(++polls > 120 ? "succeeded" : "running"),
      })),
    });
    await c.start("2026-11-05");
    expect(polls).toBe(121);
    expect(c.getSnapshot().phase).toBe("idle");
    expect(notes).toEqual(["Exported 2026-11-05 to iCloud."]);
  });
  it("shows the original date when resolving an earlier export", async () => {
    const { c } = setup(uncertainPath());
    await c.start("2026-11-06");
    expect(c.getSnapshot().date).toBe("2026-11-05");
  });
});

it("recovers a paused export after reload instead of stranding its reservation", async () => {
  const startExport = vi
    .fn()
    .mockRejectedValueOnce(new AuthenticError("resource_reserved", false))
    .mockResolvedValueOnce(preview());
  const { c } = setup({
    startExport,
    listExports: vi.fn(async () => [result("needs_confirmation")]),
  });
  await c.start("2026-11-06");
  expect(c.getSnapshot().phase).toBe("preview");
  expect(c.getSnapshot().date).toBe("2026-11-05");
  expect(startExport).toHaveBeenLastCalledWith(
    expect.objectContaining({ operation_id: OP, date: "2026-11-05" }),
  );
});

it("names the failure when a paused export cannot be resumed", async () => {
  const startExport = vi
    .fn()
    .mockRejectedValueOnce(new AuthenticError("resource_reserved", false))
    .mockRejectedValueOnce(new AuthenticError("invalid_response", false));
  const { c, notes } = setup({
    startExport,
    listExports: vi.fn(async () => [result("needs_confirmation")]),
  });
  await c.start("2026-11-06");
  expect(notes).toEqual([
    "Could not resume the earlier export (invalid_response).",
  ]);
  expect(c.getSnapshot().phase).toBe("idle");
});

it("narrates each step and reports progress every 2 seconds when verbose", async () => {
  vi.useFakeTimers();
  try {
    const notes: string[] = [];
    let release!: () => void;
    const gate = new Promise<void>((r) => (release = r));
    const bridge = {
      loadAuthentic: vi.fn(async () => ({
        date: "2026-11-05",
        exists: true,
        body: "x",
        revision: { sequence: 3, sha256: "a".repeat(64) },
      })),
      startExport: vi.fn(async () => {
        await gate;
        return preview();
      }),
    } as unknown as Bridge;
    const c = new ExportController(bridge, {
      flush: async () => true,
      notify: (t) => notes.push(t),
      newId: () => OP,
      verbose: true,
    });
    const started = c.start("2026-11-05", "notion");
    await vi.advanceTimersByTimeAsync(4100);
    release();
    await started;
    expect(notes[0]).toBe("Notion: export of 2026-11-05 requested");
    expect(notes.filter((t) => t.includes("still working"))).toHaveLength(2);
    expect(notes.some((t) => t.includes("needs your confirmation"))).toBe(true);
    const before = notes.length;
    await vi.advanceTimersByTimeAsync(6000);
    expect(notes).toHaveLength(before);
  } finally {
    vi.useRealTimers();
  }
});

describe("Notion export", () => {
  const notionResult = (state: ExportResult["state"], reason: string | null) =>
    ({ ...result(state, reason), target: "notion" }) as ExportResult;

  it("sends the Notion target and names the destination in messages", async () => {
    const { c, bridge, notes } = setup({
      startExport: vi.fn(async () => ({
        outcome: "result" as const,
        result: notionResult("succeeded", null),
      })),
    });
    await c.start("2026-11-05", "notion");
    expect(bridge.startExport).toHaveBeenCalledWith(
      expect.objectContaining({ target: "notion" }),
    );
    expect(notes).toEqual(["Exported 2026-11-05 to Notion."]);
  });

  it("reports kept-as-text Markdown as a warning after success", async () => {
    const { c, notes } = setup({
      startExport: vi.fn(async () => ({
        outcome: "result" as const,
        result: notionResult("succeeded", "projection_warnings"),
      })),
    });
    await c.start("2026-11-05", "notion");
    expect(notes).toHaveLength(2);
    expect(notes[1]).toMatch(/plain text/);
  });

  it.each([
    ["export_unauthorized", /shared with the integration/],
    ["export_throttled", /few minutes/],
    ["export_network", /could not be reached/],
    ["export_structural", /out-of-order/],
    ["export_disabled", /Notion export is not set up/],
  ])("maps %s to app-authored Notion text", async (code, text) => {
    const { c, notes } = setup({
      startExport: vi.fn(async () => {
        throw new AuthenticError(code, false);
      }),
    });
    await c.start("2026-11-05", "notion");
    expect(notes[0]).toMatch(text);
  });

  it("maps a failed operation's recorded reason to the same wording", async () => {
    const { c, notes } = setup({
      startExport: vi.fn(async () => ({
        outcome: "result" as const,
        result: notionResult("failed", "throttled"),
      })),
    });
    await c.start("2026-11-05", "notion");
    expect(notes[0]).toMatch(/few minutes/);
  });

  it("an unknown Notion outcome names the section, not a file", async () => {
    const { c, notes } = setup({
      startExport: vi.fn(async () => ({
        outcome: "result" as const,
        result: notionResult("uncertain", null),
      })),
    });
    await c.start("2026-11-05", "notion");
    expect(notes[0]).toMatch(/Notion section/);
    expect(c.getSnapshot().target).toBe("notion");
  });
});

// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { EditorView } from "@codemirror/view";
import { AuthenticError, type Bridge } from "../src/application/bridge";
import {
  ResourceCoordinator,
  type SourceChoice,
} from "../src/application/navigation/resources";
import { SessionBudget } from "../src/application/navigation/budget";
import type {
  Source,
  SourceBody,
  SourceCapabilities,
} from "../src/application/navigation/contracts";
import {
  sourceSaved,
  sourceBodyDto,
} from "../src/adapters/navigation-validation";

const d = (c: string) => c.repeat(64);
const ready: SourceCapabilities = {
  read: true,
  save: true,
  trash: false,
  trash_reason: "trash_unverified",
  reason: "ready",
};
const coordinators: ResourceCoordinator[] = [];
afterEach(() => {
  coordinators.forEach((c) => c.dispose());
  coordinators.length = 0;
  document.body.innerHTML = "";
});
async function setup(capabilities: SourceCapabilities = ready) {
  const source = {
    id: "source",
    relative_key: "test.md",
    digest: d("a"),
    import_id: "snap",
    capabilities,
  } as Source;
  const bridge = {
    readSource: vi.fn(async () => ({
      source,
      snapshot_id: "snap",
      body: "base",
      capabilities,
    })) as unknown as ReturnType<typeof vi.fn>,
    saveSource: vi.fn(
      async (_s: string, op: string, _b: string, t: string) => ({
        operation_id: op,
        digest: d("b"),
        size: t.length,
        unchanged: false,
      }),
    ),
    loadAuthentic: vi.fn(async (date: string) => ({
      date,
      exists: false,
      revision: null,
      body: "",
    })),
  };
  const events = {
    notice: vi.fn(),
    sourceGuard: vi.fn(async (): Promise<SourceChoice> => "cancel"),
    savedDatesChanged: vi.fn(),
  };
  const c = new ResourceCoordinator(
    bridge as unknown as Bridge,
    new SessionBudget(),
    events,
  );
  coordinators.push(c);
  const host = document.createElement("div");
  document.body.append(host);
  c.attach(host);
  await c.connect(true, true);
  await c.changeDate("2024-02-29");
  await c.openSource(source);
  c.active!.buffer.setEditable(true);
  const type = (text: string) => {
    const v = EditorView.findFromDOM(
      host.querySelector<HTMLElement>(".cm-editor")!,
    )!;
    v.dispatch({ changes: { from: v.state.doc.length, insert: text } });
  };
  return { c, bridge, events, source, type };
}

describe("source save", () => {
  it("saves_active_source_and_advances_base", async () => {
    const { c, bridge, type } = await setup();
    type(" more");
    expect(await c.saveSource()).toBe(true);
    expect(bridge.saveSource).toHaveBeenCalledWith(
      "source",
      expect.any(String),
      d("a"),
      "base more",
    );
    expect(c.active!.baseDigest).toBe(d("b"));
    expect(c.active!.dirty).toBe(false);
  });
  it("second_save_uses_returned_digest", async () => {
    const { c, bridge, type } = await setup();
    type("1");
    await c.saveSource();
    type("2");
    await c.saveSource();
    expect(bridge.saveSource.mock.calls[1][2]).toBe(d("b"));
  });
  it("external_change_keeps_text_and_reports", async () => {
    const { c, bridge, events, type } = await setup();
    bridge.saveSource.mockRejectedValueOnce(
      new AuthenticError("external_change", false),
    );
    type("x");
    expect(await c.saveSource()).toBe(false);
    expect(c.active!.buffer.getText()).toBe("basex");
    expect(c.active!.dirty).toBe(true);
    expect(c.active!.saveError).toBe("external_change");
    expect(events.notice).toHaveBeenLastCalledWith(
      expect.stringContaining("Changed on disk"),
    );
  });
  it("own_save_does_not_mark_stale", async () => {
    const { c, source, type } = await setup();
    type("x");
    await c.saveSource();
    // A newer import lists the saved file, whose digest is the one the save returned.
    await c.openSource({ ...source, import_id: "newer", digest: d("b") });
    expect(c.active!.stale).toBe(false);
    await c.openSource({ ...source, import_id: "newer", digest: d("c") });
    expect(c.active!.stale).toBe(true);
  });
  it("dirty_tab_saves_after_reconnect", async () => {
    const { c, bridge, type } = await setup();
    type("x");
    await c.connect(false, false);
    await c.connect(true, true);
    expect(c.active!.stale).toBe(true);
    expect(await c.saveSource()).toBe(true);
    expect(bridge.saveSource).toHaveBeenCalledTimes(1);
  });
  it("failed_save_on_stale_tab_keeps_text_and_offers_retry", async () => {
    const { c, bridge, type } = await setup();
    type("x");
    await c.connect(false, false);
    await c.connect(true, true);
    bridge.saveSource.mockRejectedValueOnce(
      new AuthenticError("source_stale", false),
    );
    expect(await c.saveSource()).toBe(false);
    expect(c.active!.buffer.getText()).toBe("basex");
    expect(await c.saveSource()).toBe(true);
  });
  it("retry_reuses_operation_id_until_text_changes", async () => {
    const { c, bridge, type } = await setup();
    bridge.saveSource.mockRejectedValue(
      new AuthenticError("outcome_uncertain", true),
    );
    type("x");
    await c.saveSource();
    await c.saveSource();
    expect(bridge.saveSource.mock.calls[1][1]).toBe(
      bridge.saveSource.mock.calls[0][1],
    );
    type("y");
    await c.saveSource();
    expect(bridge.saveSource.mock.calls[2][1]).not.toBe(
      bridge.saveSource.mock.calls[0][1],
    );
  });
  it("close_waits_for_inflight_source_save and guard Save runs the save", async () => {
    const { c, bridge, events, type } = await setup();
    let release!: () => void;
    bridge.saveSource.mockImplementationOnce(
      (_s: string, op: string) =>
        new Promise((r) => {
          release = () =>
            r({ operation_id: op, digest: d("b"), size: 1, unchanged: false });
        }),
    );
    type("x");
    const saving = c.saveSource();
    const closing = c.closeTab("source:source");
    release();
    expect(await saving).toBe(true);
    expect(await closing).toBe(true);
    expect(events.sourceGuard).not.toHaveBeenCalled();
    const second = await setup();
    second.type("x");
    second.events.sourceGuard.mockResolvedValue("save");
    expect(await second.c.closeTab("source:source")).toBe(true);
    expect(second.bridge.saveSource).toHaveBeenCalledTimes(1);
  });
  it("guard_save_failure_blocks_transition", async () => {
    const { c, bridge, events, type } = await setup();
    bridge.saveSource.mockRejectedValue(
      new AuthenticError("storage_error", false),
    );
    events.sourceGuard.mockResolvedValue("save");
    type("x");
    expect(await c.closeTab("source:source")).toBe(false);
    expect(c.resources.has("source:source")).toBe(true);
  });
  it("capability_off_shows_reason", async () => {
    const { c, bridge, events } = await setup({
      read: true,
      save: false,
      trash: false,
      trash_reason: "trash_unverified",
      reason: "writer_unverified",
    });
    expect(await c.saveSource()).toBe(false);
    expect(bridge.saveSource).not.toHaveBeenCalled();
    expect(events.notice).toHaveBeenLastCalledWith(
      expect.stringContaining("Source Save is off"),
    );
  });
  it("composition_blocks_save", async () => {
    const { c, bridge, type } = await setup();
    type("x");
    Object.defineProperty(c.active!.buffer, "composing", { get: () => true });
    expect(await c.saveSource()).toBe(false);
    expect(bridge.saveSource).not.toHaveBeenCalled();
  });
});

describe("source save contract", () => {
  const body = (caps: object) => ({
    source: {
      id: "s",
      relative_key: "k.md",
      digest: d("a"),
      import_id: "i",
      dating: {
        state: "agreed",
        header: { unix_ms: 0, precision: "milli" },
        filename: { unix_ms: 0, precision: "milli" },
      },
      duplicate_producer_id: false,
      header_invalid: null,
      filename_invalid: null,
      navigation: {
        group_id: "group",
        header: {
          date: "2024-02-29",
          datetime: "2024-02-29T00:00:00",
          offset_seconds: 0,
        },
        filename: {
          date: "2024-02-29",
          datetime: "2024-02-29T00:00:00",
          offset_seconds: 0,
        },
        included: {
          date: "2024-02-29",
          datetime: "2024-02-29T00:00:00",
          offset_seconds: 0,
        },
        authority: null,
      },
      eligible: true,
      capabilities: caps,
    },
    snapshot_id: "i",
    body: "",
    capabilities: caps,
  });
  it("validates_capabilities_shape", () => {
    const ok = sourceBodyDto(
      body({
        read: true,
        save: true,
        trash: false,
        trash_reason: "trash_unverified",
        reason: "ready",
      }),
    ) as SourceBody;
    expect(ok.capabilities.save).toBe(true);
    for (const reason of [
      "producer_unverified",
      "writer_unverified",
      "unsupported_format",
      "checked_on_open",
      "reimport_required",
    ])
      expect(
        sourceBodyDto(
          body({
            read: true,
            save: false,
            trash: false,
            trash_reason: "trash_unverified",
            reason,
          }),
        ).capabilities.reason,
      ).toBe(reason);
  });
  it("validates Trash readiness independently from source saving", () => {
    const caps = {
      read: true,
      save: false,
      reason: "writer_unverified",
      trash: true,
      trash_reason: "ready",
    };
    expect(sourceBodyDto(body(caps)).capabilities.trash).toBe(true);
    for (const invalid of [
      { ...caps, trash_reason: "trash_unverified" },
      { ...caps, trash: false },
      { ...caps, trash_reason: "private_native_error" },
      { ...caps, trash_reason: undefined },
    ])
      expect(() => sourceBodyDto(body(invalid))).toThrow();
  });
  it("rejects_unknown_reason_and_inconsistent_capabilities", () => {
    for (const caps of [
      {
        read: true,
        save: false,
        trash: false,
        trash_reason: "trash_unverified",
        reason: "source_writer_unavailable",
      },
      { read: true, save: false, trash: true, reason: "writer_unverified" },
      {
        read: true,
        save: true,
        trash: false,
        trash_reason: "trash_unverified",
        reason: "writer_unverified",
      },
      {
        read: true,
        save: false,
        trash: false,
        trash_reason: "trash_unverified",
        reason: "ready",
      },
    ])
      expect(() => sourceBodyDto(body(caps))).toThrow();
  });
  it("validates_save_response", () => {
    const ok = {
      operation_id: "op",
      digest: d("a"),
      size: 3,
      unchanged: false,
    };
    expect(sourceSaved(ok).digest).toBe(d("a"));
    expect(() => sourceSaved({ ...ok, digest: "short" })).toThrow();
    expect(() => sourceSaved({ ...ok, unchanged: "no" })).toThrow();
  });
});

it("keeps close blocked if the source is edited while its guarded Save is in flight", async () => {
  const { c, bridge, events, type } = await setup();
  type(" first");
  vi.mocked(events.sourceGuard).mockResolvedValue("save");
  let finish = () => {};
  vi.mocked(bridge.saveSource).mockImplementation(
    (_source, operation_id, _base, body) =>
      new Promise((resolve) => {
        finish = () =>
          resolve({
            operation_id,
            digest: d("b"),
            size: body.length,
            unchanged: false,
          });
      }),
  );
  const closing = c.closeApp();
  for (
    let i = 0;
    i < 20 && !vi.mocked(bridge.saveSource).mock.calls.length;
    i++
  )
    await Promise.resolve();
  expect(bridge.saveSource).toHaveBeenCalledTimes(1);
  type(" later");
  finish();
  expect(await closing).toBe(false);
  expect(c.active!.dirty).toBe(true);
  expect(c.active!.buffer.getText()).toContain(" later");
});

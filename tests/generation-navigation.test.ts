import { SessionBudget } from "../src/application/navigation/budget";
import { GlobalMessagesStore } from "../src/application/global-messages/store";
// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Bridge } from "../src/application/bridge";
import { NavigationModel } from "../src/application/navigation/model";
import { generationPreparation } from "../src/application/generation/navigation";
import { navigationFake } from "./navigation-fake";
import { generationSource } from "./generation-fixtures";
const models: NavigationModel[] = [];
afterEach(() => {
  for (const m of models) m.dispose();
  models.length = 0;
});
async function setup(changeClean = false) {
  const day = new Date().toLocaleDateString("sv");
  let serial = 0,
    digest = "a".repeat(64),
    other = "a".repeat(64);
  const sources = () => [
    generationSource(day, "source", digest, `import${serial}`),
    generationSource(day, "clean", other, `import${serial}`),
  ];
  const bridge = {
    ...navigationFake(),
    listAuthentic: async () => ({ items: [], next_cursor: null }),
    loadAuthentic: async (date: string) => ({
      date,
      exists: false,
      revision: null,
      body: "",
    }),
    importDate: vi.fn(async () => {
      serial++;
      return {
        import_id: `import${serial}`,
        job: { id: `import${serial}`, sequence: 1, state: "succeeded" },
      };
    }),
    listSources: vi.fn(async () => ({
      import_id: `import${serial}`,
      date: day,
      zone: "Asia/Seoul",
      timezone_database: "test",
      config_generation: 1,
      completed_at: 1,
      items: sources().map((source) => ({
        kind: "included",
        source,
        instant: { unix_ms: 0, precision: "milli" },
        misplaced: false,
      })),
      next_cursor: null,
      incomplete: false,
      groups: [{ id: "g", alias: "Diary", type: "narihito" }],
      reimport_required: false,
    })),
    readSource: async () => ({
      source: sources()[0],
      snapshot_id: `import${serial}`,
      body: "base",
      capabilities: sources()[0].capabilities,
    }),
    saveSource: vi.fn(
      async (_id: string, op: string, _base: string, body: string) => {
        digest = "b".repeat(64);
        if (changeClean) other = "c".repeat(64);
        return {
          operation_id: op,
          digest,
          size: body.length,
          unchanged: false,
        };
      },
    ),
  } as unknown as Bridge;
  const model = new NavigationModel(
    bridge,
    new GlobalMessagesStore(),
    new SessionBudget(),
  );
  models.push(model);
  await model.connect(true, "i", 1, true);
  await model.resources.openSource(sources()[0]);
  const resource = model.resources.active!;
  resource.buffer.setText("saved body");
  resource.dirty = true;
  const preparation = generationPreparation(model, () => false);
  return { day, model, resource, bridge, preparation };
}
describe("generation navigation preparation", () => {
  it("saves exact dirty buffers and reselects only matching fresh saved observations", async () => {
    const { day, model, resource, bridge, preparation } = await setup();
    const selected = preparation.selected(day);
    const dirty = preparation.dirty(selected);
    const old = selected[0].import_id;
    expect(dirty).toHaveLength(1);
    expect(await preparation.flushAuthentic(day)).toBe(true);
    const fresh = await preparation.saveAndRefresh(
      day,
      selected,
      dirty,
      new AbortController().signal,
    );
    expect(bridge.saveSource).toHaveBeenCalledWith(
      "source",
      expect.any(String),
      "a".repeat(64),
      "saved body",
    );
    expect(fresh[0].import_id).not.toBe(old);
    expect(fresh[0].digest).toBe("b".repeat(64));
    expect(resource.dirty).toBe(false);
    expect(model.imports.eligible(day).map((s) => s.id)).toEqual([
      "source",
      "clean",
    ]);
  });
  it("refuses a clean selected source that changes while another source is saved", async () => {
    const { day, preparation, model } = await setup(true);
    const selected = preparation.selected(day);
    await expect(
      preparation.saveAndRefresh(
        day,
        selected,
        preparation.dirty(selected),
        new AbortController().signal,
      ),
    ).rejects.toMatchObject({ code: "reimport_required" });
    expect(model.imports.eligible(day).map((s) => s.id)).not.toContain("clean");
  });
  it("cancellation before save sends no source mutation", async () => {
    const { day, preparation, bridge } = await setup();
    const selected = preparation.selected(day);
    const abort = new AbortController();
    abort.abort();
    await expect(
      preparation.saveAndRefresh(
        day,
        selected,
        preparation.dirty(selected),
        abort.signal,
      ),
    ).rejects.toMatchObject({ code: "cancelled" });
    expect(bridge.saveSource).not.toHaveBeenCalled();
  });
});

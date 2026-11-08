// @vitest-environment node
import { afterEach, describe, expect, it, vi } from "vitest";
import { AuthenticError, type Bridge } from "../src/application/bridge";
import { ResourceCoordinator } from "../src/application/navigation/resources";
import { SessionBudget } from "../src/application/navigation/budget";
import { FakeBuffer, FakeBuffers } from "./fake-buffers";
import { navigationFake } from "./navigation-fake";
import { generationSource } from "./generation-fixtures";
const day = "2026-11-06";
const resources: ResourceCoordinator[] = [];
afterEach(() => resources.splice(0).forEach((r) => r.dispose()));
async function setup() {
  const source = generationSource(day);
  const bridge = {
    ...navigationFake(),
    readSource: vi.fn(async () => ({
      source,
      snapshot_id: source.import_id,
      capabilities: source.capabilities,
      body: "original",
    })),
    saveSource: vi.fn(async (_source: string, id: string) => ({
      operation_id: id,
      digest: "b".repeat(64),
      size: 4,
      unchanged: false,
    })),
  } as unknown as Bridge;
  const buffers = new FakeBuffers();
  const events = {
    notice: vi.fn(),
    sourceGuard: vi.fn(async () => "discard" as const),
    savedDatesChanged: vi.fn(),
  };
  const coordinator = new ResourceCoordinator(
    bridge,
    new SessionBudget(),
    events,
    buffers,
  );
  resources.push(coordinator);
  await coordinator.connect(true, true);
  await coordinator.changeDate(day);
  await coordinator.openSource(source);
  return {
    coordinator,
    bridge,
    buffers,
    events,
    source,
    buffer: coordinator.active!.buffer as FakeBuffer,
  };
}
describe("resource policy without an editor runtime", () => {
  it("retries uncertain saves with the same operation and keeps edits made during acknowledgement", async () => {
    const { coordinator, bridge, buffer } = await setup();
    buffer.edit("first");
    vi.mocked(bridge.saveSource).mockRejectedValueOnce(
      new AuthenticError("unavailable", true),
    );
    expect(await coordinator.saveSource()).toBe(false);
    let acknowledge!: (
      result: Awaited<ReturnType<Bridge["saveSource"]>>,
    ) => void;
    vi.mocked(bridge.saveSource).mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          acknowledge = resolve;
        }),
    );
    const retry = coordinator.saveSource();
    const calls = vi.mocked(bridge.saveSource).mock.calls;
    expect(calls[1]).toEqual(calls[0]);
    buffer.edit("newer");
    acknowledge({
      operation_id: calls[1][1],
      digest: "b".repeat(64),
      size: 5,
      unchanged: false,
    });
    expect(await retry).toBe(true);
    expect(buffer.getText()).toBe("newer");
    expect(coordinator.active!.dirty).toBe(true);
    expect(await coordinator.saveSource()).toBe(true);
    expect(vi.mocked(bridge.saveSource).mock.calls[2][2]).toBe("b".repeat(64));
    expect(coordinator.active!.dirty).toBe(false);
  });
  it("guards composition and source recovery while retaining resource identity", async () => {
    const { coordinator, bridge, buffers, buffer, source } = await setup();
    const resource = coordinator.active!;
    buffer.edit("unsaved");
    buffer.composing = true;
    expect(await coordinator.closeTab(resource.key)).toBe(false);
    expect(await coordinator.saveSource()).toBe(false);
    buffer.composing = false;
    coordinator.reserveSourceDelete(source.id, "operation");
    expect(await coordinator.saveSource()).toBe(false);
    expect(bridge.saveSource).not.toHaveBeenCalled();
    coordinator.sourceMoved(source.id);
    expect(coordinator.active).toBe(resource);
    expect(resource.detached).toBe(true);
    expect(buffer.getText()).toBe("unsaved");
    expect(buffer.editable).toBe(false);
    expect(buffers.active).toBe(buffer);
    coordinator.reconcileSourceReservations(new Set());
    expect(coordinator.sourceReserved(source.id)).toBe(false);
    expect(await coordinator.closeTab(resource.key)).toBe(true);
    expect(buffer.disposed).toBe(true);
    expect(buffers.active).not.toBe(buffer);
  });
});

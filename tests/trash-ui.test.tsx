import { connectFromSettings } from "./server-connection";
// @vitest-environment jsdom
import { useEffect, useReducer } from "react";
import { afterEach, describe, it, expect, vi } from "vitest";
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  within,
} from "@testing-library/react";
import { TrashCoordinator } from "../src/application/trash/coordinator";
import { SessionBudget } from "../src/application/navigation/budget";
import type { Bridge } from "../src/application/bridge";
import { TrashDialog } from "../src/components/TrashDialog";
import { TrashRecovery } from "../src/components/TrashRecovery";
import { TrashActivity } from "../src/components/TrashActivity";
import { App } from "../src/components/App";
import { settingsFake } from "./settings-fake";
import { day, sourceId, result, trashFake, deferred, uuid } from "./trash-fake";
import type { TrashPreview } from "../src/application/trash/contracts";

const controllers: TrashCoordinator[] = [];
afterEach(() => {
  cleanup();
  controllers.splice(0).forEach((c) => c.dispose());
  vi.useRealTimers();
});
async function setup() {
  const fake = trashFake();
  let serial = 0,
    clock = 0;
  const coordinator = new TrashCoordinator(
    fake.bridge as unknown as Bridge,
    new SessionBudget(),
    {
      prepare: vi.fn(async () => null),
      reserve: vi.fn(),
      release: vi.fn(),
      reconcileReservations: vi.fn(),
      moved: vi.fn(),
      notice: vi.fn(),
    },
    () => uuid(++serial),
    () => clock,
    10,
  );
  controllers.push(coordinator);
  await coordinator.connect(true);
  return {
    ...fake,
    coordinator,
    setClock: (n: number) => {
      clock = n;
    },
  };
}
function Surface({ coordinator }: { coordinator: TrashCoordinator }) {
  const [, update] = useReducer((n) => n + 1, 0);
  useEffect(() => coordinator.subscribe(update), [coordinator]);
  return (
    <>
      <button onClick={() => void coordinator.start(sourceId, "snippet")}>
        Open Trash preview
      </button>
      <TrashDialog coordinator={coordinator} />
      <TrashActivity coordinator={coordinator} />
    </>
  );
}
async function open(coordinator: TrashCoordinator) {
  await act(async () => {
    await coordinator.start(sourceId, "snippet");
  });
}

describe("Trash confirmation and recovery UI", () => {
  it("explains connected, checking, failed and read-only recovery states", async () => {
    const e = await setup();
    const view = render(
      <TrashRecovery coordinator={e.coordinator} onClose={() => {}} />,
    );
    expect(screen.getByText(/No recovery action is needed/)).toBeTruthy();
    e.coordinator.discovering = true;
    view.rerender(
      <TrashRecovery coordinator={e.coordinator} onClose={() => {}} />,
    );
    expect(screen.getByText(/Checking the server for unfinished/)).toBeTruthy();
    expect(
      screen.getByRole<HTMLButtonElement>("button", {
        name: "Check unfinished moves",
      }).disabled,
    ).toBe(true);
    e.coordinator.discovering = false;
    e.coordinator.discoveryError = "unavailable";
    e.coordinator.writable = false;
    view.rerender(
      <TrashRecovery coordinator={e.coordinator} onClose={() => {}} />,
    );
    expect(screen.getByText(/The check could not finish/)).toBeTruthy();
    expect(
      screen.getByText(/Storage is read-only or unavailable/),
    ).toBeTruthy();
  });
  it("moves focus to Cancel when a slow preparation becomes a confirmation", async () => {
    const e = await setup(),
      pending = deferred<TrashPreview>();
    e.bridge.previewSourceDelete.mockImplementationOnce(() => pending.promise);
    render(<Surface coordinator={e.coordinator} />);
    let started!: Promise<void>;
    await act(async () => {
      started = e.coordinator.start(sourceId, "snippet");
      await Promise.resolve();
    });
    expect(document.activeElement?.textContent).toBe("Cancel");
    await act(async () => {
      pending.resolve({
        token: "fresh",
        plan_hash: "c".repeat(64),
        expected_checkpoint: 0,
        expires_in_ms: 60_000,
        ordinals: [0, 1],
        members: result(uuid(1), "running").members,
      });
      await started;
    });
    expect(document.activeElement).toBe(
      screen.getByRole("button", { name: "Cancel" }),
    );
    expect(e.bridge.confirmSourceDelete).not.toHaveBeenCalled();
  });
  it("shows exact members, defaults to Cancel, traps focus and restores its invoker", async () => {
    const e = await setup();
    render(<Surface coordinator={e.coordinator} />);
    const invoker = screen.getByRole("button", { name: "Open Trash preview" });
    invoker.focus();
    await open(e.coordinator);
    const dialog = screen.getByRole("dialog", { name: "Move source to Trash" }),
      cancel = within(dialog).getByRole("button", { name: "Cancel" }),
      confirm = within(dialog).getByRole("button", {
        name: "Move listed files to Trash",
      });
    expect(document.activeElement).toBe(cancel);
    expect(
      within(dialog).getByText(result().members[0].relative_label),
    ).toBeTruthy();
    expect(
      within(dialog).getByText(result().members[1].relative_label),
    ).toBeTruthy();
    fireEvent.keyDown(cancel, { key: "Tab", shiftKey: true });
    expect(document.activeElement).toBe(confirm);
    fireEvent.keyDown(confirm, { key: "Tab" });
    expect(document.activeElement).toBe(cancel);
    fireEvent.keyDown(cancel, { key: "Escape", isComposing: true });
    expect(screen.getByRole("dialog")).toBeTruthy();
    await act(async () => {
      fireEvent.keyDown(cancel, { key: "Escape" });
    });
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(document.activeElement).toBe(invoker);
    expect(e.bridge.confirmSourceDelete).not.toHaveBeenCalled();
  });
  it("cancels a slow read-only preparation and ignores its late response", async () => {
    const e = await setup(),
      pending = deferred<TrashPreview>();
    e.bridge.previewSourceDelete.mockImplementationOnce(() => pending.promise);
    render(<Surface coordinator={e.coordinator} />);
    let started!: Promise<void>;
    await act(async () => {
      started = e.coordinator.start(sourceId, "snippet");
      await Promise.resolve();
    });
    expect(screen.getByRole("status").textContent).toContain(
      "Checking the source",
    );
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    await act(async () => {
      pending.resolve({
        token: "late",
        plan_hash: "c".repeat(64),
        expected_checkpoint: 0,
        expires_in_ms: 60_000,
        ordinals: [0, 1],
        members: result(uuid(1), "running").members,
      });
      await started;
    });
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(e.bridge.confirmSourceDelete).not.toHaveBeenCalled();
  });
  it("disables an expired confirmation and requires an explicit new preview and confirmation", async () => {
    vi.useFakeTimers();
    const e = await setup();
    render(<Surface coordinator={e.coordinator} />);
    await open(e.coordinator);
    e.setClock(60_000);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(500);
    });
    expect(
      screen.getByRole<HTMLButtonElement>("button", {
        name: "Move listed files to Trash",
      }).disabled,
    ).toBe(true);
    expect(screen.getByRole("alert").textContent).toContain("expired");
    await act(async () => {
      await e.coordinator.refreshPreview();
    });
    expect(e.bridge.confirmSourceDelete).not.toHaveBeenCalled();
    await act(async () => {
      fireEvent.click(
        screen.getByRole("button", { name: "Move listed files to Trash" }),
      );
    });
    expect(e.bridge.confirmSourceDelete).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(
      screen.getByLabelText("Move to Trash activity").textContent,
    ).toContain("Completed");
  });
  it("describes missing expected audio without claiming it will move", async () => {
    const e = await setup();
    const members = result(uuid(1), "running").members;
    members[0].retry_eligible = true;
    members[1] = { ...members[1], state: "missing", retry_eligible: false };
    e.bridge.previewSourceDelete.mockResolvedValueOnce({
      token: "preview",
      plan_hash: "c".repeat(64),
      expected_checkpoint: 0,
      expires_in_ms: 60_000,
      ordinals: [0, 1],
      members,
    });
    render(<Surface coordinator={e.coordinator} />);
    await open(e.coordinator);
    expect(screen.getByText("Missing · no file will be moved")).toBeTruthy();
    expect(screen.getAllByText("Will move to Trash")).toHaveLength(1);
  });
  it("keeps partial/uncertain results inspectable after the source row disappears", async () => {
    const e = await setup(),
      partial = result(uuid(20), "needs_confirmation"),
      uncertain = result(uuid(21), "uncertain");
    e.operations.set(partial.operation_id, partial);
    e.operations.set(uncertain.operation_id, uncertain);
    await e.coordinator.connect(false);
    await e.coordinator.connect(true);
    render(<Surface coordinator={e.coordinator} />);
    const items = screen
      .getByLabelText("Move to Trash activity")
      .querySelectorAll("li");
    expect(items).toHaveLength(2);
    await act(async () => {
      fireEvent.click(
        within(items[0]).getByRole("button", { name: "Inspect" }),
      );
    });
    expect(
      screen.getByRole("button", { name: "Preview remaining files" }),
    ).toBeTruthy();
    await act(async () => {
      fireEvent.click(
        screen.getByRole("button", { name: "Preview remaining files" }),
      );
      await Promise.resolve();
    });
    expect(e.bridge.confirmSourceDelete).not.toHaveBeenCalled();
    expect(e.bridge.previewSourceDelete.mock.calls[0][1].operation_id).toBe(
      partial.operation_id,
    );
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    await act(async () => {
      e.coordinator.inspect(uncertain.operation_id);
    });
    expect(screen.getByRole("alert").textContent).toContain(
      "Automatic retries are disabled",
    );
    expect(
      screen.queryByRole("button", { name: "Preview remaining files" }),
    ).toBeNull();
  });
});

describe("Explorer Trash action", () => {
  it("uses the focused snippet for the registered shortcut and keeps cancelled selections", async () => {
    const fake = trashFake(),
      settings = settingsFake();
    const status = {
      instance_id: "instance",
      workspace_key: "workspace",
      server_version: "1.0.0",
      api_version: 1 as const,
      api_minor: 0,
      storage_ready: true,
      capabilities: ["status", "sessions", "jobs", "events"],
    };
    fake.bridge.getCalendar = vi.fn(async () => ({
      anchor: day,
      today: day,
      year: 2024,
      month: 2,
      days: Array.from({ length: 29 }, (_, i) => ({
        date: `2024-02-${String(i + 1).padStart(2, "0")}`,
        day: i + 1,
        weekday: (i + 4) % 7,
      })),
      previous_month: null,
      next_month: null,
    }));
    const bridge = {
      ...fake.bridge,
      ...settings,
      metadata: async () => ({ version: "1.0.0" }),
      connect: async () => status,
      status: async () => status,
      watch: async () => async () => {},
      listAuthentic: async () => ({ items: [], next_cursor: null }),
    } as unknown as Bridge;
    render(<App bridge={bridge} />);
    connectFromSettings();
    await screen.findByText("Connected");
    fireEvent.click(screen.getByRole("button", { name: "Go back" }));
    const recoveryButton = screen.getByRole("button", {
      name: "Trash Recovery",
    });
    fireEvent.click(recoveryButton);
    await screen.findByText("No unfinished moves.");
    await act(async () => {
      const partial = result(uuid(22), "needs_confirmation");
      partial.source_id = "c".repeat(32);
      fake.operations.set(uuid(22), partial);
      fireEvent.click(
        screen.getByRole("button", { name: "Check unfinished moves" }),
      );
    });
    fireEvent.click(await screen.findByRole("button", { name: "Inspect" }));
    expect(screen.getAllByRole("dialog")).toHaveLength(1);
    expect(screen.getByRole("dialog", { name: "Trash result" })).toBeTruthy();
    fireEvent.click(
      screen.getByRole("button", { name: "Preview remaining files" }),
    );
    await screen.findByRole("dialog", { name: "Move source to Trash" });
    expect(fake.bridge.confirmSourceDelete).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(screen.getByRole("dialog", { name: "Trash Recovery" })).toBeTruthy();
    fireEvent.click(
      screen.getByRole("button", { name: "Close Trash Recovery" }),
    );
    fake.bridge.previewSourceDelete.mockClear();
    fireEvent.click(screen.getByRole("button", { name: "Snippet tab" }));
    const move = await screen.findAllByRole("button", {
      name: /^Move .+ to Trash$/,
    });
    await act(async () => {
      move[1].focus();
    });
    fireEvent.keyDown(window, {
      key: "Backspace",
      code: "Backspace",
      metaKey: true,
    });
    await screen.findByRole("dialog", { name: "Move source to Trash" });
    expect(fake.bridge.previewSourceDelete.mock.calls[0][0]).toBe(
      "b".repeat(32),
    );
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(fake.bridge.confirmSourceDelete).not.toHaveBeenCalled();
    expect(
      screen.getAllByRole<HTMLInputElement>("checkbox").every((c) => c.checked),
    ).toBe(true);
  });
});

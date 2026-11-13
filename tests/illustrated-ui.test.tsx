// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import {
  act,
  cleanup,
  render,
  renderHook,
  screen,
  fireEvent,
} from "@testing-library/react";
import { useAppControllers } from "../src/components/useAppControllers";
import { ImageEnginesCard } from "../src/components/illustrated/ImageEnginesCard";
import { AIActivityDetails } from "../src/components/ai-activities/AIActivityDetails";
import { AIActivityStatus } from "../src/components/ai-activities/AIActivityStatus";
import { createAppActions } from "../src/application/actions/app-actions";
import { AIActivitiesStore } from "../src/application/ai-activities/store";
import { navigationFake } from "./navigation-fake";
import type { Bridge, Connection } from "../src/application/bridge";
import {
  illustratedDate as date,
  illustratedEntry,
  illustratedProgress,
  imageEngines,
} from "./illustrated-fixtures";
afterEach(cleanup);
it("uses Authentic chat context for an Illustrated tab and disables its save/export actions", async () => {
  const bridge = {
    ...navigationFake(),
    listIllustrated: async () => [date],
    readIllustrated: async () => illustratedEntry(),
    listAuthentic: async () => ({ items: [], next_cursor: null }),
    loadAuthentic: async (d: string) => ({
      date: d,
      exists: true,
      revision: { sequence: 1, sha256: "a".repeat(64) },
      body: "Authentic context",
    }),
  } as unknown as Bridge;
  const { result } = renderHook(() => useAppControllers(bridge));
  await act(async () => {
    await result.current.model.connect(true, "i", 1, true);
    await result.current.model.chooseDate(date);
    await result.current.model.resources.openIllustrated(date);
  });
  expect(result.current.chatContext().document?.body).toBe("Authentic context");
  expect(result.current.chatContext().document?.kind).toBe("authentic");
  const actions = createAppActions({
    ...result.current,
    connection: () =>
      ({
        kind: "connected",
        status: { storage_ready: true, capabilities: ["generation"] },
      }) as Connection,
    user: () => undefined,
    openPalette: () => {},
    openSettings: () => {},
  });
  expect(actions.handlers["editor.save"].reason).toContain("read-only");
  expect(actions.handlers["export.start"].reason).toContain(
    "do not support export",
  );
  expect(actions.handlers["export.start.notion"].reason).toContain(
    "do not support export",
  );
  expect(result.current.model.resources.active?.session).toBeUndefined();
});
it("shows Illustrated photo progress and stopping in the existing activity details/status without invented agent metadata", async () => {
  const store = new AIActivitiesStore();
  const running = {
    id: "job",
    sequence: 1,
    state: "running" as const,
    progress: illustratedProgress({
      phase: "photo",
      photo_index: 2,
      photo_count: 3,
    }),
  };
  store.event({
    kind: "job_snapshot",
    instance_id: "i",
    session_id: "s",
    jobs: [running],
    first: true,
    complete: true,
  });
  const cancel = vi.fn(async () => {});
  render(
    <>
      <AIActivityDetails
        store={store}
        view={async () => true}
        cancel={cancel}
        discard={async () => {}}
      />
      <AIActivityStatus store={store} />
    </>,
  );
  expect(screen.getByText("Creating photo 2 of 3")).toBeTruthy();
  expect(screen.getByLabelText("AI Activities status").textContent).toContain(
    "photo 2 of 3",
  );
  expect(screen.queryByText(/Profile:/)).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Cancel AI run" }));
  expect(cancel).toHaveBeenCalledWith(running);
  act(() => {
    store.observe({
      ...running,
      sequence: 2,
      progress: illustratedProgress({
        phase: "stopping",
        photo_index: 2,
        photo_count: 3,
      }),
    });
  });
  expect(
    (screen.getByRole("button", { name: "Cancel AI run" }) as HTMLButtonElement)
      .disabled,
  ).toBe(true);
  expect(screen.getByLabelText("AI Activities status").textContent).toContain(
    "Stopping",
  );
  store.dispose();
});
it("engine status identifies the effective choice and automatically checks on Settings entry and exposes a prerequisite-only retry", () => {
  const state = { engines: imageEngines(), checking: null, engineError: false };
  const checkEngine = vi.fn(async () => {});
  const checkEngines = vi.fn(async () => {});
  const controller = {
    state,
    subscribe: () => () => {},
    checkEngine,
    checkEngines,
  };
  render(
    <ImageEnginesCard
      controller={
        controller as unknown as import("../src/application/illustrated/controller").IllustratedController
      }
      connected
    />,
  );
  expect(checkEngines).toHaveBeenCalledOnce();
  expect(screen.getByText(/Selected for Illustrated/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Check Local" }));
  expect(checkEngine).toHaveBeenCalledWith("local");
});

import { App } from "../src/components/App";
import { navigationFake } from "./navigation-fake";
import { settingsFake } from "./settings-fake";
import { generationSource } from "./generation-fixtures";
import type { Bridge } from "../src/application/bridge";
// @vitest-environment jsdom
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  within,
  waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  CandidateBanner,
  GenerationControls,
  GenerationDialog,
} from "../src/components/GenerationPane";
import { GenerationController } from "../src/application/generation/controller";
import { generationServer } from "./generation-fake";
import {
  candidateKey,
  useCandidateTabs,
} from "../src/components/candidateTabs";
import { useGeneration } from "../src/components/useGeneration";
import { ResourceCoordinator } from "../src/application/navigation/resources";
import { SessionBudget } from "../src/application/navigation/budget";
import { useSyncExternalStore } from "react";
function coordinator() {
  const r = new ResourceCoordinator({} as Bridge, new SessionBudget(), {
    sourceGuard: async () => "cancel",
    notice: vi.fn(),
    savedDatesChanged: vi.fn(),
  } as never);
  const host = document.createElement("div");
  document.body.append(host);
  r.attach(host);
  return r;
}
/** Candidate tabs as the app wires them: one tab per slot plus the active tab's banner. */
function Tabs({ c, r }: { c: GenerationController; r: ResourceCoordinator }) {
  useCandidateTabs(c, r);
  useSyncExternalStore(r.subscribe, () => `${r.resources.size}:${r.activeKey}`);
  return (
    <>
      <div role="tablist">
        {[...r.resources.values()].map((x) => (
          <button key={x.key} role="tab">
            {x.label}
          </button>
        ))}
      </div>
      <CandidateBanner controller={c} resources={r} />
    </>
  );
}
const controllers: GenerationController[] = [];
async function setup() {
  const server = generationServer();
  const c = new GenerationController(server.bridge, server.preparation);
  controllers.push(c);
  c.connect(true);
  await c.loadProfiles();
  return { server, c };
}
beforeEach(() => vi.useFakeTimers());
afterEach(() => {
  cleanup();
  controllers.forEach((c) => c.dispose());
  controllers.length = 0;
  vi.useRealTimers();
});
function Controls({ c }: { c: GenerationController }) {
  useGeneration(c);
  return (
    <>
      <GenerationControls
        controller={c}
        date="2026-11-06"
        locale="ko-KR"
        count={3}
        reason={c.reason("2026-11-06", "ko-KR")}
      />
      <GenerationDialog controller={c} />
    </>
  );
}
describe("generation interaction", () => {
  it("opens an accessible dirty dialog and sends nothing until its choice", async () => {
    const { server, c } = await setup();
    server.dirty(true);
    render(<Controls c={c} />);
    fireEvent.click(
      screen.getByRole("button", { name: "Generate", exact: true }),
    );
    expect(screen.getByRole("button", { name: "Submitting…" })).toHaveProperty(
      "disabled",
      true,
    );
    const dialog = screen.getByRole("dialog", {
      name: "Unsaved selected sources",
    });
    expect(
      within(dialog).getByRole("button", { name: "Cancel", exact: true }),
    ).toBe(document.activeElement);
    expect(server.bridge.startGeneration).not.toHaveBeenCalled();
    await act(async () => {
      fireEvent.click(
        within(dialog).getByRole("button", { name: "Exclude", exact: true }),
      );
    });
    expect(server.bridge.startGeneration).toHaveBeenCalledTimes(1);
    expect(server.input[0].selections).toEqual([
      { import_id: "i", source_id: "b" },
    ]);
  });
  it("opens one read-only tab per candidate, streams then finalizes text without moving focus", async () => {
    const { server, c } = await setup();
    const r = coordinator();
    await c.start("2026-11-06", "ko-KR", 3);
    render(
      <>
        <button>Editor focus</button>
        <Tabs c={c} r={r} />
      </>,
    );
    const focus = screen.getByRole("button", { name: "Editor focus" });
    focus.focus();
    expect(screen.getAllByRole("tab")).toHaveLength(3);
    expect(
      screen.getByText(/Candidate 1 · running · provisional/),
    ).not.toBeNull();
    const [job] = [...c.state.jobs.values()];
    const key = candidateKey(job.job.id, 1);
    await act(async () => {
      server.snapshot().provisional = "preview expanded";
      await c.refresh();
    });
    expect(r.resources.get(key)!.buffer.getText()).toBe("preview expanded");
    await act(async () => {
      server.finish();
      await c.refresh();
    });
    expect(r.resources.get(key)!.buffer.getText()).toBe("draft");
    expect(r.activeKey).toBe(key);
    expect(document.activeElement).toBe(focus);
    // An explicit user choice moves the active tab.
    await r.openCandidate(candidateKey(job.job.id, 2));
    expect(r.activeKey).toBe(candidateKey(job.job.id, 2));
  });
  it("announces completion once, labels stale results and provides keyboard cancellation", async () => {
    const { server, c } = await setup();
    const r = coordinator();
    render(
      <>
        <Controls c={c} />
        <Tabs c={c} r={r} />
      </>,
    );
    await act(async () => {
      await c.start("2026-11-06", "ko-KR", 3);
    });
    const cancel = screen.getByRole("button", { name: "Cancel generation" });
    cancel.focus();
    await act(async () => {
      fireEvent.click(cancel);
    });
    expect(server.bridge.cancelJob).toHaveBeenCalledTimes(1);
    const status = screen.getByRole("status");
    expect(status.textContent).toContain("1 cancelled");
    await act(async () => {
      server.snapshot().stale_baseline = true;
      await c.refresh(true);
    });
    expect(
      screen.getByText("Entry changed — regenerate to review"),
    ).not.toBeNull();
    expect(status.textContent).toContain("1 cancelled");
  });
});

it("a new App request uses the saved generation count and actual Include selection", async () => {
  vi.useRealTimers();
  const server = generationServer(),
    settings = settingsFake({ writing_locale: "ko-KR" });
  const day = new Date().toLocaleDateString("sv"),
    source = generationSource(day);
  const status = {
    instance_id: "i",
    workspace_key: "w",
    server_version: "1.0.0",
    api_version: 1 as const,
    api_minor: 0,
    storage_ready: true,
    capabilities: ["status", "sessions", "jobs", "events", "generation"],
  };
  const bridge = {
    ...navigationFake(),
    ...server.bridge,
    ...settings,
    metadata: async () => ({ version: "1.0.0" }),
    connect: async () => status,
    status: async () => status,
    watch: async () => async () => {},
    listAuthentic: async () => ({ items: [], next_cursor: null }),
    loadAuthentic: async (date: string) => ({
      date,
      exists: false,
      revision: null,
      body: "",
    }),
    saveAuthentic: async () => {
      throw new Error("unexpected save");
    },
    createEmptyAuthentic: async () => {
      throw new Error("unexpected creation");
    },
    refreshAuthentic: async () => ({ files: 0, problems: 0 }),
    getOperation: async () => {
      throw new Error("unexpected operation");
    },
    listSources: async () => ({
      import_id: "import",
      date: day,
      zone: "Asia/Seoul",
      timezone_database: "test",
      config_generation: 1,
      completed_at: 1,
      items: [
        {
          kind: "included",
          source,
          instant: { unix_ms: 0, precision: "milli" },
          misplaced: false,
        },
      ],
      next_cursor: null,
      incomplete: false,
      groups: [{ id: "g", alias: "Diary", type: "narihito" }],
      reimport_required: false,
    }),
  } as Bridge;
  const app = render(<App bridge={bridge} />);
  await waitFor(() =>
    expect(
      screen
        .getByRole("button", { name: "Generate", exact: true })
        .hasAttribute("disabled"),
    ).toBe(false),
  );
  fireEvent.click(
    screen.getByRole("button", { name: "Settings", exact: true }),
  );
  fireEvent.click(
    screen.getByRole("button", { name: "Generation", exact: true }),
  );
  fireEvent.change(screen.getByLabelText("Candidates per generation"), {
    target: { value: "5" },
  });
  await waitFor(() => expect(settings.user.generation_count).toBe(5));
  fireEvent.click(screen.getByRole("button", { name: "Go back" }));
  fireEvent.click(
    screen.getByRole("button", { name: "Generate", exact: true }),
  );
  await waitFor(() =>
    expect(server.bridge.startGeneration).toHaveBeenCalledTimes(1),
  );
  expect(server.input[0].count).toBe(5);
  expect(server.input[0].selections).toEqual([
    { import_id: "import", source_id: "source" },
  ]);
  await act(async () => app.unmount());
});

it("discarding results closes the candidate tabs", async () => {
  const { server, c } = await setup();
  const r = coordinator();
  await c.start("2026-11-06", "ko-KR", 2);
  render(<Tabs c={c} r={r} />);
  expect(screen.getAllByRole("tab")).toHaveLength(2);
  await act(async () => {
    server.finish();
    await c.refresh();
  });
  const [job] = [...c.state.jobs.values()];
  await act(async () => {
    await c.discard(job.job.id);
  });
  expect(screen.queryAllByRole("tab")).toHaveLength(0);
});

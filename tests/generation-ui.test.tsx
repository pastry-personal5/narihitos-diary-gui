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
  CandidateCards,
  GenerationControls,
  GenerationDialog,
} from "../src/components/GenerationPane";
import { GenerationController } from "../src/application/generation/controller";
import { generationServer } from "./generation-fake";
import { useGeneration } from "../src/components/useGeneration";
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
  it("provisional_then_final and cards_survive_date_change without focus or selection changes", async () => {
    const { server, c } = await setup();
    await c.start("2026-11-06", "ko-KR", 3);
    const app = render(
      <>
        <button>Editor focus</button>
        <CandidateCards controller={c} date="2026-11-06" />
      </>,
    );
    const focus = screen.getByRole("button", { name: "Editor focus" });
    focus.focus();
    expect(
      screen.getByText("Candidate 1 · running · provisional"),
    ).not.toBeNull();
    const preview = screen.getByLabelText("Candidate 1 provisional text");
    const text = preview.firstChild!;
    document.getSelection()!.setBaseAndExtent(text, 1, text, 4);
    await act(async () => {
      server.snapshot().provisional = "preview expanded";
      await c.refresh();
    });
    expect(document.activeElement).toBe(focus);
    expect(document.getSelection()!.toString()).toBe("rev");
    await act(async () => {
      server.finish();
      await c.refresh();
    });
    expect(screen.getByLabelText("Candidate 1 text").textContent).toBe("draft");
    expect(screen.getByText("Candidate 2 · failed")).not.toBeNull();
    expect(document.activeElement).toBe(focus);
    app.rerender(
      <>
        <button>Editor focus</button>
        <CandidateCards controller={c} date="2026-11-07" />
      </>,
    );
    expect(screen.queryByLabelText("Candidate 1 text")).toBeNull();
    app.rerender(
      <>
        <button>Editor focus</button>
        <CandidateCards controller={c} date="2026-11-06" />
      </>,
    );
    expect(screen.getByLabelText("Candidate 1 text").textContent).toBe("draft");
    expect(server.bridge.startGeneration).toHaveBeenCalledTimes(1);
  });
  it("keeps scroll while output arrives and shows New output", async () => {
    const { server, c } = await setup();
    await c.start("2026-11-06", "ko-KR", 3);
    const app = render(<CandidateCards controller={c} date="2026-11-06" />);
    const list = app.container.querySelector<HTMLElement>(".candidate-list")!;
    Object.defineProperties(list, {
      scrollHeight: { configurable: true, value: 1000 },
      clientHeight: { configurable: true, value: 100 },
    });
    list.scrollTop = 200;
    fireEvent.scroll(list);
    await act(async () => {
      server.snapshot().provisional = "new output";
      await c.refresh();
    });
    expect(list.scrollTop).toBe(200);
    expect(screen.getByRole("button", { name: "New output" })).not.toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "New output" }));
    expect(list.scrollTop).toBe(1000);
  });
  it("announces completion once, labels stale results and provides keyboard cancellation", async () => {
    const { server, c } = await setup();
    render(
      <>
        <Controls c={c} />
        <CandidateCards controller={c} date="2026-11-06" />
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
  fireEvent.click(screen.getByRole("button", { name: "Close Settings" }));
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

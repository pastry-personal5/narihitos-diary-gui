// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  within,
} from "@testing-library/react";
import { readFileSync } from "node:fs";
import { CandidateTable } from "../src/components/explorer/CandidateTable";
import { GenerationController } from "../src/application/generation/controller";
import {
  candidateStyle,
  openGenerationCandidate,
} from "../src/application/generation/candidates";
import { candidateKey } from "../src/application/generation/contracts";
import { ResourceCoordinator } from "../src/application/navigation/resources";
import { SessionBudget } from "../src/application/navigation/budget";
import { FakeBuffer, FakeBuffers } from "./fake-buffers";
import { navigationFake } from "./navigation-fake";
import { generationServer } from "./generation-fake";
import { candidateView, generationView } from "./generation-fixtures";

const controllers: GenerationController[] = [];
const resources: ResourceCoordinator[] = [];
afterEach(() => {
  cleanup();
  controllers.splice(0).forEach((c) => c.dispose());
  resources.splice(0).forEach((r) => r.dispose());
});

function setup() {
  const server = generationServer();
  const c = new GenerationController(server.bridge, server.preparation);
  controllers.push(c);
  const r = new ResourceCoordinator(
    navigationFake(),
    new SessionBudget(),
    {
      sourceGuard: async () => "cancel",
      notice: vi.fn(),
      savedDatesChanged: vi.fn(),
    },
    new FakeBuffers(),
  );
  resources.push(r);
  const job = generationView();
  c.state = { ...c.state, jobs: new Map([[job.job.id, job]]) };
  vi.spyOn(c, "candidate").mockImplementation((id) =>
    id === "j-1" ? candidateView() : undefined,
  );
  vi.spyOn(c, "preview").mockReturnValue("Live preview");
  return { c, r, job };
}

describe("Draft candidate table", () => {
  it("uses Snippet table styling and numbers across jobs while keeping each job's actual style", () => {
    const { c, r, job } = setup();
    const second = { ...job, job: { ...job.job, id: "second" } };
    c.state = {
      ...c.state,
      jobs: new Map([
        ["j", job],
        ["second", second],
      ]),
    };
    render(<CandidateTable controller={c} resources={r} date={job.date} />);
    const table = screen.getByRole("table", { name: "Candidates" });
    expect(table.classList.contains("snippet-table")).toBe(true);
    expect(
      within(table)
        .getAllByRole("columnheader")
        .map((h) => h.textContent),
    ).toEqual([
      "Number",
      "Style label",
      "Info button",
      "Open in Tabbed Editor button",
    ]);
    const rows = within(table).getAllByRole("row").slice(1);
    expect(
      rows.map((row) =>
        within(row)
          .getAllByRole("cell")
          .slice(0, 2)
          .map((cell) => cell.textContent),
      ),
    ).toEqual([
      ["1", "standard"],
      ["2", "concise"],
      ["3", "standard"],
      ["4", "concise"],
    ]);
    expect(table.querySelectorAll("button .bitmap-icon")).toHaveLength(8);
  });

  it("shows the matching description, dismisses on Escape and restores focus without opening an editor", async () => {
    const { c, r, job } = setup();
    render(<CandidateTable controller={c} resources={r} date={job.date} />);
    const info = screen.getByRole("button", {
      name: "Style description of candidate 2 · concise",
    });
    fireEvent.click(info);
    const dialog = screen.getByRole("dialog", { name: "Style description" });
    expect(dialog.textContent).toContain(
      "short and plain; keep only what matters",
    );
    expect(r.resources.size).toBe(0);
    await act(async () => fireEvent.keyDown(dialog, { key: "Escape" }));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(document.activeElement).toBe(info);
  });

  it("focuses an existing tab, reopens a closed tab with retained text and preserves running previews", async () => {
    const { c, r, job } = setup();
    render(<CandidateTable controller={c} resources={r} date={job.date} />);
    const key = candidateKey("j", 1);
    r.syncCandidate(key, "Candidate 1", job.date, "draft");
    const original = r.resources.get(key);
    const focus = vi.spyOn(original!.buffer, "focus");
    await act(async () =>
      fireEvent.click(
        screen.getByRole("button", {
          name: "Open candidate 1 · standard in Tabbed Editor",
        }),
      ),
    );
    expect(r.activeKey).toBe(key);
    expect(focus).toHaveBeenCalled();
    expect(r.resources.get(key)).toBe(original);
    await r.closeTab(key);
    await act(async () =>
      fireEvent.click(
        screen.getByRole("button", {
          name: "Open candidate 1 · standard in Tabbed Editor",
        }),
      ),
    );
    expect(r.activeKey).toBe(key);
    expect(r.active?.buffer.getText()).toBe("draft");
    expect(r.resources.size).toBe(1);
    await act(async () =>
      fireEvent.click(
        screen.getByRole("button", {
          name: "Open candidate 2 · concise in Tabbed Editor",
        }),
      ),
    );
    expect(r.active?.buffer.getText()).toBe("Live preview");
  });

  it("explains unavailable slots, removes discarded results and filters other dates", async () => {
    const { c, r, job } = setup();
    job.candidates = ["not_started", "failed", "cancelled", "ready"].map(
      (status, i) => ({
        index: i + 1,
        id: null,
        status: status as "not_started" | "failed" | "cancelled" | "ready",
        error_code: null,
      }),
    );
    const pane = render(
      <CandidateTable controller={c} resources={r} date={job.date} />,
    );
    const buttons = screen.getAllByRole("button", { name: /Open candidate/ });
    expect(
      buttons.map((button) =>
        document
          .getElementById(button.getAttribute("aria-describedby")!)
          ?.textContent?.trim(),
      ),
    ).toEqual([
      "Candidate not started.",
      "Candidate failed.",
      "Candidate cancelled.",
      "Reading candidate…",
    ]);
    expect(buttons.map((b) => (b as HTMLButtonElement).disabled)).toEqual([
      true,
      true,
      true,
      true,
    ]);
    expect(buttons.map((b) => b.title)).toEqual([
      "Candidate not started.",
      "Candidate failed.",
      "Candidate cancelled.",
      "Reading candidate…",
    ]);
    c.state = { ...c.state, jobs: new Map() };
    pane.rerender(
      <CandidateTable controller={c} resources={r} date={job.date} />,
    );
    expect(screen.queryByRole("table")).toBeNull();
    expect(await openGenerationCandidate(c, r, "j", 1)).toBe(false);
    c.state = { ...c.state, jobs: new Map([["j", job]]) };
    pane.rerender(
      <CandidateTable controller={c} resources={r} date="2026-11-07" />,
    );
    expect(screen.queryByRole("table")).toBeNull();
  });

  it("does not create or activate a candidate tab during text composition", async () => {
    const { c, r, job } = setup();
    r.syncCandidate(
      "other",
      "Other candidate",
      job.date,
      "Unfinished composition",
    );
    const active = r.active!;
    const buffer = active.buffer as FakeBuffer;
    buffer.composing = true;
    const focus = vi.spyOn(active.buffer, "focus");
    expect(await openGenerationCandidate(c, r, "j", 1)).toBe(false);
    expect(r.resources.size).toBe(1);
    expect(r.active).toBe(active);
    expect(active.buffer.getText()).toBe("Unfinished composition");
    expect(focus).not.toHaveBeenCalled();
    buffer.composing = false;
    expect(await openGenerationCandidate(c, r, "j", 1)).toBe(true);
    expect(r.activeKey).toBe(candidateKey("j", 1));
  });

  it("makes the application background inert and removes the dialog when its results are discarded", async () => {
    const { c, r, job } = setup();
    const background = document.createElement("button");
    document.body.append(background);
    try {
      const pane = render(
        <CandidateTable controller={c} resources={r} date={job.date} />,
      );
      fireEvent.click(
        screen.getByRole("button", {
          name: "Style description of candidate 1 · standard",
        }),
      );
      expect(background.inert).toBe(true);
      const dialog = screen.getByRole("dialog", { name: "Style description" });
      const close = within(dialog).getByRole("button", {
        name: "Close Style description",
      });
      expect(document.activeElement).toBe(close);
      fireEvent.keyDown(close, { key: "Tab" });
      expect(document.activeElement).toBe(close);
      c.state = { ...c.state, jobs: new Map() };
      await act(async () =>
        pane.rerender(
          <CandidateTable controller={c} resources={r} date={job.date} />,
        ),
      );
      expect(screen.queryByRole("dialog")).toBeNull();
      expect(background.inert).not.toBe(true);
    } finally {
      background.remove();
    }
  });

  it("keeps style descriptions aligned with all bundled factory prompts", () => {
    for (const provider of [
      "codex",
      "claude",
      "qwen",
      "apple-foundation-models",
    ]) {
      const prompt = readFileSync(
        `assets/prompts/${provider}/generate-candidate.md`,
        "utf8",
      );
      for (let index = 1; index <= 5; index++) {
        const style = candidateStyle({
          index,
          status: "ready",
          id: null,
          error_code: null,
        })!;
        expect(prompt).toContain(`- \`${style.label}\`: ${style.description}`);
      }
    }
  });
});

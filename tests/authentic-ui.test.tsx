import { connectFromSettings } from "./server-connection";
import { navigationFake } from "./navigation-fake";
// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { EditorView } from "@codemirror/view";
import { App } from "../src/components/App";
import {
  AuthenticError,
  type Bridge,
  type Revision,
  type SaveRequest,
} from "../src/application/bridge";
import type { StreamEvent } from "../src/application/events";

import { settingsFake } from "./settings-fake";
const status = {
  instance_id: "instance",
  workspace_key: "workspace",
  server_version: "1.0.0",
  api_version: 1 as const,
  api_minor: 0,
  storage_ready: true,
  capabilities: ["status", "sessions", "jobs", "events"],
};
afterEach(cleanup);

/** In-memory stand-in for the server's Authentic routes; it outlives an App to model a restart. */
class FakeServer {
  files = new Map<string, { body: string; sequence: number }>();
  saves: SaveRequest[] = [];
  loads: string[] = [];
  gate: Promise<void> | null = null;
  failNext: AuthenticError | null = null;
  observer: ((event: StreamEvent) => void) | null = null;
  closeHandler: (() => Promise<boolean>) | null = null;
  private rev = (sequence: number): Revision => ({
    sequence,
    sha256: String(sequence).padStart(64, "0"),
  });
  bridge(): Bridge {
    return {
      ...navigationFake(),
      metadata: async () => ({ version: "1.0.0" }),
      ...settingsFake(),
      connect: async () => status,
      status: async () => status,
      watch: async (observer) => {
        this.observer = observer;
        return async () => {};
      },
      onCloseRequested: async (handler) => {
        this.closeHandler = handler;
        return () => {};
      },
      listAuthentic: async () => ({
        items: [...this.files.keys()].sort().map((date) => ({
          date,
          revision: this.rev(this.files.get(date)!.sequence),
          problem: null,
        })),
        next_cursor: null,
      }),
      loadAuthentic: async (date) => {
        this.loads.push(date);
        const f = this.files.get(date);
        return {
          date,
          exists: !!f,
          revision: f ? this.rev(f.sequence) : null,
          body: f?.body ?? "",
        };
      },
      saveAuthentic: async (r) => {
        this.saves.push(r);
        await this.gate;
        if (this.failNext) {
          const e = this.failNext;
          this.failNext = null;
          throw e;
        }
        const f = this.files.get(r.date);
        if ((f?.sequence ?? null) !== (r.expectedRevision?.sequence ?? null))
          throw new AuthenticError("revision_conflict", false);
        const sequence = (f?.sequence ?? 0) + 1;
        this.files.set(r.date, { body: r.body, sequence });
        return {
          operation_id: r.operationId,
          revision: this.rev(sequence),
          buffer_generation: r.bufferGeneration,
        };
      },
      createEmptyAuthentic: async (date, operationId) => {
        if (this.files.has(date))
          throw new AuthenticError("already_exists", false);
        this.files.set(date, { body: "", sequence: 1 });
        return {
          operation_id: operationId,
          revision: this.rev(1),
          buffer_generation: 0,
        };
      },
      refreshAuthentic: async () => ({ files: this.files.size, problems: 0 }),
      getOperation: async () => {
        throw new AuthenticError("not_found", false);
      },
    };
  }
}

async function open(server: FakeServer) {
  render(<App bridge={server.bridge()} />);
  connectFromSettings();
  await screen.findByText("Connected");
  fireEvent.click(screen.getByRole("button", { name: "Go back" }));
  await waitFor(() => expect(screen.queryByText("No file open")).toBeNull());
}
function editor() {
  const dom = document.querySelector<HTMLElement>(".cm-editor")!;
  return EditorView.findFromDOM(dom)!;
}
const type = (text: string) =>
  act(() => {
    const v = editor();
    v.dispatch({ changes: { from: v.state.doc.length, insert: text } });
  });
const createEmpty = () => screen.getByRole("button", { name: "Create Empty" });
const date = () => (screen.getByLabelText("Date") as HTMLInputElement).value;

describe("Authentic editing", () => {
  it("moves calendar focus by civil day without selecting until activation", async () => {
    const server = new FakeServer();
    await open(server);
    const original = date();
    const selected = screen.getByRole("gridcell", { name: original });
    selected.focus();
    fireEvent.keyDown(selected, { key: "ArrowRight" });
    await waitFor(() =>
      expect(document.activeElement?.getAttribute("aria-label")).not.toBe(
        original,
      ),
    );
    expect(date()).toBe(original);
    expect(
      document.querySelectorAll('[role="gridcell"][tabindex="0"]'),
    ).toHaveLength(1);
  });

  it("Create Empty saves an empty file, then typing autosaves with one debounced save", async () => {
    const server = new FakeServer();
    await open(server);
    await waitFor(() =>
      expect(createEmpty().hasAttribute("disabled")).toBe(false),
    );
    fireEvent.click(createEmpty());
    await waitFor(() =>
      expect(server.files.get(date())).toEqual({ body: "", sequence: 1 }),
    );
    await waitFor(() =>
      expect(createEmpty().hasAttribute("disabled")).toBe(true),
    );
    expect(createEmpty().getAttribute("title")).toMatch(/already exists/);
    type("a");
    type("b");
    expect(screen.getByText(/Unsaved changes\./)).toBeTruthy();
    await waitFor(() => expect(screen.getByText(/Saved\./)).toBeTruthy(), {
      timeout: 3000,
    });
    expect(server.saves).toHaveLength(1);
    expect(server.files.get(date())).toEqual({ body: "ab", sequence: 2 });
    expect(server.saves[0].expectedRevision?.sequence).toBe(1);
  });

  it("Draft and Entry show the same single editor and keep edits", async () => {
    const server = new FakeServer();
    await open(server);
    type("shared");
    fireEvent.click(screen.getByRole("button", { name: "Entry" }));
    expect(document.querySelectorAll(".cm-editor")).toHaveLength(1);
    expect(editor().state.doc.toString()).toBe("shared");
    fireEvent.click(screen.getByRole("button", { name: "Draft" }));
    expect(editor().state.doc.toString()).toBe("shared");
  });

  it("a saved file reloads after restart and cannot be re-created", async () => {
    const server = new FakeServer();
    server.files.set(new Date().toLocaleDateString("sv"), {
      body: "kept",
      sequence: 4,
    });
    await open(server);
    await waitFor(() => expect(editor().state.doc.toString()).toBe("kept"));
    expect(createEmpty().hasAttribute("disabled")).toBe(true);
    expect(screen.queryByLabelText("Save status")).toBeNull();
  });

  it("changing the date waits for the slow save and saves to the original date", async () => {
    const server = new FakeServer();
    await open(server);
    const original = date();
    let release!: () => void;
    server.gate = new Promise((r) => (release = r));
    type("slow");
    fireEvent.change(screen.getByLabelText("Date"), {
      target: { value: "2020-01-02" },
    });
    await waitFor(() => expect(server.saves).toHaveLength(1));
    expect(server.loads).not.toContain("2020-01-02");
    act(() => release());
    await waitFor(() => expect(date()).toBe("2020-01-02"));
    expect(server.saves[0].date).toBe(original);
    expect(server.files.get(original)?.body).toBe("slow");
    expect(server.loads).toContain("2020-01-02");
  });

  it("a failed save blocks date change and close until retry succeeds", async () => {
    const server = new FakeServer();
    await open(server);
    const original = date();
    server.failNext = new AuthenticError("storage_error", false);
    type("x");
    await screen.findByText(
      /Save failed \(storage_error\)/,
      {},
      { timeout: 3000 },
    );
    expect(editor().state.doc.toString()).toBe("x");
    fireEvent.change(screen.getByLabelText("Date"), {
      target: { value: "2020-01-02" },
    });
    await screen.findByText(
      /Resolve the save problem before changing the date/,
    );
    expect(date()).toBe(original);
    expect(await server.closeHandler!()).toBe(false);
    fireEvent.click(screen.getByRole("button", { name: "Retry save" }));
    await waitFor(() => expect(screen.getByText(/Saved\./)).toBeTruthy());
    expect(server.files.get(original)?.body).toBe("x");
    expect(await server.closeHandler!()).toBe(true);
  });

  it("a clean close is immediate and a dirty close flushes first", async () => {
    const server = new FakeServer();
    await open(server);
    expect(await server.closeHandler!()).toBe(true);
    type("late");
    expect(await server.closeHandler!()).toBe(true);
    expect(server.files.get(date())?.body).toBe("late");
  });

  it("an external change reloads a clean buffer and conflicts a dirty one", async () => {
    const server = new FakeServer();
    server.files.set(new Date().toLocaleDateString("sv"), {
      body: "v1",
      sequence: 1,
    });
    await open(server);
    await waitFor(() => expect(editor().state.doc.toString()).toBe("v1"));
    server.files.set(date(), { body: "v2", sequence: 2 });
    act(() =>
      server.observer!({
        kind: "authentic_changed",
        instance_id: "i",
        session_id: "s",
      }),
    );
    await waitFor(() => expect(editor().state.doc.toString()).toBe("v2"));
    type("!");
    server.files.set(date(), { body: "v3", sequence: 3 });
    act(() =>
      server.observer!({
        kind: "authentic_changed",
        instance_id: "i",
        session_id: "s",
      }),
    );
    await screen.findByText(/changed on disk while you had unsaved edits/);
    expect(editor().state.doc.toString()).toBe("v2!");
    fireEvent.click(screen.getByRole("button", { name: "Discard changes" }));
    fireEvent.click(screen.getByRole("button", { name: "Confirm discard" }));
    await waitFor(() => expect(editor().state.doc.toString()).toBe("v3"));
    expect(screen.queryByLabelText("Save status")).toBeNull();
  });
});

it("Export stays available for dirty Authentic edits and waits for their saved revision", async () => {
  const server = new FakeServer();
  const base = server.bridge();
  let exported: Revision | undefined;
  server.bridge = () => ({
    ...base,
    startExport: async (request) => {
      exported = request.expected_revision;
      return {
        outcome: "result",
        result: {
          kind: "export",
          operation_id: request.operation_id,
          target: "icloud",
          date: request.date,
          revision: request.expected_revision,
          state: "succeeded",
          checkpoint: 2,
          reason: null,
        },
      };
    },
  });
  await open(server);
  fireEvent.click(createEmpty());
  await waitFor(() =>
    expect(createEmpty().hasAttribute("disabled")).toBe(true),
  );
  let release!: () => void;
  server.gate = new Promise<void>((resolve) => {
    release = resolve;
  });
  type("new text");
  const button = screen.getByRole("button", { name: "Export" });
  expect(button.hasAttribute("disabled")).toBe(false);
  fireEvent.click(button);
  await waitFor(() => expect(server.saves).toHaveLength(1));
  expect(exported).toBeUndefined();
  await act(async () => release());
  await waitFor(() => expect(exported?.sequence).toBe(2));
  expect(server.files.get(date())?.body).toBe("new text");
});

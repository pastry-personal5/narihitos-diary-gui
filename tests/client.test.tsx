import { connectFromSettings } from "./server-connection";
import { navigationFake } from "./navigation-fake";
// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { App } from "../src/components/App";
import { mountScratchEditor } from "../src/editor/scratch-editor";
import type { Bridge } from "../src/application/bridge";
import { settingsFake } from "./settings-fake";
const status = {
  instance_id: "instance",
  workspace_key: "workspace",
  server_version: "1.0.0",
  api_version: 1 as const,
  api_minor: 0,
  storage_ready: false,
  capabilities: ["status", "sessions", "jobs", "events"],
};
afterEach(cleanup);
function bridge(): Bridge {
  return {
    ...navigationFake(),
    metadata: async () => ({ version: "1.0.0" }),
    ...settingsFake(),
    connect: async () => status,
    status: async () => status,
    watch: async () => async () => {},
    listAuthentic: async () => ({ items: [], next_cursor: null }),
    loadAuthentic: async (date) => ({
      date,
      exists: false,
      revision: null,
      body: "",
    }),
    saveAuthentic: async () => {
      throw new Error("unused");
    },
    createEmptyAuthentic: async () => {
      throw new Error("unused");
    },
    refreshAuthentic: async () => ({ files: 0, problems: 0 }),
    getOperation: async () => {
      throw new Error("unused");
    },
  };
}
describe("scaffold shell", () => {
  it("renders canonical regions and explains unavailable features", async () => {
    render(<App bridge={bridge()} />);
    for (const name of [
      "Standard Layout",
      "Main Toolbar Strip",
      "Icon Bar",
      "Explorer",
      "Tabbed Editor",
      "AI Chat",
    ])
      expect(screen.getByLabelText(name)).toBeTruthy();
    expect(screen.queryByText(/^Version/)).toBeNull();
    expect(screen.queryByRole("heading", { name: "Tabbed Editor" })).toBeNull();
    const editor = screen.getByLabelText("Tabbed Editor");
    const messages = screen.getByLabelText("Global Messages");
    expect(
      within(editor).queryByText("Connect to the server to open this date."),
    ).toBeNull();
    expect(
      within(messages).getByText(/Connect to the server to open this date/),
    ).toBeTruthy();
    expect(
      screen
        .getByRole("button", { name: "Create Empty" })
        .hasAttribute("disabled"),
    ).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "Entry tab" }));
    expect(screen.getByText("No saved dates yet.")).toBeTruthy();
  });
  it("opens AI Activities and its bitmap explanations with keyboard focus restoration", async () => {
    render(<App bridge={bridge()} />);
    const opener = within(
      screen.getByLabelText("Main Toolbar Strip"),
    ).getByRole("button", { name: "AI Activities" });
    expect(screen.queryByLabelText("AI Activity")).toBeNull();
    expect(screen.getByLabelText("AI activity status").textContent).toContain(
      "unavailable",
    );
    opener.focus();
    fireEvent.click(opener);
    const dialog = screen.getByRole("dialog", { name: "AI Activities" });
    const close = within(dialog).getByRole("button", {
      name: "Close AI Activities",
    });
    expect(document.activeElement).toBe(close);
    expect(within(dialog).getByText(/No AI activities yet/)).toBeTruthy();
    const info = within(dialog).getByRole("button", {
      name: "About progress and cancellation",
    });
    expect(info.querySelector(".bitmap-icon")?.getAttribute("style")).toContain(
      "state-details.png",
    );
    fireEvent.click(info);
    const explanation = screen.getByRole("dialog", {
      name: "Progress and cancellation",
    });
    expect(explanation.textContent).toContain("not an estimate");
    fireEvent.keyDown(document.activeElement!, { key: "Tab", shiftKey: true });
    expect(document.activeElement?.getAttribute("aria-label")).toBe(
      "Close explanation",
    );
    fireEvent.keyDown(explanation, { key: "Escape" });
    await waitFor(() => expect(document.activeElement).toBe(info));
    expect(screen.getByRole("dialog", { name: "AI Activities" })).toBeTruthy();
    fireEvent.click(
      within(dialog).getByRole("button", {
        name: "About results and session storage",
      }),
    );
    expect(
      screen.getByRole("dialog", { name: "Results and session storage" })
        .textContent,
    ).toContain("Authentic file");
    fireEvent.click(screen.getByRole("button", { name: "Close explanation" }));
    fireEvent.keyDown(close, { key: "Escape" });
    await waitFor(() => expect(document.activeElement).toBe(opener));
    fireEvent.click(opener);
    fireEvent.click(
      screen.getByRole("button", { name: "Close AI Activities" }),
    );
    expect(screen.queryByRole("dialog")).toBeNull();
  });
  it("opens Trash Recovery from the toolbar and keeps recovery controls out of Global Messages", async () => {
    render(<App bridge={bridge()} />);
    const toolbar = screen.getByLabelText("Main Toolbar Strip");
    const opener = within(toolbar).getByRole("button", {
      name: "Trash Recovery",
    });
    const log = screen.getByLabelText("Global Messages");
    expect(within(log).queryByText("Check unfinished moves")).toBeNull();
    expect(screen.queryByRole("dialog")).toBeNull();
    opener.focus();
    fireEvent.click(opener);
    const dialog = screen.getByRole("dialog", { name: "Trash Recovery" });
    const close = within(dialog).getByRole("button", {
      name: "Close Trash Recovery",
    });
    expect(document.activeElement).toBe(close);
    expect(within(dialog).getByText(/The server is disconnected/)).toBeTruthy();
    expect(
      within(dialog).getByRole<HTMLButtonElement>("button", {
        name: "Check unfinished moves",
      }).disabled,
    ).toBe(true);
    expect(within(dialog).getByText("Continue a partial move")).toBeTruthy();
    expect(
      within(dialog).getByText("Resolve an uncertain outcome"),
    ).toBeTruthy();
    fireEvent.keyDown(close, { key: "Escape" });
    await waitFor(() => expect(document.activeElement).toBe(opener));
    expect(screen.queryByRole("dialog")).toBeNull();
    fireEvent.click(opener);
    fireEvent.click(
      screen.getByRole("button", { name: "Close Trash Recovery" }),
    );
    expect(screen.queryByRole("dialog")).toBeNull();
  });
  it("reports disconnected and reconnects without changing scratch state", async () => {
    const adapter = bridge();
    adapter.connect = vi
      .fn()
      .mockRejectedValueOnce(new Error("private token must not appear"))
      .mockResolvedValueOnce(status);
    render(<App bridge={adapter} />);
    connectFromSettings();
    await screen.findByText(/Server unavailable/);
    expect(screen.queryByText(/private token/)).toBeNull();
    connectFromSettings();
    await screen.findByText("Connected · Storage unavailable");
  });
  it("destroys CodeMirror on unmount and permits a clean remount", async () => {
    const view = render(<App bridge={bridge()} />);
    connectFromSettings();
    await screen.findByText("Connected · Storage unavailable");
    fireEvent.click(screen.getByRole("button", { name: "Go back" }));
    await waitFor(() =>
      expect(document.querySelectorAll(".cm-editor").length).toBe(1),
    );
    view.unmount();
    expect(document.querySelectorAll(".cm-editor").length).toBe(0);
    render(<App bridge={bridge()} />);
    expect(document.querySelectorAll(".cm-editor").length).toBe(0);
  });
  it("dedicated editor disposes its DOM without writing files", () => {
    const parent = document.createElement("div");
    document.body.append(parent);
    const changed = vi.fn();
    const dispose = mountScratchEditor(parent, "synthetic", changed);
    expect(parent.textContent).toContain("synthetic");
    expect(changed).not.toHaveBeenCalled();
    dispose();
    expect(parent.childElementCount).toBe(0);
    parent.remove();
  });
  it("is read-only and reports no open file while disconnected", async () => {
    render(<App bridge={bridge()} />);
    expect(document.querySelector(".cm-editor")).toBeNull();
    expect(screen.getByText("No file open")).toBeTruthy();
    expect(screen.queryByText("Saved")).toBeNull();
  });
  it("ignores a settings reply that arrives after the subscription disconnects", async () => {
    const adapter = bridge();
    let observer!: (
      event: import("../src/application/events").StreamEvent,
    ) => void;
    let resolve!: (
      value: Awaited<ReturnType<Bridge["serverSettings"]>>,
    ) => void;
    adapter.serverSettings = vi.fn(
      () =>
        new Promise((r) => {
          resolve = r;
        }),
    );
    const stop = vi.fn(async () => {});
    adapter.watch = async (listener) => {
      observer = listener;
      return stop;
    };
    render(<App bridge={adapter} />);
    connectFromSettings();
    await waitFor(() => expect(observer).toBeDefined());
    await act(async () => observer({ kind: "disconnected" }));
    await act(async () =>
      resolve({
        config_generation: 1,
        last_reload: null,
        storage_ready: true,
        sources: [],
        profiles: [],
      }),
    );
    expect(screen.getByText(/Connection interrupted/)).toBeTruthy();
    expect(adapter.serverSettings).toHaveBeenCalledTimes(1);
    expect(document.querySelector(".cm-editor")).toBeNull();
    expect(stop).toHaveBeenCalled();
  });
  it("tells in Global Messages how to enable source import when no source is available", async () => {
    render(<App bridge={bridge()} />);
    connectFromSettings();
    await screen.findByText("Connected · Storage unavailable");
    fireEvent.click(screen.getByRole("button", { name: "Go back" }));
    const log = screen.getByRole("log");
    await waitFor(() =>
      expect(log.textContent).toContain("Source import is off"),
    );
    expect(log.textContent).toContain("ayumi or narihito");
    expect(log.textContent).toContain("Reload configuration");
  });
  it("stays quiet about source import when a source is available", async () => {
    const adapter = {
      ...bridge(),
      ...settingsFake(
        {},
        { sources: [{ alias: "ayumi", available: true, reason: "ready" }] },
      ),
    };
    render(<App bridge={adapter} />);
    connectFromSettings();
    await screen.findByText("Connected · Storage unavailable");
    fireEvent.click(screen.getByRole("button", { name: "Go back" }));
    expect(screen.getByRole("log").textContent).not.toContain(
      "Source import is off",
    );
  });
  it("does not log rejected connection details to the console", async () => {
    const spies = (["log", "info", "warn", "error"] as const).map((m) =>
      vi.spyOn(console, m).mockImplementation(() => {}),
    );
    const adapter = bridge();
    adapter.connect = vi
      .fn()
      .mockRejectedValue(new Error("private token /private/path"));
    render(<App bridge={adapter} />);
    connectFromSettings();
    await screen.findByText(/Server unavailable/);
    for (const spy of spies)
      expect(JSON.stringify(spy.mock.calls)).not.toMatch(/private/);
    vi.restoreAllMocks();
  });
});

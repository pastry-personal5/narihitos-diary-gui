// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { App } from "../src/components/App";
import { EditorView } from "@codemirror/view";
import { mountScratchEditor } from "../src/editor/scratch-editor";
import type { Bridge } from "../src/application/bridge";
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
    metadata: async () => ({ version: "1.0.0" }),
    connect: async () => status,
    status: async () => status,
    watch: async () => async () => {},
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
    expect(await screen.findByText("Version v1.0.0")).toBeTruthy();
    expect(
      screen
        .getByRole("button", { name: "Create Empty" })
        .hasAttribute("disabled"),
    ).toBe(true);
    expect(screen.getByText(/Never saved/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Entry tab" }));
    expect(
      screen.getByText("Saved Entry dates become available with persistence."),
    ).toBeTruthy();
  });
  it("reports disconnected and reconnects without changing scratch state", async () => {
    const adapter = bridge();
    adapter.connect = vi
      .fn()
      .mockRejectedValueOnce(new Error("private token must not appear"))
      .mockResolvedValueOnce(status);
    render(<App bridge={adapter} />);
    fireEvent.click(screen.getByRole("button", { name: "Connect" }));
    await screen.findByText(/Server unavailable/);
    expect(screen.queryByText(/private token/)).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Connect" }));
    await screen.findByText("Connected · Storage unavailable");
  });
  it("destroys CodeMirror on unmount and permits a clean remount", async () => {
    const view = render(<App bridge={bridge()} />);
    await waitFor(() =>
      expect(document.querySelectorAll(".cm-editor").length).toBe(1),
    );
    view.unmount();
    expect(document.querySelectorAll(".cm-editor").length).toBe(0);
    render(<App bridge={bridge()} />);
    expect(document.querySelectorAll(".cm-editor").length).toBe(1);
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
  it("keeps edits in memory and never reports them saved", async () => {
    render(<App bridge={bridge()} />);
    const dom = await waitFor(() => {
      const el = document.querySelector<HTMLElement>(".cm-editor");
      expect(el).toBeTruthy();
      return el!;
    });
    const view = EditorView.findFromDOM(dom)!;
    view.dispatch({ changes: { from: 0, insert: "edit" } });
    expect(view.state.doc.toString()).toBe("edit");
    expect(screen.getByText(/Never saved/)).toBeTruthy();
    expect(screen.queryByText(/^Saved/)).toBeNull();
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
    fireEvent.click(screen.getByRole("button", { name: "Connect" }));
    await screen.findByText(/Server unavailable/);
    for (const spy of spies)
      expect(JSON.stringify(spy.mock.calls)).not.toMatch(/private/);
    vi.restoreAllMocks();
  });
});

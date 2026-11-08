// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { App } from "../src/components/App";
import { PaneSeparator } from "../src/components/standard-layout/PaneSeparator";
import { GlobalMessages } from "../src/components/global-messages/GlobalMessages";
import { GlobalMessagesStore } from "../src/application/global-messages/store";
import { navigationFake } from "./navigation-fake";
import { settingsFake } from "./settings-fake";
import type { Bridge } from "../src/application/bridge";

afterEach(cleanup);

function Fixture({ reverse = false }: { reverse?: boolean }) {
  const [value, setValue] = useState(200);
  return (
    <PaneSeparator
      label="Sep"
      orientation="vertical"
      min={160}
      max={480}
      value={value}
      change={setValue}
      reverse={reverse}
    />
  );
}
function drag(el: HTMLElement, from: number, to: number) {
  // jsdom lacks pointer capture; stub it so the handler path matches a browser.
  el.setPointerCapture = vi.fn();
  el.releasePointerCapture = vi.fn();
  fireEvent.pointerDown(el, { pointerId: 1, button: 0, clientX: from });
  fireEvent.pointerMove(el, { pointerId: 1, clientX: to });
  return () => fireEvent.pointerUp(el, { pointerId: 1 });
}

describe("N01 pane separators", () => {
  it("clamps pointer drags to the usable range in both directions", () => {
    render(<Fixture />);
    const sep = screen.getByRole("separator", { name: "Sep" });
    const up = drag(sep, 0, 5000);
    expect(sep.getAttribute("data-dragging")).toBe("true");
    expect(sep.getAttribute("aria-valuenow")).toBe("480");
    fireEvent.pointerMove(sep, { pointerId: 1, clientX: -5000 });
    expect(sep.getAttribute("aria-valuenow")).toBe("160");
    up();
    expect(sep.hasAttribute("data-dragging")).toBe(false);
    // After release, further moves do nothing.
    fireEvent.pointerMove(sep, { pointerId: 1, clientX: 300 });
    expect(sep.getAttribute("aria-valuenow")).toBe("160");
  });
  it("inverts pointer direction for a right-hand pane and ignores other buttons/pointers", () => {
    render(<Fixture reverse />);
    const sep = screen.getByRole("separator", { name: "Sep" });
    drag(sep, 100, 60);
    expect(sep.getAttribute("aria-valuenow")).toBe("240");
    fireEvent.pointerMove(sep, { pointerId: 2, clientX: 0 });
    expect(sep.getAttribute("aria-valuenow")).toBe("240");
    fireEvent.pointerCancel(sep, { pointerId: 1 });
    expect(sep.hasAttribute("data-dragging")).toBe(false);
    fireEvent.pointerMove(sep, { pointerId: 1, clientX: 0 });
    expect(sep.getAttribute("aria-valuenow")).toBe("240");
    fireEvent.pointerDown(sep, { pointerId: 3, button: 2, clientX: 0 });
    fireEvent.pointerMove(sep, { pointerId: 3, clientX: 90 });
    expect(sep.getAttribute("aria-valuenow")).toBe("240");
  });
  it("ignores arrow keys during IME composition and off-axis keys", () => {
    render(<Fixture />);
    const sep = screen.getByRole("separator", { name: "Sep" });
    fireEvent.keyDown(sep, { key: "ArrowRight", isComposing: true });
    fireEvent.keyDown(sep, { key: "ArrowUp" });
    expect(sep.getAttribute("aria-valuenow")).toBe("200");
    fireEvent.keyDown(sep, { key: "ArrowLeft" });
    expect(sep.getAttribute("aria-valuenow")).toBe("190");
  });
});

describe("N01 Standard Layout containment", () => {
  const status = {
    instance_id: "i",
    workspace_key: "w",
    server_version: "1.0.0",
    api_version: 1 as const,
    api_minor: 0,
    storage_ready: true,
    capabilities: ["status", "sessions", "jobs", "events"],
  };
  const bridge = (): Bridge =>
    ({
      ...navigationFake(),
      ...settingsFake(),
      metadata: async () => ({ version: "1.0.0" }),
      connect: async () => status,
      status: async () => status,
      watch: async () => async () => {},
      listAuthentic: async () => ({ items: [], next_cursor: null }),
    }) as unknown as Bridge;

  it("places Global Messages inside the editor column and drives the grid from separators", () => {
    render(<App bridge={bridge()} />);
    const messages = screen.getByLabelText("Global Messages");
    const editorColumn = messages.closest(".tabbed-editor");
    expect(editorColumn).toBeTruthy();
    expect(editorColumn?.contains(screen.getByLabelText("Editor tabs"))).toBe(
      true,
    );
    expect(
      screen.getByLabelText("Explorer").closest(".tabbed-editor"),
    ).toBeNull();
    expect(
      screen.getByLabelText("AI Chat").closest(".tabbed-editor"),
    ).toBeNull();

    const workspace = document.querySelector<HTMLElement>(".main-content")!;
    const explorer = screen.getByRole("separator", { name: "Resize Explorer" });
    const before = Number(explorer.getAttribute("aria-valuenow"));
    for (let i = 0; i < 100; i++)
      fireEvent.keyDown(explorer, { key: "ArrowRight" });
    expect(explorer.getAttribute("aria-valuenow")).toBe("480");
    expect(workspace.style.gridTemplateColumns).toContain("480px");
    expect(before).toBeLessThan(480);

    const chat = screen.getByRole("separator", { name: "Resize AI Chat" });
    for (let i = 0; i < 100; i++)
      fireEvent.keyDown(chat, { key: "ArrowRight" });
    expect(chat.getAttribute("aria-valuenow")).toBe("160");
    // The editor column keeps a minimum track so a small window never collapses it.
    expect(workspace.style.gridTemplateColumns).toContain("minmax(220px, 1fr)");

    const height = screen.getByRole("separator", {
      name: "Resize Global Messages",
    });
    for (let i = 0; i < 100; i++)
      fireEvent.keyDown(height, { key: "ArrowDown" });
    expect(height.getAttribute("aria-valuenow")).toBe("80");
    for (let i = 0; i < 100; i++) fireEvent.keyDown(height, { key: "ArrowUp" });
    expect(height.getAttribute("aria-valuenow")).toBe("320");
    expect(messages.style.height).toBe("320px");
  });
});

describe("N10 Global Messages presentation", () => {
  it("lists oldest first with time, severity text and icon, polite log, and no focus theft", () => {
    const messages = new GlobalMessagesStore(() => "09:15");
    messages.add("Imported 3 sources");
    messages.add("Disk nearly full", "warning");
    messages.add("Save failed", "error");
    const outside = document.createElement("button");
    document.body.append(outside);
    outside.focus();
    render(<GlobalMessages messages={messages} height={120} />);
    const log = screen.getByRole("log");
    expect(log.getAttribute("aria-live")).toBe("polite");
    const rows = [...log.querySelectorAll("p")];
    expect(rows.map((r) => r.className)).toEqual([
      "message-info",
      "message-warning",
      "message-error",
    ]);
    expect(rows[0].textContent).toContain("09:15");
    expect(rows[0].textContent).toContain("info");
    expect(rows[1].textContent).toContain("warning: Disk nearly full");
    expect(rows[2].textContent).toContain("✕");
    expect(rows[1].textContent).toContain("!");
    // Icons are decorative; severity is always present as text.
    expect(log.querySelectorAll('[aria-hidden="true"]')).toHaveLength(3);
    expect(document.activeElement).toBe(outside);
    outside.remove();
  });
});

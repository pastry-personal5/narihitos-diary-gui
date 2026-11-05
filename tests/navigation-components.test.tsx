// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { PaneSeparator } from "../src/components/PaneSeparator";
import { GlobalMessages } from "../src/components/GlobalMessages";
import { Messages } from "../src/application/navigation/messages";
afterEach(cleanup);
it("resizes separators with keyboard bounds and the correct direction", () => {
  function Fixture() {
    const [value, setValue] = useState(180);
    return (
      <PaneSeparator
        label="Resize messages"
        orientation="horizontal"
        min={80}
        max={200}
        value={value}
        change={setValue}
        reverse
      />
    );
  }
  render(<Fixture />);
  const separator = screen.getByRole("separator", { name: "Resize messages" });
  fireEvent.keyDown(separator, { key: "ArrowUp" });
  expect(separator.getAttribute("aria-valuenow")).toBe("190");
  fireEvent.keyDown(separator, { key: "ArrowUp" });
  fireEvent.keyDown(separator, { key: "ArrowUp" });
  expect(separator.getAttribute("aria-valuenow")).toBe("200");
  fireEvent.keyDown(separator, { key: "ArrowDown" });
  expect(separator.getAttribute("aria-valuenow")).toBe("190");
});
it("keeps a scrolled-up message log in place and offers an explicit jump", () => {
  const messages = new Messages(() => "10:00");
  messages.add("First");
  const ui = render(<GlobalMessages messages={messages} height={160} />);
  const log = screen.getByRole("log");
  Object.defineProperty(log, "scrollHeight", {
    configurable: true,
    value: 1000,
  });
  Object.defineProperty(log, "clientHeight", {
    configurable: true,
    value: 100,
  });
  log.scrollTop = 200;
  fireEvent.scroll(log);
  messages.add("Second", "warning");
  ui.rerender(<GlobalMessages messages={messages} height={160} />);
  expect(log.scrollTop).toBe(200);
  expect(screen.getByText("warning")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Jump to latest" }));
  expect(log.scrollTop).toBe(1000);
});
it("bounds retained messages and suppresses adjacent duplicate notices", () => {
  const messages = new Messages(() => "10:00");
  messages.add("same", "info", true);
  messages.add("same", "info", true);
  expect(messages.items).toHaveLength(1);
  for (let i = 0; i < 501; i++) messages.add(`message ${i}`);
  expect(messages.items).toHaveLength(500);
  expect(messages.removed).toBe(2);
  expect(messages.items[0].text).toBe("message 1");
  const listener = vi.fn();
  const off = messages.subscribe(listener);
  messages.add("new");
  off();
  messages.add("last");
  expect(listener).toHaveBeenCalledTimes(1);
});

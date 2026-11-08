// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
} from "@testing-library/react";
import { useState } from "react";
import {
  CommandPalette,
  type PaletteEntry,
} from "../src/components/CommandPalette";
afterEach(cleanup);
const entries: PaletteEntry[] = [
  { id: "save", label: "Save", reason: null, shortcuts: ["Command+S"] },
  { id: "export", label: "Export", reason: "Connect first", shortcuts: [] },
];
function Fixture({
  items = entries,
  run = () => {},
}: {
  items?: PaletteEntry[];
  run?: (id: string) => void;
}) {
  const [open, setOpen] = useState(false);
  return (
    <>
      <button id="palette-toggle" onClick={() => setOpen(!open)}>
        Toggle
      </button>
      <button>Outside</button>
      {open && (
        <CommandPalette
          entries={items}
          onClose={() => setOpen(false)}
          onRun={run}
        />
      )}
    </>
  );
}
function open() {
  const toggle = screen.getByText("Toggle");
  toggle.focus();
  fireEvent.click(toggle);
  return screen.getByRole("combobox");
}
it("keeps the toggle operable, resets search and restores Escape focus", () => {
  render(<Fixture />);
  let input = open();
  expect(document.activeElement).toBe(input);
  expect(screen.getByRole("dialog").hasAttribute("aria-modal")).toBe(false);
  fireEvent.change(input, { target: { value: "zzzz" } });
  expect(screen.getByText("No matching command")).toBeTruthy();
  expect(input.hasAttribute("aria-activedescendant")).toBe(false);
  fireEvent.pointerDown(screen.getByText("Toggle"));
  screen.getByText("Toggle").focus();
  fireEvent.click(screen.getByText("Toggle"));
  expect(screen.queryByRole("dialog")).toBeNull();
  input = open();
  expect(input).toHaveProperty("value", "");
  fireEvent.keyDown(input, { key: "Escape" });
  expect(document.activeElement).toBe(screen.getByText("Toggle"));
});
it("preserves selected action IDs through live availability changes", () => {
  const view = render(<Fixture />);
  const input = open();
  expect(input.getAttribute("aria-activedescendant")).toBe(
    "palette-option-save",
  );
  view.rerender(
    <Fixture
      items={[
        { ...entries[0], reason: "No file" },
        { ...entries[1], reason: null },
      ]}
    />,
  );
  expect(screen.getAllByRole("option")[0].textContent).toBe("Export");
  expect(input.getAttribute("aria-activedescendant")).toBe(
    "palette-option-save",
  );
  fireEvent.keyDown(input, { key: "ArrowUp" });
  expect(input.getAttribute("aria-activedescendant")).toBe(
    "palette-option-export",
  );
});
it("ignores IME keys and held Enter, then executes once after closing", () => {
  const run = vi.fn(() => expect(screen.queryByRole("dialog")).toBeNull());
  render(<Fixture run={run} />);
  const input = open();
  for (const key of ["Escape", "ArrowDown", "Enter"])
    fireEvent.keyDown(input, { key, isComposing: true });
  fireEvent.keyDown(input, { key: "Enter", repeat: true });
  expect(run).not.toHaveBeenCalled();
  fireEvent.keyDown(input, { key: "Enter" });
  expect(run).toHaveBeenCalledExactlyOnceWith("save");
});
it("dismisses on outside pointer/focus without stealing the destination", () => {
  render(<Fixture />);
  open();
  act(() => screen.getByText("Outside").focus());
  expect(screen.queryByRole("dialog")).toBeNull();
  expect(document.activeElement).toBe(screen.getByText("Outside"));
  open();
  fireEvent.pointerDown(screen.getByText("Outside"));
  expect(screen.queryByRole("dialog")).toBeNull();
});
it("pointer execution retains unavailable rows and modal focus wins", () => {
  render(
    <Fixture
      run={() => {
        const dialog = document.createElement("div");
        dialog.setAttribute("role", "dialog");
        dialog.setAttribute("aria-modal", "true");
        const button = document.createElement("button");
        dialog.append(button);
        document.body.append(dialog);
        button.focus();
      }}
    />,
  );
  open();
  fireEvent.click(screen.getByRole("option", { name: /Export/u }));
  expect(
    document.activeElement?.parentElement?.getAttribute("aria-modal"),
  ).toBe("true");
  document.querySelector('[aria-modal="true"]')?.remove();
});

// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { App } from "../src/components/App";
import type { Bridge } from "../src/application/bridge";
import { navigationFake } from "./navigation-fake";
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
const local = {
  date: "2024-02-29",
  datetime: "2024-02-29T00:00:00",
  offset_seconds: 0,
};
const sourceItem = {
  kind: "included",
  source: {
    id: "one",
    relative_key: "2024/02/29/one.md",
    digest: "a".repeat(64),
    import_id: "import",
    dating: {
      state: "agreed",
      header: { unix_ms: 0, precision: "milli" },
      filename: { unix_ms: 0, precision: "milli" },
    },
    duplicate_producer_id: false,
    header_invalid: null,
    filename_invalid: null,
    eligible: true,
    capabilities: { read: true, save: false, trash: false, reason: "x" },
    navigation: {
      group_id: "group",
      header: local,
      filename: local,
      included: local,
      authority: null,
    },
  },
  instant: { unix_ms: 0, precision: "milli" },
  misplaced: false,
};
async function shell(withSource = false) {
  const base = navigationFake();
  const bridge = {
    ...base,
    ...(withSource && {
      listSources: async () => ({
        ...(await base.listSources("import")),
        items: [sourceItem],
        groups: [{ id: "group", alias: "Diary", type: "ayumi" }],
      }),
    }),
    ...settingsFake(),
    metadata: async () => ({ version: "1.0.0" }),
    connect: async () => status,
    status: async () => status,
    watch: async () => async () => {},
    listAuthentic: async () => ({ items: [], next_cursor: null }),
    getSymbol: async () => ({ status: "ok" as const, png_base64: "AQ==" }),
  } as unknown as Bridge;
  const view = render(<App bridge={bridge} />);
  fireEvent.click(screen.getByRole("button", { name: "Connect" }));
  await screen.findByText("Connected");
  return view;
}

describe("accessible interaction", () => {
  it("gives every interactive control a non-empty accessible name", async () => {
    const { container } = await shell();
    const controls = container.querySelectorAll(
      'button, [role="tab"], [role="separator"], input, textarea, [contenteditable="true"]',
    );
    expect(controls.length).toBeGreaterThan(5);
    for (const el of controls) {
      const labels = [...((el as HTMLInputElement).labels ?? [])];
      const name =
        el.getAttribute("aria-label") ||
        labels.map((label) => label.textContent).join(" ") ||
        el.getAttribute("title") ||
        el.textContent ||
        "";
      expect(name.trim(), el.outerHTML.slice(0, 100)).not.toBe("");
    }
  });
  it("orders the regions as toolbar, Icon Bar, Explorer, Tabbed Editor, Global Messages", async () => {
    await shell();
    const order = [
      "Main Toolbar Strip",
      "Icon Bar",
      "Explorer",
      "Tabbed Editor",
      "Global Messages",
    ].map((label) => screen.getAllByLabelText(label)[0]);
    for (let i = 1; i < order.length; i++)
      expect(
        order[i - 1].compareDocumentPosition(order[i]) &
          Node.DOCUMENT_POSITION_FOLLOWING,
        order[i].getAttribute("aria-label") ?? "",
      ).toBeTruthy();
  });
  it("keeps focus on the Include checkbox when it is toggled", async () => {
    await shell(true);
    fireEvent.click(screen.getByRole("button", { name: "Snippet tab" }));
    fireEvent.click(screen.getByRole("button", { name: "Refresh snippets" }));
    const box = await screen.findByRole("checkbox", {
      name: "Include 2024/02/29/one.md",
    });
    box.focus();
    const before = (box as HTMLInputElement).checked;
    fireEvent.click(box);
    await waitFor(() =>
      expect((box as HTMLInputElement).checked).toBe(!before),
    );
    expect(document.activeElement).toBe(box);
  });
});

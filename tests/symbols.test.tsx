// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { ToolbarButton } from "../src/components/standard-layout/ToolbarButton";
import { App } from "../src/components/App";
import {
  SymbolLabel,
  SymbolProvider,
  type SymbolHost,
} from "../src/components/Symbol";
import { readFileSync } from "node:fs";
import { symbolDto } from "../src/adapters/validation";
import type { Bridge, SymbolResult } from "../src/application/bridge";
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
const ok: SymbolResult = { status: "ok", png_base64: "AQ==" };
afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});
function appBridge(getSymbol?: Bridge["getSymbol"]): Bridge {
  return {
    ...navigationFake(),
    ...settingsFake(),
    metadata: async () => ({ version: "1.0.0" }),
    connect: async () => status,
    status: async () => status,
    watch: async () => async () => {},
    listAuthentic: async () => ({ items: [], next_cursor: null }),
    getSymbol,
  } as unknown as Bridge;
}
const host = (load: SymbolHost["load"], unsupported = vi.fn()): SymbolHost => ({
  load,
  unsupported,
});

describe("symbolDto", () => {
  it("accepts ok and unsupported", () => {
    expect(symbolDto({ status: "ok", png_base64: "AQ==", x: 1 })).toEqual(ok);
    expect(symbolDto({ status: "unsupported" })).toEqual({
      status: "unsupported",
    });
  });
  it("rejects a malformed or oversized png field", () => {
    for (const png_base64 of ["", "not base64!", "A".repeat(43_701), 5])
      expect(() => symbolDto({ status: "ok", png_base64 })).toThrow();
    expect(() => symbolDto({ status: "other" })).toThrow();
  });
});

describe("SymbolLabel", () => {
  it("renders decoration as aria-hidden and keeps the text label", async () => {
    const h = host(async () => ok);
    const { container } = render(
      <SymbolProvider value={h}>
        <button>
          <SymbolLabel name="gearshape" label="Settings" />
        </button>
      </SymbolProvider>,
    );
    await waitFor(() =>
      expect(container.querySelector(".symbol")).not.toBeNull(),
    );
    expect(
      container.querySelector(".symbol")?.getAttribute("aria-hidden"),
    ).toBe("true");
    expect(screen.getByRole("button", { name: "Settings" })).toBeTruthy();
  });
  it("shares one native call between simultaneous and repeat mounts", async () => {
    const load = vi.fn(async () => ok);
    const h = host(load);
    const tree = (
      <SymbolProvider value={h}>
        <SymbolLabel name="xmark" label="a" />
        <SymbolLabel name="xmark" label="b" />
      </SymbolProvider>
    );
    const first = render(tree);
    await waitFor(() => expect(load).toHaveBeenCalledTimes(1));
    first.unmount();
    render(tree);
    await Promise.resolve();
    expect(load).toHaveBeenCalledTimes(1);
  });
  it("shows the fallback text and reports an unsupported symbol", async () => {
    const unsupported = vi.fn();
    const h = host(async () => ({ status: "unsupported" }), unsupported);
    render(
      <SymbolProvider value={h}>
        <SymbolLabel name="xmark" label="Close x" fallback="×" />
      </SymbolProvider>,
    );
    expect(screen.getByText("×")).toBeTruthy();
    await waitFor(() => expect(unsupported).toHaveBeenCalledWith("xmark"));
  });
  it("falls back on a command error and retries on the next mount", async () => {
    const load = vi
      .fn<SymbolHost["load"]>()
      .mockRejectedValueOnce(new Error("symbol_unavailable"))
      .mockResolvedValue(ok);
    const unsupported = vi.fn();
    const h = host(load, unsupported);
    const tree = (
      <SymbolProvider value={h}>
        <SymbolLabel name="command" label="Palette" />
      </SymbolProvider>
    );
    const first = render(tree);
    await waitFor(() => expect(load).toHaveBeenCalledTimes(1));
    expect(screen.getByText("Palette")).toBeTruthy();
    expect(unsupported).not.toHaveBeenCalled();
    first.unmount();
    const { container } = render(tree);
    await waitFor(() =>
      expect(container.querySelector(".symbol")).not.toBeNull(),
    );
    expect(load).toHaveBeenCalledTimes(2);
  });
});

describe("toolbar bitmaps", () => {
  it("uses distinct custom bitmap buttons with names, tooltips and disabled states", () => {
    const load = vi.fn(async () => ok);
    render(<App bridge={appBridge(load)} />);
    const toolbar = screen.getByLabelText("Main Toolbar Strip");
    const buttons = [...toolbar.querySelectorAll("button")];
    expect(buttons).toHaveLength(10);
    const icons = buttons.map((button) => {
      expect(button.getAttribute("aria-label")).toBeTruthy();
      expect(button.getAttribute("title")).toBeTruthy();
      expect(button.textContent).toBe("");
      const icon = button.querySelector('.bitmap-icon[aria-hidden="true"]');
      expect(icon).not.toBeNull();
      expect(icon?.getAttribute("style")).toContain("/toolbar/");
      return icon?.getAttribute("style");
    });
    expect(new Set(icons).size).toBe(buttons.length);
    expect(toolbar.querySelector(".symbol")).toBeNull();
    for (const name of ["Create Empty", "Command palette", "Settings"])
      expect(screen.getByRole("button", { name })).toBeTruthy();
    const generate = screen.getByRole("button", { name: "Generate" });
    expect((generate as HTMLButtonElement).disabled).toBe(true);
    expect(generate.getAttribute("title")).toMatch(
      /Generation is (unavailable|not supported by this server)/u,
    );
    expect(
      screen
        .getByRole("button", { name: "Create Empty" })
        .getAttribute("title"),
    ).toBeTruthy();
    expect(
      screen.getByRole("button", { name: "Settings" }).getAttribute("title"),
    ).toBeTruthy();
  });
  it("covers all eight toolbar actions, including conditional Save, with unique PNGs", () => {
    const actions = [
      ["createEmpty", "Create Empty", "create-empty"],
      ["exportIcloud", "Export", "export-icloud"],
      ["exportNotion", "Notion", "export-notion"],
      ["commandPalette", "Command palette", "command-palette"],
      ["trashRecovery", "Trash Recovery", "trash-recovery"],
      ["aiActivities", "AI Activities", "ai-activities"],
      ["save", "Save", "save"],
      ["generate", "Generate", "generate"],
    ] as const;
    render(
      <div>
        {actions.map(([action, label]) => (
          <ToolbarButton
            key={action}
            action={action}
            label={label}
            title={label}
          />
        ))}
      </div>,
    );
    const files = actions.map(([, label, file]) => {
      const button = screen.getByRole("button", { name: label });
      expect(
        button.querySelector(".bitmap-icon")?.getAttribute("style"),
      ).toContain(`${file}.png`);
      const png = readFileSync(`src/assets/toolbar/${file}.png`);
      expect(png.subarray(1, 4).toString()).toBe("PNG");
      return png.toString("base64");
    });
    expect(new Set(files).size).toBe(8);
  });
  it("keeps toolbar icons when native symbols are unavailable", () => {
    const load = vi.fn(async (): Promise<SymbolResult> => ({
      status: "unsupported",
    }));
    render(<App bridge={appBridge(load)} />);
    expect(
      screen
        .getByLabelText("Main Toolbar Strip")
        .querySelectorAll(".bitmap-icon"),
    ).toHaveLength(10);
    expect(screen.getByRole("button", { name: "Settings" })).toBeTruthy();
  });
  it("keeps accessible names without a symbol bridge", () => {
    render(<App bridge={appBridge(undefined)} />);
    expect(screen.getByRole("button", { name: "Settings" })).toBeTruthy();
  });
});

describe("symbol styling", () => {
  it("tints the template mask with the current text color so disabled state applies", () => {
    const css = readFileSync("src/styles.css", "utf8");
    const rule = css.match(/\.symbol\s*\{[^}]*\}/u)?.[0] ?? "";
    expect(rule).toMatch(/background-color:\s*currentColor/u);
    expect(rule).toMatch(/mask-size:\s*contain/u);
    const bitmapRule = css.match(/^\.bitmap-icon\s*\{[^}]*\}/mu)?.[0] ?? "";
    expect(bitmapRule).toMatch(/mask-mode:\s*alpha/u);
    expect(bitmapRule).toMatch(/background-color:\s*currentColor/u);
  });
});

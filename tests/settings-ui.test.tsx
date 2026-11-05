import { navigationFake } from "./navigation-fake";
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
import type { Bridge } from "../src/application/bridge";
import { settingsFake, type SettingsFake } from "./settings-fake";

const status = {
  instance_id: "instance",
  workspace_key: "workspace",
  server_version: "1.0.0",
  api_version: 1 as const,
  api_minor: 0,
  storage_ready: true,
  capabilities: ["status", "sessions", "jobs", "events"],
};
afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});
function bridge(fake: SettingsFake): Bridge {
  return {
    ...navigationFake(),
    metadata: async () => ({ version: "1.0.0" }),
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
    ...fake,
  };
}
function languages(list: string[]) {
  vi.spyOn(navigator, "languages", "get").mockReturnValue(list);
}
async function openSettings(section?: string) {
  fireEvent.click(screen.getByRole("button", { name: "Settings" }));
  if (section) fireEvent.click(screen.getByRole("button", { name: section }));
}

describe("Settings", () => {
  it("stores the first system language once and keeps it across a restart", async () => {
    languages(["ja-JP", "en-US"]);
    const fake = settingsFake();
    render(<App bridge={bridge(fake)} />);
    await waitFor(() => expect(fake.user.writing_locale).toBe("ja-JP"));
    expect(fake.updates.map((u) => u.section)).toEqual(["writing_init"]);
    cleanup();
    languages(["ko-KR"]);
    render(<App bridge={bridge(fake)} />);
    await openSettings("Writing Language");
    await screen.findByText(/Current: ja-JP/u);
    expect(fake.updates).toHaveLength(1);
  });

  it("shows the error and the two shortcuts when the language is unresolved", async () => {
    languages(["???"]);
    const fake = settingsFake();
    render(<App bridge={bridge(fake)} />);
    await openSettings("Writing Language");
    await screen.findByText(/system language could not be resolved/u);
    expect(fake.user.writing_locale).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "ko-KR" }));
    await screen.findByText(/Current: ko-KR/u);
    expect(fake.user.writing_locale).toBe("ko-KR");
    expect(screen.queryByRole("button", { name: "en-US" })).toBeNull();
  });

  it("validates typed tags and reports a rejected one", async () => {
    languages(["en-US"]);
    const fake = settingsFake({ writing_locale: "en-US", revision: "r0" });
    render(<App bridge={bridge(fake)} />);
    await openSettings("Writing Language");
    const input = await screen.findByLabelText("Language tag");
    fireEvent.change(input, { target: { value: "not a tag" } });
    fireEvent.click(screen.getByRole("button", { name: "Save language" }));
    await screen.findByText("That value is not valid.");
    expect(fake.user.writing_locale).toBe("en-US");
    fireEvent.change(screen.getByLabelText("Language tag"), {
      target: { value: "ja-JP" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save language" }));
    await screen.findByText(/Current: ja-JP/u);
  });

  it("reloads from disk when another window changed the settings", async () => {
    languages(["en-US"]);
    const fake = settingsFake({ writing_locale: "en-US", revision: "r0" });
    render(<App bridge={bridge(fake)} />);
    await openSettings("Writing Language");
    await screen.findByText(/Current: en-US/u);
    fake.user = {
      ...fake.user,
      revision: "elsewhere",
      writing_locale: "fr-FR",
    };
    fireEvent.change(screen.getByLabelText("Language tag"), {
      target: { value: "ko-KR" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save language" }));
    await screen.findByText(/changed elsewhere/u);
    await screen.findByText(/Current: fr-FR/u);
  });

  it("shows read-only and recoverable files and recovers only after confirmation", async () => {
    languages(["en-US"]);
    const fake = settingsFake({
      revision: "bad",
      recoverable: true,
      warnings: [{ section: "file", code: "file_invalid" }],
    });
    render(<App bridge={bridge(fake)} />);
    await openSettings("Writing Language");
    await screen.findByText(/settings file could not be read/u);
    expect(fake.updates).toHaveLength(0); // no default installed over an invalid file
    fireEvent.click(screen.getByRole("button", { name: "Recover settings…" }));
    expect(fake.updates).toHaveLength(0);
    fireEvent.click(screen.getByRole("button", { name: "Confirm recovery" }));
    await waitFor(() => expect(fake.updates[0]?.recover).toBe(true));
    await waitFor(() =>
      expect(screen.queryByText(/could not be read/u)).toBeNull(),
    );
  });

  it("explains a read-only newer file and offers no language save", async () => {
    languages(["en-US"]);
    const fake = settingsFake({
      revision: "new",
      read_only: true,
      recoverable: true,
      warnings: [{ section: "file", code: "newer_schema" }],
    });
    render(<App bridge={bridge(fake)} />);
    await openSettings("Writing Language");
    await screen.findByText(/newer version/u);
    expect(
      (
        screen.getByRole("button", {
          name: "Save language",
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(true);
    expect(fake.updates).toHaveLength(0);
  });

  it("shows redacted server status and the explicit reload result", async () => {
    languages(["en-US"]);
    const fake = settingsFake(
      { writing_locale: "en-US", revision: "r0" },
      {
        sources: [
          { alias: "mobile", available: false, reason: "source_root_missing" },
        ],
      },
    );
    fake.reloadAnswer = {
      outcome: "restart_required",
      config_generation: null,
      issues: [{ path: "server", code: "restart_required" }],
    };
    render(<App bridge={bridge(fake)} />);
    await openSettings();
    await screen.findByText(
      "Connect to the server to see its configuration status.",
    );
    fireEvent.click(screen.getByRole("button", { name: "Connect" }));
    await screen.findByText("mobile: source_root_missing");
    fireEvent.click(
      screen.getByRole("button", { name: "Reload configuration" }),
    );
    await screen.findByText(/need a restart/u);
    expect(screen.getByText("server: restart_required")).toBeTruthy();
    expect(document.body.textContent).not.toMatch(/token|\/Users/u);
  });
});

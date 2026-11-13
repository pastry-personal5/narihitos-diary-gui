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
  it("keeps connection controls in Settings Server and disables Connect while connecting", async () => {
    const adapter = bridge(settingsFake());
    let finish!: (value: typeof status) => void;
    adapter.connect = vi.fn(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );
    render(<App bridge={adapter} />);
    expect(screen.queryByRole("button", { name: "Connect" })).toBeNull();
    expect(screen.queryByLabelText("Server connection status")).toBeNull();
    await openSettings("Server");
    expect(screen.getByLabelText("Server connection status").textContent).toBe(
      "Server disconnected",
    );
    fireEvent.click(screen.getByRole("button", { name: "Connect" }));
    await screen.findByText("Connecting…");
    expect(screen.getByRole("button", { name: "Connect" })).toHaveProperty(
      "disabled",
      true,
    );
    await act(async () => finish(status));
    await screen.findByText("Connected");
    expect(screen.getByRole("button", { name: "Connect" })).toHaveProperty(
      "disabled",
      false,
    );
    fireEvent.click(screen.getByRole("button", { name: "Go back" }));
    expect(screen.queryByRole("button", { name: "Connect" })).toBeNull();
    expect(screen.queryByLabelText("Server connection status")).toBeNull();
    expect(screen.queryByLabelText("Save status")).toBeNull();
  });

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
    await openSettings("Server");
    await screen.findByText(
      "Connect to the server to see its configuration status.",
    );
    connectFromSettings();
    fireEvent.click(
      screen.getByRole("button", { name: "Sources", exact: true }),
    );
    await screen.findByText("mobile: source_root_missing");
    fireEvent.click(
      screen.getByRole("button", { name: "Server", exact: true }),
    );
    fireEvent.click(
      screen.getByRole("button", { name: "Reload configuration" }),
    );
    await screen.findByText(/need a restart/u);
    expect(screen.getByText("server: restart_required")).toBeTruthy();
    expect(document.body.textContent).not.toMatch(/token|\/Users/u);
  });

  it("posts the source-import advice after a reload leaves no source available", async () => {
    languages(["en-US"]);
    const fake = settingsFake(
      { writing_locale: "en-US", revision: "r0" },
      { sources: [{ alias: "ayumi", available: true, reason: "ready" }] },
    );
    render(<App bridge={bridge(fake)} />);
    connectFromSettings();
    await screen.findByText("Connected");
    fireEvent.click(screen.getByRole("button", { name: "Go back" }));
    expect(screen.getByRole("log").textContent).not.toContain(
      "Source import is off",
    );
    await openSettings("Server");
    fake.server = {
      ...fake.server,
      sources: [
        { alias: "ayumi", available: false, reason: "source_root_missing" },
      ],
      config_generation: 2,
    };
    fake.reloadAnswer = {
      outcome: "applied",
      config_generation: 2,
      issues: [],
    };
    fireEvent.click(
      screen.getByRole("button", { name: "Reload configuration" }),
    );
    await screen.findByText("Configuration reloaded.");
    fireEvent.click(screen.getByRole("button", { name: "Go back" }));
    await waitFor(() =>
      expect(screen.getByRole("log").textContent).toContain(
        "ayumi: folder not found",
      ),
    );
  });
});

describe("Command palette and Keyboard Shortcuts", () => {
  const press = (init: KeyboardEventInit) =>
    fireEvent.keyDown(window, { bubbles: true, ...init });
  const palette = () =>
    screen.findByRole("dialog", { name: "Command palette" });

  it("opens from the toolbar and the default shortcut, filters, runs and restores focus", async () => {
    languages(["en-US"]);
    const fake = settingsFake({ writing_locale: "en-US", revision: "r0" });
    render(<App bridge={bridge(fake)} />);
    const button = screen.getByRole("button", { name: "Command palette" });
    button.focus();
    fireEvent.click(button);
    await palette();
    expect(screen.getByText("Shift+Command+P")).toBeTruthy();
    fireEvent.change(screen.getByLabelText("Filter actions"), {
      target: { value: "open set" },
    });
    expect(screen.getAllByRole("option")).toHaveLength(1);
    fireEvent.keyDown(screen.getByLabelText("Filter actions"), {
      key: "Enter",
      isComposing: true,
    });
    expect(screen.getAllByRole("option")).toHaveLength(1);
    expect(screen.queryByRole("heading", { name: "Status" })).toBeNull();
    fireEvent.keyDown(screen.getByLabelText("Filter actions"), {
      key: "Enter",
    });
    await screen.findByRole("heading", { name: "Status" });
    expect(screen.queryByRole("dialog")).toBeNull();
    cleanup();
    render(<App bridge={bridge(fake)} />);
    press({ code: "KeyP", metaKey: true, shiftKey: true });
    await palette();
    const input = screen.getByLabelText("Filter actions");
    fireEvent.keyDown(input, { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  });

  it("lists unavailable actions with a reason and posts it when run", async () => {
    languages(["en-US"]);
    const adapter = bridge(settingsFake({ revision: "r0" }));
    adapter.connect = vi.fn().mockRejectedValue(new Error("offline"));
    render(<App bridge={adapter} />);
    press({ code: "KeyP", metaKey: true, shiftKey: true });
    await palette();
    fireEvent.change(screen.getByLabelText("Filter actions"), {
      target: { value: "Export" },
    });
    // "Export" matches both destinations.
    const options = screen.getAllByRole("option");
    expect(options).toHaveLength(2);
    for (const option of options)
      expect(option.textContent).toMatch(/Connect to the server first\./u);
    fireEvent.keyDown(screen.getByLabelText("Filter actions"), {
      key: "Enter",
    });
    await screen.findByText(/Connect to the server first\./u);
  });

  it("Command+S with no file posts the reason instead of saving", async () => {
    languages(["en-US"]);
    render(<App bridge={bridge(settingsFake({ revision: "r0" }))} />);
    press({ code: "KeyS", metaKey: true });
    await screen.findByText(/No file is open\./u);
  });

  it("shows the table, captures by code, reassigns on conflict and resets", async () => {
    languages(["en-US"]);
    const fake = settingsFake({ writing_locale: "en-US", revision: "r0" });
    render(<App bridge={bridge(fake)} />);
    await openSettings("Keyboard Shortcuts");
    expect(screen.getAllByRole("row")).toHaveLength(16);
    const edit = screen.getByRole("button", { name: "Edit Open Settings" });
    await waitFor(() => expect(edit.hasAttribute("disabled")).toBe(false));
    fireEvent.click(edit);
    fireEvent.click(
      screen.getByRole("button", { name: "Add shortcut to Open Settings" }),
    );
    const box = () => screen.getByRole("textbox", { name: "Press a shortcut" });
    fireEvent.keyDown(box(), { code: "KeyC", key: "ç", metaKey: true });
    await screen.findByText(/reserved/u);
    fireEvent.keyDown(box(), { code: "KeyS", key: "ß", metaKey: true });
    await screen.findByText("Command+S is used by Save.");
    expect(
      fake.updates.filter((u) => u.section === "keybindings"),
    ).toHaveLength(0);
    fireEvent.click(screen.getByRole("button", { name: "Reassign" }));
    await waitFor(() =>
      expect(fake.updates.at(-1)?.value).toEqual({
        set: { "settings.open": ["Command+S"], "editor.save": [] },
      }),
    );
    await screen.findAllByText("Modified");
    fireEvent.click(screen.getByRole("button", { name: "Reset Save" }));
    await waitFor(() =>
      expect(fake.updates.at(-1)?.value).toEqual({
        set: { "editor.save": null },
      }),
    );
    fireEvent.click(screen.getByRole("button", { name: "Reset all" }));
    expect(fake.updates.at(-1)?.value).toEqual({
      set: { "editor.save": null },
    });
    fireEvent.click(screen.getByRole("button", { name: "Confirm reset all" }));
    await waitFor(() =>
      expect(fake.updates.at(-1)?.value).toEqual({ reset_all: true }),
    );
  });

  it("Escape cancels capture; read-only disables editing; shortcuts are spoken text", async () => {
    languages(["en-US"]);
    const fake = settingsFake({ writing_locale: "en-US", revision: "r0" });
    render(<App bridge={bridge(fake)} />);
    await openSettings("Keyboard Shortcuts");
    expect(
      screen.getAllByLabelText("Shift plus Command plus P").length,
    ).toBeGreaterThan(0);
    const edit = screen.getByRole("button", { name: "Edit Open Settings" });
    await waitFor(() => expect(edit.hasAttribute("disabled")).toBe(false));
    fireEvent.click(edit);
    fireEvent.click(
      screen.getByRole("button", { name: "Add shortcut to Open Settings" }),
    );
    fireEvent.keyDown(
      screen.getByRole("textbox", { name: "Press a shortcut" }),
      {
        key: "Escape",
      },
    );
    expect(
      screen.queryByRole("textbox", { name: "Press a shortcut" }),
    ).toBeNull();
    cleanup();
    render(
      <App
        bridge={bridge(settingsFake({ read_only: true, revision: "r0" }))}
      />,
    );
    await openSettings("Keyboard Shortcuts");
    await waitFor(() =>
      expect(
        screen
          .getByRole("button", { name: "Edit Open Settings" })
          .hasAttribute("disabled"),
      ).toBe(true),
    );
  });
});

it("persists generation count and reloads the saved value", async () => {
  const fake = settingsFake({ writing_locale: "en-US" });
  const app = render(<App bridge={bridge(fake)} />);
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "Settings" })).toHaveProperty(
      "disabled",
      false,
    ),
  );
  await openSettings("Generation");
  const count = screen.getByLabelText("Candidates per generation");
  await waitFor(() => expect(count).toHaveProperty("value", "3"));
  await waitFor(() => expect(count).toHaveProperty("disabled", false));
  fireEvent.change(count, { target: { value: "5" } });
  await waitFor(() => expect(fake.user.generation_count).toBe(5));
  expect(fake.updates.at(-1)).toMatchObject({
    section: "generation",
    value: { count: 5 },
    recover: false,
  });
  app.unmount();
  render(<App bridge={bridge(fake)} />);
  await openSettings("Generation");
  await waitFor(() =>
    expect(screen.getByLabelText("Candidates per generation")).toHaveProperty(
      "value",
      "5",
    ),
  );
});

it("close_failure_keeps_window_available and reports the server failure", async () => {
  const fake = settingsFake({ writing_locale: "en-US" });
  const b = bridge(fake);
  let close: () => Promise<boolean> = async () => false,
    failed = () => {};
  b.onCloseRequested = async (handler, onFailure) => {
    close = handler;
    failed = onFailure ?? (() => {});
    return () => {};
  };
  render(<App bridge={b} />);
  await waitFor(() =>
    expect(screen.getByText(/Connected to the Diary server/u)).toBeTruthy(),
  );
  let proceed = false;
  await act(async () => {
    proceed = await close();
  });
  expect(proceed).toBe(true);
  expect(screen.getByRole("main", { hidden: true }).hasAttribute("inert")).toBe(
    true,
  );
  await act(async () => {
    failed();
  });
  expect(screen.getByRole("main").hasAttribute("inert")).toBe(false);
  expect(
    screen.getByText(/The server could not confirm closing/u),
  ).toBeTruthy();
});

it("groups configuration and profiles in their requested categories and reveals the folder", async () => {
  const adapter = bridge(
    settingsFake(
      { writing_locale: "en-US" },
      {
        sources: [{ alias: "mobile", available: true, reason: "ready" }],
        profiles: [{ alias: "main", available: true, reason: "ready" }],
      },
    ),
  );
  let finish!: () => void;
  adapter.revealConfigurationFolder = vi.fn(
    () =>
      new Promise<void>((resolve) => {
        finish = resolve;
      }),
  );
  render(<App bridge={adapter} />);
  expect(screen.queryByLabelText("Chat profile")).toBeNull();
  expect(screen.queryByLabelText("Generation profile")).toBeNull();
  await openSettings("Sources");
  expect(
    screen.getByRole("button", {
      name: "Reveal configuration folder in Finder",
    }),
  ).toHaveProperty("disabled", true);
  connectFromSettings();
  await screen.findByText("Connected");
  fireEvent.click(screen.getByRole("button", { name: "Status", exact: true }));
  expect(screen.queryByRole("heading", { name: "Server" })).toBeNull();
  expect(screen.queryByRole("heading", { name: "Sources" })).toBeNull();
  expect(screen.queryByRole("heading", { name: "Agent profiles" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Sources", exact: true }));
  await screen.findByText("mobile: available");
  fireEvent.click(
    screen.getByRole("button", {
      name: "Reveal configuration folder in Finder",
    }),
  );
  expect(
    screen.getByRole("button", { name: "Opening Finder…" }),
  ).toHaveProperty("disabled", true);
  expect(adapter.revealConfigurationFolder).toHaveBeenCalledTimes(1);
  await act(async () => finish());
  adapter.revealConfigurationFolder = vi
    .fn()
    .mockRejectedValue(new Error("/private/secret"));
  fireEvent.click(
    screen.getByRole("button", {
      name: "Reveal configuration folder in Finder",
    }),
  );
  await screen.findByText(/Could not open the server configuration folder/u);
  expect(document.body.textContent).not.toContain("/private/secret");
  fireEvent.click(
    screen.getByRole("button", { name: "AI Agents", exact: true }),
  );
  expect(screen.getByRole("heading", { name: "Chat profile" })).toBeTruthy();
  expect(
    screen.getByRole("heading", { name: "Generation profile" }),
  ).toBeTruthy();
  expect(screen.getByRole("heading", { name: "Agent profiles" })).toBeTruthy();
  expect(screen.getByText("main: available")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Go back" }));
  expect(screen.queryByLabelText("Chat profile")).toBeNull();
  expect(screen.queryByLabelText("Generation profile")).toBeNull();
});

it("orders Settings categories and persists independent ready indicator toggles", async () => {
  const fake = settingsFake({ writing_locale: "en-US" });
  const adapter = bridge(fake);
  const first = render(<App bridge={adapter} />);
  await waitFor(() =>
    expect(screen.getByLabelText("AI Activities status")).toBeTruthy(),
  );
  await openSettings("Status Line");
  const nav = screen.getByRole("navigation", { name: "Settings sections" });
  expect(
    within(nav)
      .getAllByRole("button")
      .map((b) => b.getAttribute("aria-label") ?? b.textContent),
  ).toEqual([
    "Go back",
    "Server",
    "Status",
    "Sources",
    "AI Agents",
    "Writing Language",
    "Generation",
    "Status Line",
    "Keyboard Shortcuts",
  ]);
  const chat = screen.getByRole("checkbox", { name: "Chat ready" });
  await waitFor(() => expect(chat).toHaveProperty("disabled", false));
  fireEvent.click(chat);
  await waitFor(() => expect(chat).toHaveProperty("checked", false));
  expect(
    screen.getByRole("checkbox", { name: "Generate ready" }),
  ).toHaveProperty("checked", true);
  expect(screen.getByRole("checkbox", { name: "Review ready" })).toHaveProperty(
    "checked",
    true,
  );
  first.unmount();
  render(<App bridge={adapter} />);
  await openSettings("Status Line");
  await waitFor(() =>
    expect(screen.getByRole("checkbox", { name: "Chat ready" })).toHaveProperty(
      "checked",
      false,
    ),
  );
  expect(fake.user.status_line).toEqual({
    generate_ready: true,
    review_ready: true,
    chat_ready: false,
  });
});

it("hides and restores the ready label without hiding operational status", async () => {
  const fake = settingsFake({ writing_locale: "en-US" });
  const adapter = bridge(fake);
  adapter.listProfiles = async () => [
    {
      alias: "main",
      type: "codex",
      status: "ready",
      reason: null,
      capabilities: {
        chat: { available: true, reason: null },
        generate: { available: true, reason: null },
      },
    },
  ];
  adapter.openChat = async () => ({ chat_id: "chat" });
  adapter.sendChat = async () => ({ job_id: "job" });
  adapter.getChat = async () => ({
    chat_id: "chat",
    profile: "main",
    type: "codex",
    locale: "en-US",
    transcript: [],
    turn: null,
  });
  render(<App bridge={adapter} />);
  const footer = document.querySelector("footer")!;
  await waitFor(() =>
    expect(within(footer).getByText("Chat ready")).toBeTruthy(),
  );
  // Without a selected paragraph or generation inputs, those actions are not ready.
  expect(within(footer).queryByText("Review ready")).toBeNull();
  expect(within(footer).queryByText("Generate ready")).toBeNull();
  await openSettings("Status Line");
  const toggle = screen.getByRole("checkbox", { name: "Chat ready" });
  fireEvent.click(toggle);
  await waitFor(() =>
    expect(within(footer).queryByText("Chat ready")).toBeNull(),
  );
  expect(within(footer).getByLabelText("AI Activities status")).toBeTruthy();
  fireEvent.click(toggle);
  await waitFor(() =>
    expect(within(footer).getByText("Chat ready")).toBeTruthy(),
  );
});

it("saves independent default-on agent progress toggles and restores them after restart", async () => {
  const fake = settingsFake({ writing_locale: "en-US" });
  const adapter = bridge(fake);
  adapter.listProfiles = async () =>
    ["main", "local"].map((alias) => ({
      alias,
      type: "codex",
      status: "ready",
      reason: null,
      capabilities: {
        chat: { available: true, reason: null },
        generate: { available: true, reason: null },
      },
    }));
  const first = render(<App bridge={adapter} />);
  connectFromSettings();
  fireEvent.click(
    screen.getByRole("button", { name: "AI Agents", exact: true }),
  );
  const main = await screen.findByRole("checkbox", {
    name: "Show progress using Global Messages for main",
  });
  expect(main).toHaveProperty("checked", true);
  expect(
    screen.getByRole("checkbox", {
      name: "Show progress using Global Messages for local",
    }),
  ).toHaveProperty("checked", true);
  fireEvent.click(main);
  await waitFor(() =>
    expect(fake.user.agent_progress).toEqual({ main: false }),
  );
  expect(main).toHaveProperty("checked", false);
  first.unmount();
  render(<App bridge={adapter} />);
  connectFromSettings();
  fireEvent.click(
    screen.getByRole("button", { name: "AI Agents", exact: true }),
  );
  expect(
    await screen.findByRole("checkbox", {
      name: "Show progress using Global Messages for main",
    }),
  ).toHaveProperty("checked", false);
  expect(
    screen.getByRole("checkbox", {
      name: "Show progress using Global Messages for local",
    }),
  ).toHaveProperty("checked", true);
  fireEvent.click(
    screen.getByRole("checkbox", {
      name: "Show progress using Global Messages for local",
    }),
  );
  await waitFor(() =>
    expect(fake.user.agent_progress).toEqual({ main: false, local: false }),
  );
});

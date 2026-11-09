import { navigationFake } from "./navigation-fake";
// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  act,
  renderHook,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import {
  checkDelay,
  useProfiles,
} from "../src/components/ai-agents/useProfiles";
import { App } from "../src/components/App";
import {
  AuthenticError,
  type Bridge,
  type ProfileInfo,
  type ProfileState,
} from "../src/application/bridge";
import { profileListDto } from "../src/adapters/validation";
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
const cap = { available: true, reason: null };
function profile(
  alias: string,
  state: ProfileState,
  type = "codex",
): ProfileInfo {
  return {
    alias,
    type,
    status: state,
    reason: null,
    capabilities: { chat: cap, generate: cap },
  };
}
afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});
function bridge(extra: Partial<Bridge>): Bridge {
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
    refreshAuthentic: async () => ({ files: 0, problems: 0 }),
    ...settingsFake({ writing_locale: "en-US" }),
    ...extra,
  } as Bridge;
}
async function open() {
  fireEvent.click(screen.getByRole("button", { name: "Settings" }));
  fireEvent.click(
    screen.getByRole("button", { name: "AI Agents", exact: true }),
  );
  const table = await screen.findByRole("table", { name: "Profile status" });
  await waitFor(() =>
    expect(within(table).getAllByRole("row").length).toBeGreaterThan(1),
  );
  return table;
}

describe("Profile status in Settings", () => {
  it("shows each status in plain words", async () => {
    const all: ProfileState[] = [
      "unchecked",
      "missing",
      "unsupported_version",
      "unavailable",
      "available_auth_unknown",
      "unauthenticated",
      "ready",
      "busy",
      "failed",
    ];
    render(
      <App
        bridge={bridge({
          listProfiles: async () => all.map((s) => profile(`p-${s}`, s)),
        })}
      />,
    );
    const items = within(await open())
      .getAllByRole("row")
      .slice(1);
    expect(items.map((i) => i.textContent)).toEqual([
      expect.stringContaining("not checked yet"),
      expect.stringContaining("command not found"),
      expect.stringContaining("unsupported version"),
      expect.stringContaining("unavailable"),
      expect.stringContaining("sign-in unknown"),
      expect.stringContaining("not signed in"),
      expect.stringContaining("ready"),
      expect.stringContaining("busy"),
      expect.stringContaining("check failed"),
    ]);
  });

  it("explains an unavailable capability for a profile without an adapter", async () => {
    const claude = profile("c", "unavailable", "claude");
    claude.capabilities = {
      chat: { available: false, reason: "adapter_unimplemented" },
      generate: { available: false, reason: "assets_missing" },
    };
    render(<App bridge={bridge({ listProfiles: async () => [claude] })} />);
    expect((await open()).textContent).toContain("no adapter yet");
  });

  it("lists what blocks Chat and Generate in the Advanced card", async () => {
    const p = profile("c", "unavailable", "claude");
    p.capabilities = {
      chat: { available: false, reason: "assets_missing" },
      generate: { available: false, reason: "assets_missing" },
    };
    render(<App bridge={bridge({ listProfiles: async () => [p] })} />);
    await open();
    fireEvent.click(
      screen.getByRole("button", { name: "Status", exact: true }),
    );
    await waitFor(() =>
      expect(screen.getByLabelText("Advanced").textContent).toContain(
        "No profile can chat",
      ),
    );
    const advanced = screen.getByLabelText("Advanced");
    expect(advanced.textContent).toContain("No profile can chat");
    expect(advanced.textContent).toContain("No profile can generate");
  });

  it("shows the reason an Apple profile is unavailable", async () => {
    const apple = {
      ...profile("mac", "unavailable", "apple-foundation-models"),
      reason: "apple_intelligence_disabled",
    };
    render(<App bridge={bridge({ listProfiles: async () => [apple] })} />);
    expect((await open()).textContent).toContain(
      "Apple Intelligence is turned off in System Settings",
    );
  });

  it("checks every profile in the background after launch, showing progress", async () => {
    const release: Record<string, (p: ProfileInfo) => void> = {};
    const checkProfile = vi.fn(
      (alias: string) => new Promise<ProfileInfo>((r) => (release[alias] = r)),
    );
    render(
      <App
        bridge={bridge({
          listProfiles: async () => [
            profile("main", "unchecked"),
            profile("other", "unchecked"),
          ],
          checkProfile,
        })}
      />,
    );
    await open();
    await waitFor(() => expect(checkProfile).toHaveBeenCalledTimes(2));
    expect(checkProfile).toHaveBeenCalledWith("main");
    expect(checkProfile).toHaveBeenCalledWith("other");
    expect(screen.getAllByText(/checking…/u)).toHaveLength(2);
    expect(
      (screen.getByRole("button", { name: "Check main" }) as HTMLButtonElement)
        .disabled,
    ).toBe(true);
    release.main(profile("main", "ready"));
    await waitFor(() =>
      expect(screen.getAllByText(/checking…/u)).toHaveLength(1),
    );
    const first = within(
      screen.getByRole("table", { name: "Profile status" }),
    ).getAllByRole("row")[1];
    expect(first.textContent).toContain("ready");
  });

  it("keeps independent profile choices when Settings closes and reopens", async () => {
    render(
      <App
        bridge={bridge({
          listProfiles: async () => [
            profile("first", "ready"),
            profile("second", "ready"),
          ],
        })}
      />,
    );
    await open();
    const chat = screen.getByRole("combobox", { name: "Chat profile" });
    const paragraph = screen.getByRole("combobox", {
      name: "Paragraph Review profile",
    });
    const generation = screen.getByRole("combobox", {
      name: "Generation profile",
    });
    fireEvent.change(chat, { target: { value: "second" } });
    await waitFor(() => expect(chat).toHaveProperty("value", "second"));
    expect(generation).toHaveProperty("value", "first");
    expect(paragraph).toHaveProperty("value", "first");
    fireEvent.change(paragraph, { target: { value: "second" } });
    fireEvent.change(chat, { target: { value: "first" } });
    await waitFor(() => expect(chat).toHaveProperty("value", "first"));
    expect(paragraph).toHaveProperty("value", "second");
    fireEvent.change(chat, { target: { value: "second" } });
    await waitFor(() => expect(chat).toHaveProperty("value", "second"));
    fireEvent.change(generation, { target: { value: "second" } });
    fireEvent.click(screen.getByRole("button", { name: "Go back" }));
    expect(screen.queryByRole("combobox", { name: "Chat profile" })).toBeNull();
    await open();
    expect(
      screen.getByRole("combobox", { name: "Paragraph Review profile" }),
    ).toHaveProperty("value", "second");
    expect(
      screen.getByRole("combobox", { name: "Chat profile" }),
    ).toHaveProperty("value", "second");
    expect(
      screen.getByRole("combobox", { name: "Generation profile" }),
    ).toHaveProperty("value", "second");
  });

  it("backs off exponentially, capped at one hour", () => {
    expect([0, 1, 2, 3, 7, 8, 9, 40].map(checkDelay)).toEqual(
      [15, 30, 60, 120, 1920, 3600, 3600, 3600].map((s) => s * 1000),
    );
  });

  it("reports a failed check without losing the list", async () => {
    render(
      <App
        bridge={bridge({
          listProfiles: async () => [profile("main", "unchecked")],
          checkProfile: async () => {
            throw new AuthenticError("unavailable", true);
          },
        })}
      />,
    );
    await open();
    await screen.findByText("The check could not be completed.");
    expect(screen.getByRole("button", { name: "Check main" })).toBeTruthy();
  });

  it("keeps Settings usable when the list cannot be read", async () => {
    render(
      <App
        bridge={bridge({
          listProfiles: async () => {
            throw new AuthenticError("unavailable", true);
          },
          checkProfile: async () => profile("main", "ready"),
        })}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    fireEvent.click(
      screen.getByRole("button", { name: "AI Agents", exact: true }),
    );
    await screen.findByText("The profile status could not be read.");
    fireEvent.click(
      screen.getByRole("button", { name: "Server", exact: true }),
    );
    expect(
      screen.getByRole("button", { name: "Reload configuration" }),
    ).toBeTruthy();
  });

  it("omits the list when the bridge has no profile commands", async () => {
    render(<App bridge={bridge({})} />);
    fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    fireEvent.click(
      screen.getByRole("button", { name: "Server", exact: true }),
    );
    await screen.findByRole("button", { name: "Reload configuration" });
    expect(screen.queryByRole("table", { name: "Profile status" })).toBeNull();
  });
});

describe("profile DTO validation", () => {
  const good = profile("main", "ready");
  it("accepts a sound list and rejects out-of-vocabulary values", () => {
    expect(profileListDto({ profiles: [good] })).toEqual([good]);
    const bad = (p: unknown) =>
      expect(() => profileListDto({ profiles: [p] })).toThrow();
    bad({ ...good, status: "/Users/x/secret" });
    bad({ ...good, alias: "" });
    bad({
      ...good,
      capabilities: {
        ...good.capabilities,
        chat: { available: false, reason: "free text" },
      },
    });
    bad({
      ...good,
      capabilities: {
        ...good.capabilities,
        chat: { available: true, reason: "assets_missing" },
      },
    });
    expect(() => profileListDto({ profiles: "x" })).toThrow();
  });
});

it("retries busy checks at the base interval even after backoff", async () => {
  vi.useFakeTimers();
  const checkProfile = vi.fn().mockResolvedValue(profile("agent", "ready"));
  const b = bridge({
    listProfiles: async () => [profile("agent", "ready")],
    checkProfile,
  });
  const { unmount } = renderHook(() => useProfiles(b, true));
  try {
    await act(async () => {
      await vi.advanceTimersByTimeAsync(0);
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(15_000);
    });
    checkProfile.mockResolvedValue(profile("agent", "busy"));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(30_000);
    });
    const count = checkProfile.mock.calls.length;
    checkProfile.mockResolvedValue(profile("agent", "ready"));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(15_000);
    });
    expect(checkProfile).toHaveBeenCalledTimes(count + 1);
  } finally {
    unmount();
    vi.useRealTimers();
  }
});

import { navigationFake } from "./navigation-fake";
// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
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
  return screen.findByRole("list", { name: "Profile status" });
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
    const items = within(await open()).getAllByRole("listitem");
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
    expect(screen.getByText(/Chat and Generate are not enabled/u)).toBeTruthy();
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

  it("checks one profile, showing progress, then the new status", async () => {
    let release!: (p: ProfileInfo) => void;
    const checkProfile = vi.fn(
      () => new Promise<ProfileInfo>((r) => (release = r)),
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
    fireEvent.click(screen.getByRole("button", { name: "Check main" }));
    await screen.findByText(/checking…/u);
    expect(checkProfile).toHaveBeenCalledWith("main");
    expect(
      (screen.getByRole("button", { name: "Check main" }) as HTMLButtonElement)
        .disabled,
    ).toBe(true);
    // Settings stays usable and the other profile can still be checked.
    expect(
      (screen.getByRole("button", { name: "Check other" }) as HTMLButtonElement)
        .disabled,
    ).toBe(false);
    release(profile("main", "ready"));
    await waitFor(() => expect(screen.queryByText(/checking…/u)).toBeNull());
    const first = within(
      screen.getByRole("list", { name: "Profile status" }),
    ).getAllByRole("listitem")[0];
    expect(first.textContent).toContain("ready");
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
    fireEvent.click(screen.getByRole("button", { name: "Check main" }));
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
    await screen.findByText("The profile status could not be read.");
    expect(
      screen.getByRole("button", { name: "Reload configuration" }),
    ).toBeTruthy();
  });

  it("omits the list when the bridge has no profile commands", async () => {
    render(<App bridge={bridge({})} />);
    fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    await screen.findByRole("button", { name: "Reload configuration" });
    expect(screen.queryByRole("list", { name: "Profile status" })).toBeNull();
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

import { generationView, candidateView } from "./generation-fixtures";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
const mocks = vi.hoisted(() => ({
  invoke: vi.fn(),
  channels: [] as { onmessage: (value: unknown) => void }[],
}));
vi.mock("@tauri-apps/api/core", () => ({
  invoke: mocks.invoke,
  Channel: class {
    onmessage: (value: unknown) => void = () => {};
    constructor() {
      mocks.channels.push(this);
    }
  },
}));
import { logClientEvent, tauriBridge } from "../src/adapters/tauri-bridge";
import { streamEvent } from "../src/application/events";
beforeEach(() => {
  vi.useFakeTimers();
  mocks.invoke.mockReset().mockResolvedValue(undefined);
  mocks.channels.length = 0;
});
afterEach(() => vi.useRealTimers());
describe("session/channel adapter lifecycle", () => {
  it("heartbeats without overlap, suppresses late callbacks and closes once", async () => {
    const observer = vi.fn();
    const stop = await tauriBridge.watch(observer);
    expect(mocks.invoke.mock.calls.map((call) => call[0])).toEqual([
      "open_session",
      "subscribe_events",
    ]);
    let resolve: () => void = () => {};
    mocks.invoke.mockImplementation((name: string) =>
      name === "heartbeat_session"
        ? new Promise<void>((r) => {
            resolve = r;
          })
        : Promise.resolve(),
    );
    await vi.advanceTimersByTimeAsync(15000);
    expect(
      mocks.invoke.mock.calls.filter((call) => call[0] === "heartbeat_session"),
    ).toHaveLength(1);
    await stop();
    await stop();
    resolve();
    await vi.advanceTimersByTimeAsync(15000);
    mocks.channels[0].onmessage({ kind: "disconnected" });
    expect(observer).not.toHaveBeenCalled();
    expect(
      mocks.invoke.mock.calls.filter((call) => call[0] === "close_session"),
    ).toHaveLength(1);
  });
  it("closes admitted session when subscription setup fails", async () => {
    mocks.invoke.mockImplementation((name: string) =>
      name === "subscribe_events"
        ? Promise.reject(new Error("invented secret"))
        : Promise.resolve(),
    );
    await expect(tauriBridge.watch(vi.fn())).rejects.toThrow(
      "Subscription unavailable",
    );
    expect(mocks.invoke).toHaveBeenCalledWith("close_session");
    expect(vi.getTimerCount()).toBe(0);
  });
  it("resubscribes for snapshot refresh after a replay gap", async () => {
    const observer = vi.fn();
    const stop = await tauriBridge.watch(observer);
    mocks.channels[0].onmessage({
      subscription_id: "07249746-e280-4d51-a86a-cbcb198ee101",
      delivery_id: 1,
      event: {
        kind: "resync_required",
        instance_id: "instance",
        session_id: "session",
      },
    });
    await Promise.resolve();
    expect(
      mocks.invoke.mock.calls.filter(
        (call) =>
          call[0] === "subscribe_events" &&
          call[1]?.request?.action === "start",
      ),
    ).toHaveLength(2);
    await stop();
  });
  it("reports heartbeat failures without exposing rejected details", async () => {
    const observer = vi.fn();
    const stop = await tauriBridge.watch(observer);
    mocks.invoke.mockRejectedValue(new Error("invented private path"));
    await vi.advanceTimersByTimeAsync(5000);
    expect(observer).toHaveBeenCalledWith({ kind: "disconnected" });
    await stop();
  });
  it("validates bounded snapshots and strips additive fields", () => {
    expect(
      streamEvent({
        kind: "job_snapshot",
        instance_id: "i",
        session_id: "s",
        jobs: [{ id: "j", sequence: 1, state: "running", token: "invented" }],
        first: true,
        complete: true,
      }),
    ).toEqual({
      kind: "job_snapshot",
      instance_id: "i",
      session_id: "s",
      jobs: [{ id: "j", sequence: 1, state: "running" }],
      first: true,
      complete: true,
    });
    expect(() =>
      streamEvent({
        kind: "job_snapshot",
        instance_id: "i",
        session_id: "s",
        jobs: Array(201).fill({}),
        first: true,
        complete: true,
      }),
    ).toThrow();
  });
});

describe("delivery acknowledgements", () => {
  const subscription = "07249746-e280-4d51-a86a-cbcb198ee101";
  it("acknowledges validated messages once and suppresses duplicates", async () => {
    const observer = vi.fn();
    const stop = await tauriBridge.watch(observer);
    const message = {
      subscription_id: subscription,
      delivery_id: 1,
      event: { kind: "disconnected" },
    };
    mocks.channels[0].onmessage(message);
    mocks.channels[0].onmessage(message);
    expect(observer).toHaveBeenCalledTimes(1);
    expect(mocks.invoke).toHaveBeenCalledWith("subscribe_events", {
      request: {
        action: "acknowledge",
        subscription_id: subscription,
        delivery_id: 1,
      },
    });
    await stop();
  });
  it("rejects delivery gaps without acknowledging unprocessed messages", async () => {
    const observer = vi.fn();
    const stop = await tauriBridge.watch(observer);
    mocks.channels[0].onmessage({
      subscription_id: subscription,
      delivery_id: 2,
      event: { kind: "disconnected" },
    });
    expect(observer).toHaveBeenCalledWith({ kind: "disconnected" });
    expect(
      mocks.invoke.mock.calls.filter(
        (call) => call[1]?.request?.action === "acknowledge",
      ),
    ).toHaveLength(0);
    await stop();
  });
  it("ignores callbacks from replaced subscriptions", async () => {
    const observer = vi.fn();
    const stop = await tauriBridge.watch(observer);
    const old = mocks.channels[0];
    old.onmessage({
      subscription_id: subscription,
      delivery_id: 1,
      event: { kind: "resync_required", instance_id: "i", session_id: "s" },
    });
    await Promise.resolve();
    observer.mockClear();
    old.onmessage({
      subscription_id: subscription,
      delivery_id: 2,
      event: { kind: "disconnected" },
    });
    expect(observer).not.toHaveBeenCalled();
    await stop();
  });
});

describe("client diagnostics adapter", () => {
  it("invokes log_client_event with the snake_case payload", async () => {
    await logClientEvent({
      code: "ui.bridge_error",
      level: "warn",
      traceId: "a".repeat(32),
      detail: { component: "bridge", count: 2 },
    });
    expect(mocks.invoke).toHaveBeenCalledWith("log_client_event", {
      event: {
        code: "ui.bridge_error",
        level: "warn",
        trace_id: "a".repeat(32),
        detail: { component: "bridge", count: 2 },
      },
    });
  });
  it("omits absent optional fields and never rejects", async () => {
    mocks.invoke.mockRejectedValue("rate_limited");
    await expect(
      logClientEvent({ code: "ui.render_error", level: "error" }),
    ).resolves.toBeUndefined();
    expect(mocks.invoke).toHaveBeenCalledWith("log_client_event", {
      event: { code: "ui.render_error", level: "error" },
    });
  });
  it("rejects server-only codes at the type boundary", () => {
    // @ts-expect-error server codes are not reportable by the WebView
    void logClientEvent({ code: "server.started", level: "warn" });
  });
});
describe("authentic commands", () => {
  const sha = "a".repeat(64);
  it("sends camelCase arguments, rebuilds the DTO and drops extra fields", async () => {
    mocks.invoke.mockResolvedValueOnce({
      operation_id: "op",
      revision: { sequence: 2, sha256: sha },
      buffer_generation: 4,
      private: "x",
    });
    const ack = await tauriBridge.saveAuthentic({
      date: "2026-11-05",
      operationId: "op",
      expectedRevision: { sequence: 1, sha256: sha },
      body: "hi",
      bufferGeneration: 4,
    });
    expect(ack).toEqual({
      operation_id: "op",
      revision: { sequence: 2, sha256: sha },
      buffer_generation: 4,
    });
    expect(mocks.invoke).toHaveBeenCalledWith("save_authentic", {
      date: "2026-11-05",
      operationId: "op",
      expectedRevision: { sequence: 1, sha256: sha },
      body: "hi",
      bufferGeneration: 4,
    });
  });
  it("turns rejections into allowlisted errors and never forwards raw text", async () => {
    mocks.invoke.mockRejectedValueOnce({
      code: "revision_conflict",
      retryable: false,
    });
    await expect(tauriBridge.loadAuthentic("2026-11-05")).rejects.toMatchObject(
      { code: "revision_conflict", retryable: false },
    );
    mocks.invoke.mockRejectedValueOnce("/Users/me/secret path");
    const error = await tauriBridge.loadAuthentic("2026-11-05").catch((e) => e);
    expect(error.code).toBe("unavailable");
    expect(String(error.message)).not.toContain("secret");
  });
  it("rejects a malformed server response", async () => {
    mocks.invoke.mockResolvedValueOnce({
      date: "2026-11-05",
      exists: true,
      revision: null,
      body: "",
    });
    await expect(tauriBridge.loadAuthentic("2026-11-05")).rejects.toMatchObject(
      { code: "invalid_response" },
    );
  });
  it("accepts the watcher hint event", () => {
    expect(
      streamEvent({
        kind: "authentic_changed",
        instance_id: "i",
        session_id: "s",
      }),
    ).toEqual({ kind: "authentic_changed", instance_id: "i", session_id: "s" });
  });
});

describe("settings commands", () => {
  const user = {
    revision: "missing",
    read_only: false,
    recoverable: false,
    warnings: [],
    writing_locale: "ja-JP",
    generation_count: 3,
    keybindings: {},
    effective_keybindings: { "editor.save": ["Command+S"] },
    keybinding_issues: [],
    path: "/Users/me/secret",
  };
  it("sends camelCase arguments and rebuilds the user settings DTO", async () => {
    mocks.invoke.mockResolvedValueOnce(user);
    const got = await tauriBridge.updateUserSettings({
      section: "writing",
      value: "ja-JP",
      expectedRevision: "missing",
      recover: false,
    });
    expect(mocks.invoke).toHaveBeenCalledWith("update_user_settings", {
      section: "writing",
      value: "ja-JP",
      expectedRevision: "missing",
      recover: false,
    });
    expect(got.writing_locale).toBe("ja-JP");
    expect(JSON.stringify(got)).not.toContain("secret");
  });
  it("round trips generation count and rejects invalid count responses", async () => {
    mocks.invoke.mockResolvedValueOnce({ ...user, generation_count: 5 });
    const got = await tauriBridge.updateUserSettings({
      section: "generation",
      value: { count: 5 },
      expectedRevision: "missing",
      recover: false,
    });
    expect(got.generation_count).toBe(5);
    expect(mocks.invoke).toHaveBeenCalledWith("update_user_settings", {
      section: "generation",
      value: { count: 5 },
      expectedRevision: "missing",
      recover: false,
    });
    for (const generation_count of [undefined, null, 0, 6, 1.5, "3", true]) {
      mocks.invoke.mockResolvedValueOnce({ ...user, generation_count });
      await expect(tauriBridge.getUserSettings()).rejects.toMatchObject({
        code: "invalid_response",
      });
    }
  });
  it("turns rejections into allowlisted codes and never forwards raw text", async () => {
    mocks.invoke.mockRejectedValueOnce({ code: "conflict" });
    await expect(tauriBridge.getUserSettings()).rejects.toMatchObject({
      code: "conflict",
    });
    mocks.invoke.mockRejectedValueOnce("/Users/me/secret path");
    const error = await tauriBridge.reloadSettings().catch((e) => e);
    expect(error.code).toBe("unavailable");
    expect(String(error.message)).not.toContain("secret");
    mocks.invoke.mockResolvedValueOnce({ outcome: "exploded" });
    await expect(tauriBridge.reloadSettings()).rejects.toMatchObject({
      code: "invalid_response",
    });
  });
  it("keeps only allowlisted server status fields", async () => {
    mocks.invoke.mockResolvedValueOnce({
      config_generation: 3,
      last_reload: { outcome: "busy", config_generation: null },
      storage_ready: true,
      sources: [
        { alias: "m", available: false, reason: "source_root_missing" },
      ],
      profiles: [],
      token: "secret",
    });
    const got = await tauriBridge.serverSettings();
    expect(got.last_reload).toEqual({
      outcome: "busy",
      config_generation: null,
      issues: [],
    });
    expect(JSON.stringify(got)).not.toContain("secret");
  });
});

describe("generation commands", () => {
  it("uses fixed commands and typed input/result arguments", async () => {
    const input = {
      request_id: "r",
      date: "2026-11-06",
      profile: "main",
      locale: "ko-KR",
      count: 2,
      selections: [{ import_id: "import", source_id: "source" }],
    };
    mocks.invoke.mockResolvedValueOnce({
      job: { id: "j", state: "running", sequence: 2 },
    });
    await tauriBridge.startGeneration!(input);
    expect(mocks.invoke).toHaveBeenLastCalledWith("start_generation", {
      input,
    });
    mocks.invoke.mockResolvedValueOnce(generationView());
    expect(await tauriBridge.getGeneration!("j")).toEqual(generationView());
    expect(mocks.invoke).toHaveBeenLastCalledWith("get_generation", {
      jobId: "j",
    });
    mocks.invoke.mockResolvedValueOnce(candidateView());
    expect(await tauriBridge.readCandidate!("j-1")).toEqual(candidateView());
    expect(mocks.invoke).toHaveBeenLastCalledWith("read_candidate", {
      candidateId: "j-1",
    });
    mocks.invoke.mockResolvedValueOnce({});
    await tauriBridge.discardGeneration!("j");
    expect(mocks.invoke).toHaveBeenLastCalledWith("discard_generation", {
      jobId: "j",
    });
  });
});

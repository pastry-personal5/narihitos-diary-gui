import { describe, expect, it, vi } from "vitest";
import {
  AuthenticError,
  type Bridge,
  type ChatSnapshot,
  type ChatTurn,
  type ProfileInfo,
} from "../src/application/bridge";
import { ChatController } from "../src/application/chat";

const cap = { available: true, reason: null };
const profile = (alias: string): ProfileInfo => ({
  alias,
  type: "codex",
  status: "ready",
  reason: null,
  capabilities: { chat: cap, generate: cap },
});
/** A tiny in-memory chat server: tests drive the turn to its end by hand. */
function server() {
  const snap: ChatSnapshot = {
    chat_id: "c1",
    profile: "main",
    type: "codex",
    locale: "en-US",
    transcript: [],
    turn: null,
  };
  let pendingMessage = "";
  const calls: string[] = [];
  const bridge = {
    listProfiles: async () => [profile("main"), profile("other")],
    openChat: vi.fn(async (p: string) => {
      calls.push(`open:${p}`);
      snap.profile = p;
      return { chat_id: "c1" };
    }),
    sendChat: vi.fn(async (_c: string, _r: string, message: string) => {
      calls.push("send");
      pendingMessage = message;
      snap.turn = turn("running");
      return { job_id: "j1" };
    }),
    getChat: vi.fn(async () => structuredClone(snap)),
    closeChat: vi.fn(async () => {
      calls.push("close");
    }),
    cancelJob: vi.fn(async () => {
      calls.push("cancel");
      snap.turn = turn("cancelled", "cancelled");
      return {} as never;
    }),
  } as unknown as Bridge;
  const turn = (
    state: ChatTurn["state"],
    error_code: string | null = null,
  ): ChatTurn => ({
    job_id: "j1",
    state,
    error_code,
    stage: "receiving_output",
    revision: 1,
    provisional: state === "running" ? "par" : null,
  });
  return {
    bridge,
    snap,
    calls,
    reply(text: string) {
      snap.transcript.push(
        { role: "user", text: pendingMessage },
        { role: "assistant", text },
      );
      snap.turn = turn("done");
    },
    fail(code: string) {
      snap.turn = turn("failed", code);
    },
  };
}
async function ready() {
  const s = server();
  const c = new ChatController(
    s.bridge,
    () => 1000,
    () => "req-1",
  );
  await c.loadProfiles();
  return { s, c };
}
const doc = {
  kind: "authentic" as const,
  date: "2026-11-06",
  unsaved: true,
  body: "b",
};

describe("ChatController", () => {
  it("opens lazily, shows provisional text, then the validated reply with its chip", async () => {
    const { s, c } = await ready();
    expect(
      await c.send(
        "hello",
        "en-US",
        doc,
        "Entry 2026-11-06, unsaved edits included",
      ),
    ).toBe(true);
    expect(c.running).toBe(true);
    expect(c.state.chat?.turn?.provisional).toBe("par");
    expect(c.state.pending?.text).toBe("hello");
    s.reply("hi there");
    await c.refresh();
    expect(c.running).toBe(false);
    expect(c.state.chat?.transcript.at(-1)?.text).toBe("hi there");
    expect(c.state.chips).toEqual(["Entry 2026-11-06, unsaved edits included"]);
    expect(c.state.announcement).toBe("AI reply ready.");
    expect(s.calls).toEqual(["open:main", "send"]);
  });

  it("restores the text and adds no chip when the turn fails", async () => {
    const { s, c } = await ready();
    await c.send("hello", "en-US", null, "No document");
    s.fail("refusal");
    await c.refresh();
    expect(c.state.error).toBe("refusal");
    expect(c.state.unsent).toBe("hello");
    expect(c.state.chips).toEqual([]);
    expect(c.state.chat?.transcript).toEqual([]);
  });

  it("surfaces busy from the server and keeps the message", async () => {
    const { s, c } = await ready();
    (s.bridge.sendChat as ReturnType<typeof vi.fn>).mockRejectedValueOnce(
      new AuthenticError("busy", false),
    );
    expect(await c.send("hello", "en-US", null, "No document")).toBe(false);
    expect(c.state.error).toBe("busy");
    expect(c.state.unsent).toBe("hello");
    expect(c.running).toBe(false);
  });

  it("refuses to send without a locale, a profile or while running", async () => {
    const { c } = await ready();
    expect(await c.send("hi", null, null, "")).toBe(false);
    expect(await c.send("   ", "en-US", null, "")).toBe(false);
    await c.send("hi", "en-US", null, "");
    expect(await c.send("again", "en-US", null, "")).toBe(false);
  });

  it("cancels through the job command", async () => {
    const { s, c } = await ready();
    await c.send("hello", "en-US", null, "No document");
    await c.cancel();
    expect(s.calls).toContain("cancel");
    expect(c.state.announcement).toBe("AI request cancelled.");
    expect(c.running).toBe(false);
  });

  it("starts a fresh chat when the profile changes", async () => {
    const { s, c } = await ready();
    await c.send("hello", "en-US", null, "No document");
    s.reply("x");
    await c.refresh();
    await c.selectProfile("other");
    expect(s.calls).toContain("close");
    expect(c.state.chat).toBeNull();
    expect(c.state.chips).toEqual([]);
  });

  it("re-reads only on events for its own job", async () => {
    const { s, c } = await ready();
    await c.send("hello", "en-US", null, "No document");
    const get = s.bridge.getChat as ReturnType<typeof vi.fn>;
    get.mockClear();
    const changed = (job_id: string) => ({
      kind: "job_changed" as const,
      instance_id: "i",
      session_id: "s",
      event_id: 1,
      job_id,
      job_sequence: 2,
      payload: { id: job_id, sequence: 2, state: "running" as const },
    });
    c.onEvent(changed("other"));
    await Promise.resolve();
    expect(get).not.toHaveBeenCalled();
    c.onEvent(changed("j1"));
    await c.refresh();
    expect(get).toHaveBeenCalled();
  });

  it("adopts a chat after a reload and drops it when the server ended it", async () => {
    const { s, c } = await ready();
    s.snap.transcript.push(
      { role: "user", text: "old" },
      { role: "assistant", text: "reply" },
    );
    await c.adopt("c1");
    expect(c.state.chat?.transcript).toHaveLength(2);
    (s.bridge.getChat as ReturnType<typeof vi.fn>).mockRejectedValueOnce(
      new AuthenticError("not_found", false),
    );
    await c.refresh();
    expect(c.state.chat).toBeNull();
    expect(c.state.error).toBe("not_found");
  });

  it("is unavailable when the bridge has no chat commands", () => {
    const c = new ChatController({} as Bridge);
    expect(c.available).toBe(false);
  });
});

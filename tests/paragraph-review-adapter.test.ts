import { afterEach, describe, expect, it, vi } from "vitest";
import { ChatParagraphReviewer } from "../src/adapters/paragraph-review/chat-reviewer";
import { paragraphReviewPrompt } from "../src/adapters/paragraph-review/prompt";
import { snippetCreationTime } from "../src/application/paragraph-review/context";
import type { Bridge } from "../src/application/bridge";
const request = {
  text: "Review me",
  context: [],
  profile: { alias: "main", type: "codex" },
  locale: "en-US",
};
function transport() {
  return {
    openChat: vi.fn(async () => ({ chat_id: "opaque" })),
    sendChat: vi.fn(async () => ({ job_id: "job" })),
    getChat: vi.fn(async () => ({
      turn: { state: "done" },
      transcript: [{ role: "assistant", text: "Improved" }],
    })),
    closeChat: vi.fn(async () => {}),
  } as unknown as Pick<
    Bridge,
    "openChat" | "sendChat" | "getChat" | "closeChat"
  >;
}
afterEach(() => vi.useRealTimers());
describe("paragraph AI adapter", () => {
  it("uses a named conversation purpose and closes after completion", async () => {
    const bridge = transport();
    expect(
      await new ChatParagraphReviewer(bridge).review(
        request,
        new AbortController().signal,
      ),
    ).toBe("Improved");
    expect(bridge.openChat).toHaveBeenCalledWith(
      "main",
      "en-US",
      "paragraph_review",
    );
    expect(bridge.sendChat).toHaveBeenCalledWith(
      "opaque",
      expect.any(String),
      expect.stringContaining('"paragraph":"Review me"'),
      null,
    );
    expect(bridge.closeChat).toHaveBeenCalledTimes(1);
  });
  it("cancels a stalled read without waiting for the transport to respond", async () => {
    const bridge = transport();
    vi.mocked(bridge.getChat!).mockImplementation(() => new Promise(() => {}));
    const abort = new AbortController();
    const run = new ChatParagraphReviewer(bridge).review(request, abort.signal);
    const outcome = expect(run).rejects.toThrow("cancelled");
    await vi.waitFor(() => expect(bridge.getChat).toHaveBeenCalled());
    abort.abort();
    await outcome;
    expect(bridge.closeChat).toHaveBeenCalledWith("opaque");
  });
  it("closes an open request that completes after cancellation", async () => {
    const bridge = transport();
    let opened!: (value: { chat_id: string }) => void;
    vi.mocked(bridge.openChat!).mockImplementation(
      () =>
        new Promise((resolve) => {
          opened = resolve;
        }),
    );
    const abort = new AbortController();
    const run = new ChatParagraphReviewer(bridge).review(request, abort.signal);
    const outcome = expect(run).rejects.toThrow("cancelled");
    abort.abort();
    await outcome;
    opened({ chat_id: "late" });
    await vi.waitFor(() =>
      expect(bridge.closeChat).toHaveBeenCalledWith("late"),
    );
    expect(bridge.sendChat).not.toHaveBeenCalled();
  });
  it("bounds a stalled operation with a timeout", async () => {
    vi.useFakeTimers();
    const bridge = transport();
    vi.mocked(bridge.getChat!).mockImplementation(() => new Promise(() => {}));
    const run = new ChatParagraphReviewer(bridge, 100).review(
      request,
      new AbortController().signal,
    );
    const outcome = expect(run).rejects.toThrow("cancelled");
    await vi.advanceTimersByTimeAsync(100);
    await outcome;
    expect(bridge.closeChat).toHaveBeenCalledTimes(1);
  });
  it("renders supported assets once and rejects unsupported prompt types", () => {
    for (const type of ["codex", "claude", "qwen", "apple-foundation-models"]) {
      const prompt = paragraphReviewPrompt(
        type,
        "Keep {{locale}} literally",
        [],
        "ko-KR",
      );
      expect(prompt).toContain("Write in ko-KR");
      expect(prompt).toContain("Keep {{locale}} literally");
      expect(prompt).not.toContain("provider:");
    }
    expect(() => paragraphReviewPrompt("unknown", "text", [], "en-US")).toThrow(
      "prompt_unavailable",
    );
  });
  it("sends header creation evidence and its conflict status in the request data", async () => {
    const bridge = transport();
    const header = {
      unix_ms: Date.parse("2026-11-09T00:00:00Z"),
      precision: "second" as const,
    };
    const context = [
      {
        date: "2026-11-09",
        selected: false,
        text: "Dinner with Jun yesterday. Keep {{locale}} literally.",
        creation_time: snippetCreationTime({
          dating: {
            state: "timestamp_conflict",
            header,
            filename: { ...header, unix_ms: header.unix_ms + 86400000 },
          },
          navigation: null,
        }),
      },
      {
        date: "2026-11-09",
        selected: true,
        text: "No header timestamp.",
        creation_time: null,
      },
    ];
    await new ChatParagraphReviewer(bridge).review(
      { ...request, context },
      new AbortController().signal,
    );
    const message = vi.mocked(bridge.sendChat!).mock.calls[0][2];
    expect(JSON.parse(message.slice(message.lastIndexOf("\n") + 1))).toEqual({
      paragraph: request.text,
      context,
    });
  });
});

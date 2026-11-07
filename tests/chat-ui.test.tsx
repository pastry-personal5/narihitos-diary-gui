// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
} from "@testing-library/react";
import {
  AuthenticError,
  type Bridge,
  type ProfileInfo,
} from "../src/application/bridge";
import { ChatController } from "../src/application/chat";
import { ChatPane } from "../src/components/ChatPane";

afterEach(cleanup);
const cap = { available: true, reason: null };
const profile: ProfileInfo = {
  alias: "main",
  type: "codex",
  status: "ready",
  reason: null,
  capabilities: { chat: cap, generate: cap },
};
function setup() {
  let turn: unknown = null;
  const transcript: unknown[] = [];
  const bridge = {
    listProfiles: async () => [profile],
    openChat: async () => ({ chat_id: "c1" }),
    sendChat: vi.fn(async () => {
      turn = {
        job_id: "j1",
        state: "running",
        error_code: null,
        stage: "waiting_for_output",
        revision: 1,
        provisional: "partial",
      };
      return { job_id: "j1" };
    }),
    getChat: async () => ({
      chat_id: "c1",
      profile: "main",
      type: "codex",
      locale: "en-US",
      transcript,
      turn,
    }),
    closeChat: async () => {},
    cancelJob: vi.fn(async () => ({})),
  } as unknown as Bridge;
  const controller = new ChatController(bridge, Date.now, () => "req");
  return {
    bridge,
    controller,
    finish: (t: string) => {
      transcript.push(
        { role: "user", text: "hello" },
        { role: "assistant", text: t },
      );
      turn = {
        job_id: "j1",
        state: "done",
        error_code: null,
        stage: "finished",
        revision: 2,
        provisional: null,
      };
    },
  };
}
const context = () => ({ document: null, chip: "No document" });
async function mount(locale: string | null = "en-US", connected = true) {
  const s = setup();
  await s.controller.loadProfiles();
  render(
    <ChatPane
      controller={s.controller}
      connected={connected}
      locale={locale}
      context={context}
    />,
  );
  return s;
}

describe("ChatPane", () => {
  it("explains why Send is disabled", async () => {
    await mount(null);
    expect(
      (screen.getByRole("button", { name: "Send" }) as HTMLButtonElement)
        .disabled,
    ).toBe(true);
    expect(
      screen.getByText("Choose a Writing Language in Settings."),
    ).toBeTruthy();
  });

  it("shows disconnected before anything else", async () => {
    await mount("en-US", false);
    expect(screen.getByText("The server is disconnected.")).toBeTruthy();
  });

  it("sends, shows provisional text and a cancel control, then the final reply", async () => {
    const s = await mount();
    const box = screen.getByLabelText("Message");
    fireEvent.change(box, { target: { value: "hello" } });
    expect(screen.getByText(/Sends No document/)).toBeTruthy();
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Send" }));
    });
    expect(screen.getByText("partial")).toBeTruthy();
    expect(screen.getByRole("list").getAttribute("aria-busy")).toBe("true");
    expect(box.closest('[aria-busy="true"]')).toBeNull();
    expect((box as HTMLTextAreaElement).disabled).toBe(false);
    expect(screen.getByText(/Assistant \(provisional\)/)).toBeTruthy();
    expect(screen.getByText(/Waiting for output/)).toBeTruthy();
    expect(
      (screen.getByRole("button", { name: "Send" }) as HTMLButtonElement)
        .disabled,
    ).toBe(true);
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    });
    expect(s.bridge.cancelJob).toHaveBeenCalledWith("j1");
    s.finish("final reply");
    await act(async () => {
      await s.controller.refresh();
    });
    expect(screen.getByText("final reply")).toBeTruthy();
    expect(screen.queryByText(/provisional/)).toBeNull();
    expect(screen.getByText("AI reply ready.")).toBeTruthy();
  });

  it("does not send on Cmd+Enter while an IME composition is active", async () => {
    const s = await mount();
    const box = screen.getByLabelText("Message");
    fireEvent.change(box, { target: { value: "かな" } });
    fireEvent.keyDown(box, { key: "Enter", metaKey: true, isComposing: true });
    expect(s.bridge.sendChat).not.toHaveBeenCalled();
    await act(async () => {
      fireEvent.keyDown(box, { key: "Enter", metaKey: true });
    });
    expect(s.bridge.sendChat).toHaveBeenCalledTimes(1);
  });

  it("puts a failed message back in the composer with a plain error", async () => {
    const s = await mount();
    (s.bridge.sendChat as ReturnType<typeof vi.fn>).mockRejectedValueOnce(
      new AuthenticError("busy", false),
    );
    fireEvent.change(screen.getByLabelText("Message"), {
      target: { value: "hello" },
    });
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Send" }));
    });
    expect(screen.getByRole("alert")).toBeTruthy();
    expect(
      (screen.getByLabelText("Message") as HTMLTextAreaElement).value,
    ).toBe("hello");
  });
});

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
import { AIChatController } from "../src/application/ai-chat/controller";
import { AIChat } from "../src/components/ai-chat/AIChat";

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
  const controller = new AIChatController(bridge, Date.now, () => "req");
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
    <AIChat
      controller={s.controller}
      connected={connected}
      locale={locale}
      context={context}
    />,
  );
  return s;
}

describe("AIChat", () => {
  it("allows Shift+Enter newlines and sends multiline text on Enter without hints", async () => {
    const s = await mount();
    const box = screen.getByLabelText("Message");
    fireEvent.change(box, { target: { value: "first" } });
    expect(fireEvent.keyDown(box, { key: "Enter", shiftKey: true })).toBe(true);
    expect(s.bridge.sendChat).not.toHaveBeenCalled();
    fireEvent.change(box, { target: { value: "first\nsecond" } });
    expect(
      screen.queryByText(/Cmd\+Enter|Shift\+Enter|Enter sends/),
    ).toBeNull();
    await act(async () => {
      expect(fireEvent.keyDown(box, { key: "Enter" })).toBe(false);
    });
    expect(s.controller.state.pending?.text).toBe("first\nsecond");
    expect(s.bridge.sendChat).toHaveBeenCalledTimes(1);
    s.controller.dispose();
  });

  it("shows current context with a decorative bitmap inside the composer", async () => {
    const s = setup();
    await s.controller.loadProfiles();
    let chip = "Entry on 2026-11-03";
    const props = {
      controller: s.controller,
      connected: true,
      locale: "en-US",
      context: () => ({ document: null, chip }),
    };
    const view = render(<AIChat {...props} />);
    const badge = screen.getByLabelText("AI Chat context");
    expect(badge.closest(".ai-chat-composer")).toBeTruthy();
    expect(badge.textContent).toBe(chip);
    expect(
      badge.querySelector('[aria-hidden="true"]')?.getAttribute("style"),
    ).toContain("chat-context.png");
    expect(
      screen.getByLabelText("Message").getAttribute("aria-describedby"),
    ).toContain(badge.id);
    chip = "Source interview, unsaved edits included";
    view.rerender(<AIChat {...props} />);
    expect(badge.textContent).toBe(chip);
    chip = "No document";
    view.rerender(<AIChat {...props} />);
    expect(badge.textContent).toBe(chip);
    s.controller.dispose();
  });

  it("clears context locally with /clear and keeps the composer uncluttered", async () => {
    const s = await mount();
    const box = screen.getByLabelText("Message");
    expect(box.getAttribute("placeholder")).toBe("Chat with AI");
    expect(screen.queryByText("Write a message first.")).toBeNull();
    expect(screen.queryByRole("button", { name: "New chat" })).toBeNull();
    expect(screen.getByRole("button", { name: "Send" })).toHaveProperty(
      "disabled",
      true,
    );
    const reset = vi.spyOn(s.controller, "newChat");
    await act(async () => {
      await s.controller.send("hello", "en-US", null, "No document");
    });
    fireEvent.change(box, { target: { value: "/clear" } });
    expect(screen.getByRole("button", { name: "Send" })).toHaveProperty(
      "disabled",
      true,
    );
    s.finish("reply");
    await act(async () => {
      await s.controller.refresh();
    });
    await act(async () => {
      fireEvent.keyDown(box, { key: "Enter" });
    });
    expect(reset).toHaveBeenCalledOnce();
    expect(s.controller.state.chat).toBeNull();
    expect(s.controller.state.chips).toEqual([]);
    expect(box).toHaveProperty("value", "");
    expect(s.bridge.sendChat).toHaveBeenCalledTimes(1);
    s.controller.dispose();
  });

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
    expect(screen.getByLabelText("AI Chat context").textContent).toBe(
      "No document",
    );
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Send" }));
    });
    expect(screen.getByText("partial")).toBeTruthy();
    expect(screen.getByRole("list").getAttribute("aria-busy")).toBe("true");
    expect(box.closest('[aria-busy="true"]')).toBeNull();
    expect((box as HTMLTextAreaElement).disabled).toBe(false);
    expect(screen.getByText(/AI Agent \(provisional\)/)).toBeTruthy();
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
    expect(screen.getByText("AI Agent", { selector: "strong" })).toBeTruthy();
    expect(screen.queryByText(/Assistant/)).toBeNull();
    expect(screen.queryByText(/provisional/)).toBeNull();
    expect(screen.getByText("AI Agent reply ready.")).toBeTruthy();
  });

  it("does not send on Enter while an IME composition is active", async () => {
    const s = await mount();
    const box = screen.getByLabelText("Message");
    fireEvent.change(box, { target: { value: "かな" } });
    fireEvent.keyDown(box, { key: "Enter", isComposing: true });
    fireEvent.keyDown(box, { key: "Enter", keyCode: 229 });
    expect(s.bridge.sendChat).not.toHaveBeenCalled();
    await act(async () => {
      fireEvent.keyDown(box, { key: "Enter" });
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

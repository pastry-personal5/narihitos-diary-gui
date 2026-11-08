import chatContextIcon from "../../assets/controls/chat-context.png";
import sendIcon from "../../assets/controls/chat-send.png";
import { useState, useSyncExternalStore } from "react";
import {
  CHAT_MESSAGE,
  type AIChatController,
} from "../../application/ai-chat/controller";
import {
  activitySummary,
  useAIChat,
  useTick,
  type ChatContext,
} from "./useAIChat";

export function AIChat({
  controller,
  connected,
  locale,
  context,
  otherRun = null,
  coordinated = false,
}: {
  controller: AIChatController;
  connected: boolean;
  locale: string | null;
  context: () => ChatContext;
  otherRun?: string | null;
  coordinated?: boolean;
}) {
  const s = useAIChat(controller);
  const [draft, setDraft] = useState("");
  const running = controller.running;
  const now = useTick(running);
  // A failed or cancelled turn puts its text back, unless the user already typed something.
  const [restored, setRestored] = useState<string | null>(null);
  if (s.unsent !== restored) {
    setRestored(s.unsent);
    if (s.unsent !== null) setDraft((d) => (d === "" ? (s.unsent ?? "") : d));
  }
  const clearCommand = draft.trim() === "/clear";
  const reason = clearCommand
    ? running
      ? "A reply is in progress."
      : null
    : !controller.available
      ? "AI Chat is unavailable."
      : !connected
        ? "The server is disconnected."
        : otherRun
          ? `An AI run is active for ${otherRun}; wait for it or cancel it.`
          : !s.profile
            ? "No chat-capable profile is configured."
            : !locale
              ? "Choose a Writing Language in Settings."
              : running
                ? "A reply is in progress."
                : null;
  function submit() {
    if (reason) return;
    if (clearCommand) {
      setDraft("");
      void controller.newChat();
      return;
    }
    if (!draft.trim()) return;
    const ctx = context();
    const text = draft;
    setDraft("");
    void controller.send(text, locale, ctx.document, ctx.chip);
  }
  const summary = activitySummary(controller, now);
  return (
    <aside aria-label="AI Chat" className="ai-chat">
      <h2>AI Chat</h2>
      <ol
        aria-busy={running}
        aria-live="off"
        aria-label="Conversation"
        className="ai-chat-log"
      >
        {s.chat?.transcript.map((m, i) => {
          const userIndex = s
            .chat!.transcript.slice(0, i)
            .filter((x) => x.role === "user").length;
          return (
            <li key={i} data-role={m.role}>
              <strong>{m.role === "user" ? "You" : "AI Agent"}</strong>
              {m.role === "user" && s.chips[userIndex] && (
                <small className="ai-chat-chip"> {s.chips[userIndex]}</small>
              )}
              <p>{m.text}</p>
            </li>
          );
        })}
        {s.pending && (
          <li data-role="user" data-pending="true">
            <strong>You</strong>
            <small className="ai-chat-chip"> {s.pending.chip}</small>
            <p>{s.pending.text}</p>
          </li>
        )}
        {running && (
          <li data-role="assistant" data-provisional="true">
            <strong>AI Agent (provisional)</strong>
            <ChatPreview controller={controller} />
          </li>
        )}
      </ol>
      {running && (
        <p aria-live="off">
          {summary}
          {s.cancellationRequested
            ? " · Cancellation requested; waiting for acknowledgement"
            : ""}{" "}
          <button
            disabled={s.cancellationRequested || !connected}
            onClick={() => void controller.cancel()}
          >
            Cancel
          </button>
        </p>
      )}
      <p
        role={coordinated ? undefined : "status"}
        aria-live={coordinated ? "off" : undefined}
        className="visually-hidden"
      >
        {s.announcement}
      </p>
      {s.uncertain && (
        <p>
          Admission is uncertain. Retry uses the original message and request
          ID.{" "}
          <button
            disabled={s.submitting || !connected}
            onClick={() => void controller.retryAdmission()}
          >
            Recover chat submission
          </button>
        </p>
      )}
      {s.error && (
        <p role="alert">
          {CHAT_MESSAGE[s.error] ?? CHAT_MESSAGE.failed}
          {s.error === "session_limit" && (
            <button onClick={() => void controller.refresh()}>
              Retry reading chat
            </button>
          )}
        </p>
      )}
      <div className="ai-chat-composer">
        <div
          id="chat-context"
          aria-label="AI Chat context"
          className="ai-chat-context"
        >
          <span
            aria-hidden="true"
            className="bitmap-icon"
            style={{
              width: 16,
              height: 16,
              maskImage: `url(${chatContextIcon})`,
              WebkitMaskImage: `url(${chatContextIcon})`,
            }}
          />
          <span>{context().chip}</span>
        </div>
        <label htmlFor="chat-message" className="visually-hidden">
          Message
        </label>
        <textarea
          id="chat-message"
          placeholder="Chat with AI"
          value={draft}
          aria-describedby="chat-context chat-reason"
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => {
            // Enter during IME composition confirms text; it never sends.
            if (
              e.key === "Enter" &&
              !e.shiftKey &&
              !e.metaKey &&
              !e.ctrlKey &&
              !e.altKey &&
              !e.nativeEvent.isComposing &&
              e.nativeEvent.keyCode !== 229
            ) {
              e.preventDefault();
              submit();
            }
          }}
        />
        <div className="ai-chat-composer-bar">
          <p id="chat-reason">{reason}</p>
          <button
            className="ai-chat-send"
            aria-label="Send"
            disabled={!!reason || !draft.trim()}
            title={reason ?? "Send"}
            onClick={submit}
          >
            <span aria-hidden="true" className="ai-chat-send-circle">
              <span
                className="bitmap-icon"
                style={{
                  width: 20,
                  height: 20,
                  maskImage: `url(${sendIcon})`,
                  WebkitMaskImage: `url(${sendIcon})`,
                }}
              />
            </span>
          </button>
        </div>
      </div>
    </aside>
  );
}

function ChatPreview({ controller }: { controller: AIChatController }) {
  const text = useSyncExternalStore(
    controller.subscribeOutput,
    controller.preview,
  );
  return <p>{text || "…"}</p>;
}

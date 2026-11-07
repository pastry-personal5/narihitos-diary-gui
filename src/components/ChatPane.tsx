import sendIcon from "../assets/send.png";
import { useState, useSyncExternalStore } from "react";
import { CHAT_MESSAGE, type ChatController } from "../application/chat";
import { activitySummary, useChat, useTick, type ChatContext } from "./useChat";

export function ChatPane({
  controller,
  connected,
  locale,
  context,
  otherRun = null,
  coordinated = false,
}: {
  controller: ChatController;
  connected: boolean;
  locale: string | null;
  context: () => ChatContext;
  otherRun?: string | null;
  coordinated?: boolean;
}) {
  const s = useChat(controller);
  const [draft, setDraft] = useState("");
  const running = controller.running;
  const now = useTick(running);
  // A failed or cancelled turn puts its text back, unless the user already typed something.
  const [restored, setRestored] = useState<string | null>(null);
  if (s.unsent !== restored) {
    setRestored(s.unsent);
    if (s.unsent !== null) setDraft((d) => (d === "" ? (s.unsent ?? "") : d));
  }
  const reason = !controller.available
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
              : draft.trim() === ""
                ? "Write a message first."
                : null;
  function submit() {
    if (reason) return;
    const ctx = context();
    const text = draft;
    setDraft("");
    void controller.send(text, locale, ctx.document, ctx.chip);
  }
  const summary = activitySummary(controller, now);
  return (
    <aside aria-label="AI Chat" className="chat">
      <h2>AI Chat</h2>
      <button
        disabled={running || (!s.chat && !s.pending)}
        onClick={() => void controller.newChat()}
      >
        New chat
      </button>
      <ol
        aria-busy={running}
        aria-live="off"
        aria-label="Conversation"
        className="chat-log"
      >
        {s.chat?.transcript.map((m, i) => {
          const userIndex = s
            .chat!.transcript.slice(0, i)
            .filter((x) => x.role === "user").length;
          return (
            <li key={i} data-role={m.role}>
              <strong>{m.role === "user" ? "You" : "Assistant"}</strong>
              {m.role === "user" && s.chips[userIndex] && (
                <small className="chat-chip"> {s.chips[userIndex]}</small>
              )}
              <p>{m.text}</p>
            </li>
          );
        })}
        {s.pending && (
          <li data-role="user" data-pending="true">
            <strong>You</strong>
            <small className="chat-chip"> {s.pending.chip}</small>
            <p>{s.pending.text}</p>
          </li>
        )}
        {running && (
          <li data-role="assistant" data-provisional="true">
            <strong>Assistant (provisional)</strong>
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
      <div className="chat-composer">
        <label htmlFor="chat-message" className="visually-hidden">
          Message
        </label>
        <textarea
          id="chat-message"
          value={draft}
          placeholder="Message the assistant"
          aria-describedby="chat-reason"
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => {
            // Enter during IME composition confirms text; it never sends.
            if (
              e.key === "Enter" &&
              (e.metaKey || e.ctrlKey) &&
              !e.nativeEvent.isComposing
            ) {
              e.preventDefault();
              submit();
            }
          }}
        />
        <div className="chat-composer-bar">
          <p id="chat-reason">
            {reason ?? `Sends ${context().chip}. Cmd+Enter sends.`}
          </p>
          <button
            className="chat-send"
            aria-label="Send"
            disabled={!!reason}
            title={reason ?? "Send"}
            onClick={submit}
          >
            <span
              aria-hidden="true"
              className="symbol"
              style={{
                width: 24,
                height: 24,
                maskImage: `url(${sendIcon})`,
                WebkitMaskImage: `url(${sendIcon})`,
              }}
            />
          </button>
        </div>
      </div>
    </aside>
  );
}

function ChatPreview({ controller }: { controller: ChatController }) {
  const text = useSyncExternalStore(
    controller.subscribeOutput,
    controller.preview,
  );
  return <p>{text || "…"}</p>;
}

import { useState } from "react";
import { CHAT_MESSAGE, type ChatController } from "../application/chat";
import { activitySummary, useChat, useTick, type ChatContext } from "./useChat";

export function ChatPane({
  controller,
  connected,
  locale,
  context,
  otherRun = null,
}: {
  controller: ChatController;
  connected: boolean;
  locale: string | null;
  context: () => ChatContext;
  otherRun?: string | null;
}) {
  const s = useChat(controller);
  const [draft, setDraft] = useState("");
  const running = controller.running;
  const now = useTick(running);
  const turn = s.chat?.turn ?? null;
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
      <label htmlFor="chat-profile">Profile</label>
      <select
        id="chat-profile"
        value={s.profile ?? ""}
        disabled={running || s.profiles.length === 0}
        onChange={(e) => void controller.selectProfile(e.target.value)}
      >
        {s.profiles.map((p) => (
          <option key={p.alias} value={p.alias}>
            {p.alias}
          </option>
        ))}
      </select>
      <button
        disabled={running || (!s.chat && !s.pending)}
        onClick={() => void controller.newChat()}
      >
        New chat
      </button>
      <ol aria-label="Conversation" className="chat-log">
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
            <p>{turn?.provisional || "…"}</p>
          </li>
        )}
      </ol>
      {running && (
        <p role="status">
          {summary}{" "}
          <button
            disabled={!turn || turn.state !== "running"}
            onClick={() => void controller.cancel()}
          >
            Cancel
          </button>
        </p>
      )}
      <p role="status" className="visually-hidden">
        {s.announcement}
      </p>
      {s.error && (
        <p role="alert">{CHAT_MESSAGE[s.error] ?? CHAT_MESSAGE.failed}</p>
      )}
      <label htmlFor="chat-message">Message</label>
      <textarea
        id="chat-message"
        value={draft}
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
      <p id="chat-reason">
        {reason ?? `Sends ${context().chip}. Cmd+Enter sends.`}
      </p>
      <button disabled={!!reason} title={reason ?? "Send"} onClick={submit}>
        Send
      </button>
    </aside>
  );
}

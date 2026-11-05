import { useLayoutEffect, useRef, useState } from "react";
import type { Messages } from "../application/navigation/messages";
export function GlobalMessages({
  messages,
  height,
}: {
  messages: Messages;
  height: number;
}) {
  const log = useRef<HTMLDivElement>(null);
  const pinned = useRef(true);
  const [away, setAway] = useState(false);
  useLayoutEffect(() => {
    const element = log.current;
    if (element && pinned.current) element.scrollTop = element.scrollHeight;
  }, [messages.items]);
  const jump = () => {
    const element = log.current;
    if (element) element.scrollTop = element.scrollHeight;
    pinned.current = true;
    setAway(false);
  };
  return (
    <section aria-label="Global Messages" style={{ height }}>
      <div className="messages-heading">
        <h2>Global Messages</h2>
        {away && <button onClick={jump}>Jump to latest</button>}
      </div>
      {messages.removed > 0 && (
        <p>{messages.removed} older session messages removed.</p>
      )}
      <div
        ref={log}
        role="log"
        aria-live="polite"
        aria-relevant="additions"
        className="messages-log"
        onScroll={(e) => {
          const element = e.currentTarget;
          const atBottom =
            element.scrollHeight - element.scrollTop - element.clientHeight <=
            4;
          pinned.current = atBottom;
          setAway(!atBottom);
        }}
      >
        {messages.items.map((message) => (
          <p key={message.id} className={`message-${message.severity}`}>
            <time>{message.time}</time>{" "}
            <span aria-hidden="true">
              {message.severity === "error"
                ? "✕"
                : message.severity === "warning"
                  ? "!"
                  : "•"}
            </span>{" "}
            <strong>{message.severity}</strong>: {message.text}
          </p>
        ))}
      </div>
    </section>
  );
}

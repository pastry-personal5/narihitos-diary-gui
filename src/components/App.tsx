import { useEffect, useRef, useState } from "react";
import type { Bridge, Connection } from "../application/bridge";
import { AuthenticPane } from "./AuthenticPane";
import { SAVE_LABEL, today, useAuthentic } from "./useAuthentic";
const tabs = ["Date tab", "Snippet tab", "Draft tab", "Entry tab"] as const;
const unavailable = "This feature is unavailable in the scaffold.";
const datePattern = /^\d{4}-\d{2}-\d{2}$/;
export function App({ bridge }: { bridge: Bridge }) {
  const mounted = useRef(true);
  const dispose = useRef<(() => Promise<void>) | undefined>(undefined);
  const [version, setVersion] = useState<string>();
  const [connection, setConnection] = useState<Connection>({
    kind: "disconnected",
    message: "Server disconnected",
  });
  const [tab, setTab] = useState<(typeof tabs)[number]>("Date tab");
  const connected = connection.kind === "connected";
  const auth = useAuthentic(
    bridge,
    connected,
    connected && connection.status.storage_ready,
  );
  const onEvent = useRef(auth.onEvent);
  useEffect(() => {
    onEvent.current = auth.onEvent;
  }, [auth.onEvent]);
  useEffect(() => {
    let disposed = false;
    mounted.current = true;
    void bridge
      .metadata()
      .then((metadata) => {
        if (!disposed) setVersion(metadata.version);
      })
      .catch(() => {});
    return () => {
      disposed = true;
      mounted.current = false;
      void dispose.current?.();
      dispose.current = undefined;
    };
  }, [bridge]);
  async function connect() {
    setConnection({ kind: "connecting" });
    try {
      await dispose.current?.();
      dispose.current = undefined;
      const status = await bridge.connect();
      if (!mounted.current) return;
      const stop = await bridge.watch((event) => {
        onEvent.current(event);
        if (mounted.current && event.kind === "disconnected")
          setConnection({
            kind: "disconnected",
            message:
              "Connection interrupted. Reconnect to refresh server state.",
          });
        if (event.kind === "resync_required")
          void bridge
            .status()
            .then((fresh) => {
              if (mounted.current)
                setConnection({ kind: "connected", status: fresh });
            })
            .catch(() => {
              if (mounted.current)
                setConnection({
                  kind: "disconnected",
                  message: "Server unavailable. Reconnect to refresh state.",
                });
            });
      });
      if (!mounted.current) {
        await stop();
        return;
      }
      dispose.current = stop;
      if (mounted.current) setConnection({ kind: "connected", status });
    } catch (error) {
      if (mounted.current)
        setConnection({
          kind: "disconnected",
          message:
            error === "client_busy"
              ? "Another Diary client is connected. Close it before reconnecting."
              : "Server unavailable. Check the separate server process and discovery path.",
        });
    }
  }
  const storage = connected && connection.status.storage_ready;
  const queueState = auth.view?.queue.state;
  const canCreate =
    storage &&
    !!auth.view &&
    !auth.view.exists &&
    !auth.view.creating &&
    queueState === "clean";
  const createReason = !connected
    ? "Connect to the server first."
    : !storage
      ? "Storage is unavailable."
      : !auth.view
        ? "Loading this date."
        : auth.view.exists
          ? "A file already exists for this date."
          : auth.view.creating
            ? "Creating the file."
            : queueState !== "clean"
              ? "Wait for pending changes to save."
              : (auth.view.createError ?? "Save an empty file for this date.");
  return (
    <main aria-label="Standard Layout" className="standard-layout">
      <header aria-label="Main Toolbar Strip" className="toolbar">
        <div>
          <h1>Narihito’s Diary</h1>
          <span>{version ? `Version v${version}` : "Version unavailable"}</span>
        </div>
        <div className="toolbar-actions">
          <button
            disabled={!canCreate}
            title={createReason}
            onClick={auth.createEmpty}
          >
            Create Empty
          </button>
          <span role="status" aria-label="Save status">
            {auth.view ? SAVE_LABEL[auth.view.queue.state] : "No file open"}
          </span>
          <button
            disabled
            title="Select eligible snippets after import becomes available."
          >
            Generate
          </button>
        </div>
        <div className="connection">
          <span role="status">
            {connection.kind === "connected"
              ? connection.status.storage_ready
                ? "Connected"
                : "Connected · Storage unavailable"
              : connection.kind === "connecting"
                ? "Connecting…"
                : connection.message}
          </span>
          <button
            onClick={() => {
              void connect();
            }}
            disabled={connection.kind === "connecting"}
          >
            Connect
          </button>
        </div>
      </header>
      <div className="workspace">
        <nav aria-label="Icon Bar" className="icon-bar">
          <span>Icon Bar</span>
          <button disabled title={unavailable}>
            Settings
          </button>
        </nav>
        <aside aria-label="Explorer" className="explorer">
          <h2>Explorer</h2>
          <div aria-label="Explorer views" className="explorer-tabs">
            {tabs.map((name) => (
              <button
                key={name}
                aria-pressed={tab === name}
                onClick={() => setTab(name)}
              >
                {name}
              </button>
            ))}
          </div>
          <h3>{tab}</h3>
          {tab === "Date tab" ? (
            <>
              <label>
                Date
                <input
                  type="date"
                  value={auth.date}
                  onChange={(e) => {
                    if (datePattern.test(e.target.value))
                      void auth.changeDate(e.target.value);
                  }}
                />
              </label>
              <button onClick={() => void auth.changeDate(today())}>
                Today
              </button>
            </>
          ) : tab === "Snippet tab" ? (
            <p>No sources imported. Import is unavailable in the scaffold.</p>
          ) : auth.dates.length === 0 ? (
            <p>No saved dates yet.</p>
          ) : (
            <ul aria-label="Saved dates">
              {auth.dates.map((d) => (
                <li key={d}>
                  <button
                    aria-pressed={d === auth.date}
                    onClick={() => void auth.changeDate(d)}
                  >
                    {d}
                  </button>
                </li>
              ))}
            </ul>
          )}
        </aside>
        <AuthenticPane
          hostRef={auth.hostRef}
          date={auth.date}
          view={auth.view}
          error={auth.error}
          notice={auth.notice}
          dispatch={auth.dispatch}
          readOnlyReason={
            !connected
              ? "Connect to the server to open this date."
              : !storage
                ? "Storage is unavailable: files are read-only."
                : null
          }
        />
        <aside aria-label="AI Chat" className="chat">
          <h2>AI Chat</h2>
          <p>No agent is active.</p>
          <p>Chat and generation are unavailable in the scaffold.</p>
          <label htmlFor="chat-message">Message</label>
          <textarea id="chat-message" disabled aria-describedby="chat-reason" />
          <p id="chat-reason">Agent integration is pending.</p>
          <button disabled>Send</button>
        </aside>
      </div>
      <footer>
        Authentic files save automatically · Imports, AI and exports are
        unavailable.
      </footer>
    </main>
  );
}

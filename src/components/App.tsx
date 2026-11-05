import { useEffect, useRef, useState } from "react";
import type { Bridge, Connection } from "../application/bridge";
import { ScratchEditor } from "./ScratchEditor";
const tabs = ["Date tab", "Snippet tab", "Draft tab", "Entry tab"] as const;
const unavailable = "This feature is unavailable in the scaffold.";
export function App({ bridge }: { bridge: Bridge }) {
  const mounted = useRef(true);
  const dispose = useRef<(() => Promise<void>) | undefined>(undefined);
  const [version, setVersion] = useState<string>();
  const [connection, setConnection] = useState<Connection>({
    kind: "disconnected",
    message: "Server disconnected",
  });
  const [tab, setTab] = useState<(typeof tabs)[number]>("Date tab");
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
  return (
    <main aria-label="Standard Layout" className="standard-layout">
      <header aria-label="Main Toolbar Strip" className="toolbar">
        <div>
          <h1>Narihito’s Diary</h1>
          <span>{version ? `Version v${version}` : "Version unavailable"}</span>
        </div>
        <div className="toolbar-actions">
          <button disabled title={unavailable}>
            Create Empty
          </button>
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
              ? "Connected · Storage unavailable"
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
          <p>
            {tab === "Date tab"
              ? "Date selection becomes available with imports."
              : tab === "Snippet tab"
                ? "No sources imported. Import is unavailable in the scaffold."
                : tab === "Draft tab"
                  ? "Authentic persistence and candidates are unavailable."
                  : "Saved Entry dates become available with persistence."}
          </p>
        </aside>
        <ScratchEditor />
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
        Scaffolding · File storage, imports, AI and exports are unavailable.
      </footer>
    </main>
  );
}

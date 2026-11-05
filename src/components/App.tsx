import {
  useCallback,
  useEffect,
  useMemo,
  useReducer,
  useRef,
  useState,
} from "react";
import type { Bridge, Connection } from "../application/bridge";
import { AuthenticPane } from "./AuthenticPane";
import { SettingsView } from "./SettingsView";
import { SAVE_LABEL } from "./useAuthentic";
import { Explorer } from "./Explorer";
import { GlobalMessages } from "./GlobalMessages";
import { PaneSeparator } from "./PaneSeparator";
import { NavigationModel } from "../application/navigation/model";
import { useSettings } from "./useSettings";
import { CommandPalette } from "./CommandPalette";
import { SymbolLabel, SymbolProvider, type SymbolHost } from "./Symbol";
import {
  ACTIONS,
  DEFAULT_BINDINGS,
  type Handlers,
} from "../application/actions/actions";
import { createDispatcher } from "../application/actions/dispatcher";
export function App({ bridge }: { bridge: Bridge }) {
  const mounted = useRef(true);
  const settingsButton = useRef<HTMLButtonElement>(null);
  const activeGeneration = useRef<number | null>(null);
  const connectionEpoch = useRef(0);
  const dispose = useRef<(() => Promise<void>) | undefined>(undefined);
  const [version, setVersion] = useState<string>();
  const [connection, setConnection] = useState<Connection>({
    kind: "disconnected",
    message: "Server disconnected",
  });
  const connected = connection.kind === "connected";
  const [explorerWidth, setExplorerWidth] = useState(220);
  const [chatWidth, setChatWidth] = useState(240);
  const [messageHeight, setMessageHeight] = useState(160);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const settings = useSettings(bridge, connected);
  const model = useMemo(() => new NavigationModel(bridge), [bridge]);
  const editorHost = useCallback(
    (element: HTMLDivElement | null) => model.resources.attach(element),
    [model],
  );
  const symbols = useMemo<SymbolHost | null>(() => {
    const load = bridge.getSymbol?.bind(bridge);
    if (!load) return null;
    const reported = new Set<string>();
    return {
      load,
      unsupported: (name) => {
        if (reported.has(name)) return;
        reported.add(name);
        model.messages.add(
          `Unsupported symbol ${name}; showing its text label.`,
          "warning",
        );
      },
    };
  }, [bridge, model]);
  const [, update] = useReducer((n: number) => n + 1, 0);
  const lifetime = useRef(0);
  useEffect(() => {
    const counter = lifetime;
    const generation = ++counter.current;
    const off = model.subscribe(update);
    return () => {
      off();
      model.resources.attach(null);
      queueMicrotask(() => {
        if (counter.current === generation) model.dispose();
      });
    };
  }, [model]);
  useEffect(() => {
    let off: (() => void) | undefined;
    let disposed = false;
    void bridge
      .onCloseRequested?.(() => model.resources.closeApp())
      .then((stop) => {
        if (disposed) stop();
        else off = stop;
      });
    return () => {
      disposed = true;
      off?.();
    };
  }, [bridge, model]);
  useEffect(() => {
    let disposed = false;
    const epoch = connectionEpoch;
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
      epoch.current++;
      void dispose.current?.();
      dispose.current = undefined;
    };
  }, [bridge]);
  useEffect(() => {
    if (!connected) return;
    const focus = () => {
      void model.browse(model.calendar?.anchor, "month");
    };
    window.addEventListener("focus", focus);
    return () => window.removeEventListener("focus", focus);
  }, [connected, model]);
  async function connect() {
    const epoch = ++connectionEpoch.current;
    const current = () => mounted.current && connectionEpoch.current === epoch;
    setConnection({ kind: "connecting" });
    try {
      await dispose.current?.();
      dispose.current = undefined;
      const status = await bridge.connect();
      if (!current()) return;
      const stop = await bridge.watch((event) => {
        if (!current()) return;
        model.onEvent(event);
        if (event.kind === "disconnected") {
          connectionEpoch.current++;
          void dispose.current?.();
          dispose.current = undefined;
          void model.connect(false, "", 0, false);
          setConnection({
            kind: "disconnected",
            message:
              "Connection interrupted. Reconnect to refresh server state.",
          });
        }
        if (event.kind === "resync_required")
          void bridge
            .status()
            .then((fresh) => {
              if (current())
                setConnection({ kind: "connected", status: fresh });
            })
            .catch(() => {
              if (current()) {
                connectionEpoch.current++;
                void dispose.current?.();
                dispose.current = undefined;
                void model.connect(false, "", 0, false);
                setConnection({
                  kind: "disconnected",
                  message: "Server unavailable. Reconnect to refresh state.",
                });
              }
            });
      });
      if (!current()) {
        await stop();
        return;
      }
      dispose.current = stop;
      if (current()) {
        const server = await bridge.serverSettings();
        if (!current()) {
          await stop();
          return;
        }
        setConnection({ kind: "connected", status });
        activeGeneration.current = server.config_generation;
        await model.connect(
          true,
          status.instance_id,
          server.config_generation,
          status.storage_ready,
        );
      }
    } catch (error) {
      if (current()) {
        await dispose.current?.();
        dispose.current = undefined;
        void model.connect(false, "", 0, false);
        setConnection({
          kind: "disconnected",
          message:
            error === "client_busy"
              ? "Another Diary client is connected. Close it before reconnecting."
              : "Server unavailable. Check the separate server process and discovery path.",
        });
      }
    }
  }
  const storage = connected && connection.status.storage_ready;
  const selectedView = model.resources.authentic?.session?.view();
  const active = model.resources.active;
  const activeView = active?.session?.view();
  const queueState = selectedView?.queue.state;
  const canCreate =
    storage &&
    !!selectedView &&
    !selectedView.exists &&
    !selectedView.creating &&
    queueState === "clean";
  const createReason = !connected
    ? "Connect to the server first."
    : !storage
      ? "Storage is unavailable."
      : !selectedView
        ? "Loading this date."
        : selectedView.exists
          ? "A file already exists for this date."
          : selectedView.creating
            ? "Creating the file."
            : queueState !== "clean"
              ? "Wait for pending changes to save."
              : (selectedView.createError ??
                "Save an empty file for this date.");
  const bindings = settings.user?.effective_keybindings ?? DEFAULT_BINDINGS;
  const later = (what: string) => `${what} is not available yet.`;
  const handlers: Handlers = {
    "palette.open": { reason: null, run: () => setPaletteOpen(true) },
    "editor.save": {
      reason:
        active?.kind === "source"
          ? "Source saving is not available yet."
          : activeView
            ? null
            : "No file is open.",
      run: () => model.resources.dispatch({ kind: "flush" }),
    },
    "authentic.create_empty": {
      reason: canCreate ? null : createReason,
      run: () => model.resources.createEmpty(),
    },
    "source.refresh": {
      reason:
        active?.kind !== "source"
          ? "Open a source snippet first."
          : active.loading || model.resources.transitioning
            ? "Wait for the source to finish loading."
            : null,
      run: () => model.refreshActiveSource(),
    },
    "settings.open": { reason: null, run: () => setSettingsOpen(true) },
    "source.trash": { reason: later("Move to Trash"), run: () => {} },
    "generate.start": { reason: later("Generation"), run: () => {} },
    "ai.activity.open": { reason: later("AI activity"), run: () => {} },
    "ai.run.cancel": { reason: later("AI cancellation"), run: () => {} },
    "ai.results.open": { reason: later("AI results"), run: () => {} },
    "ai.results.discard": { reason: later("AI results"), run: () => {} },
    "export.start": { reason: later("Export"), run: () => {} },
  };
  const live = useRef({ bindings, handlers });
  useEffect(() => {
    live.current = { bindings, handlers };
  });
  useEffect(() => {
    const onKey = createDispatcher({
      bindings: () => live.current.bindings,
      handlers: () => live.current.handlers,
      suspended: () =>
        document.querySelector('[role="dialog"], [data-capture]') !== null,
      inSnippetTree: () =>
        !!document.activeElement?.closest("[data-snippet-tree]"),
      notice: (text) => model.messages.add(text, "warning"),
    });
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [model]);
  const hint = (id: string, base: string) =>
    bindings[id]?.length ? `${base} (${bindings[id].join(", ")})` : base;
  return (
    <SymbolProvider value={symbols}>
      <main aria-label="Standard Layout" className="standard-layout">
        <header aria-label="Main Toolbar Strip" className="toolbar">
          <div>
            <h1>Narihito’s Diary</h1>
            <span>
              {version ? `Version v${version}` : "Version unavailable"}
            </span>
          </div>
          <div className="toolbar-actions">
            <button
              disabled={!canCreate}
              title={hint("authentic.create_empty", createReason)}
              onClick={() => void handlers["authentic.create_empty"].run()}
            >
              <SymbolLabel name="doc.badge.plus" label="Create Empty" />
            </button>
            <button
              title={hint("palette.open", "Open command palette")}
              onClick={() => void handlers["palette.open"].run()}
            >
              <SymbolLabel name="command" label="Command palette" />
            </button>
            <span role="status" aria-label="Save status">
              {activeView
                ? SAVE_LABEL[activeView.queue.state]
                : active?.kind === "source"
                  ? "Source is read-only"
                  : "No file open"}
            </span>
            <button
              disabled
              title="Select eligible snippets after import becomes available."
            >
              <SymbolLabel name="sparkles" label="Generate" />
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
        {selectedView?.createError && (
          <p role="alert" className="notice">
            Create Empty could not be confirmed ({selectedView.createError}).{" "}
            <button onClick={() => void model.resources.retryCreation()}>
              Retry creation
            </button>
          </p>
        )}
        {model.sourceGuard && (
          <div role="dialog" aria-label="Unsaved source">
            <p>Discard local changes to {model.sourceGuard.resource.label}?</p>
            <button onClick={() => model.sourceGuard?.finish("discard")}>
              Discard changes
            </button>
            <button onClick={() => model.sourceGuard?.finish("cancel")}>
              Cancel
            </button>
          </div>
        )}
        {paletteOpen && (
          <CommandPalette
            entries={ACTIONS.map((a) => ({
              id: a.id,
              label: a.label,
              shortcuts: bindings[a.id] ?? [],
              reason: handlers[a.id].reason,
            }))}
            onClose={() => setPaletteOpen(false)}
            onRun={(id) => {
              setPaletteOpen(false);
              const h = handlers[id];
              if (h.reason) model.messages.add(h.reason, "warning");
              else void h.run();
            }}
          />
        )}
        {settingsOpen && (
          <SettingsView
            model={{
              ...settings,
              reload: async () => {
                await settings.reload();
                if (connection.kind !== "connected") return;
                try {
                  const server = await bridge.serverSettings();
                  if (activeGeneration.current !== server.config_generation) {
                    activeGeneration.current = server.config_generation;
                    await model.connect(
                      false,
                      "",
                      server.config_generation,
                      false,
                    );
                    await model.connect(
                      true,
                      connection.status.instance_id,
                      server.config_generation,
                      server.storage_ready,
                    );
                  }
                } catch {
                  model.messages.add(
                    "Configuration could not be rechecked. Reconnect to refresh source access.",
                    "warning",
                    true,
                  );
                }
              },
            }}
            connected={connected}
            onClose={() => {
              setSettingsOpen(false);
              requestAnimationFrame(() => settingsButton.current?.focus());
            }}
          />
        )}
        <div
          className="workspace"
          hidden={settingsOpen}
          style={{
            gridTemplateColumns: `90px ${explorerWidth}px 6px minmax(220px, 1fr) 6px ${chatWidth}px`,
          }}
        >
          <nav aria-label="Icon Bar" className="icon-bar">
            <span>Icon Bar</span>
            <button
              ref={settingsButton}
              aria-pressed={settingsOpen}
              onClick={() => setSettingsOpen((open) => !open)}
              title={hint("settings.open", "Settings")}
            >
              <SymbolLabel name="gearshape" label="Settings" />
            </button>
          </nav>
          <Explorer model={model} />
          <PaneSeparator
            label="Resize Explorer"
            orientation="vertical"
            value={explorerWidth}
            min={160}
            max={480}
            change={setExplorerWidth}
          />
          <div className="editor-column">
            <div role="tablist" aria-label="Open editor resources">
              {[...model.resources.resources.values()].map((resource) => (
                <span key={resource.key}>
                  <button
                    role="tab"
                    aria-selected={model.resources.activeKey === resource.key}
                    onClick={() => void model.resources.switchTab(resource.key)}
                  >
                    {resource.label}
                  </button>
                  <button
                    aria-label={`Close ${resource.label}`}
                    onClick={() => void model.resources.closeTab(resource.key)}
                  >
                    <SymbolLabel
                      name="xmark"
                      label={`Close ${resource.label}`}
                      fallback="×"
                      size={12}
                    />
                  </button>
                </span>
              ))}
            </div>
            {active?.kind === "source" && (
              <button
                disabled={active.loading || model.resources.transitioning}
                onClick={() => void handlers["source.refresh"].run()}
              >
                <SymbolLabel name="arrow.clockwise" label="Refresh source" />
              </button>
            )}
            {(active?.error || activeView?.loadError) &&
              active?.kind === "authentic" && (
                <button onClick={() => void model.resources.retryActiveLoad()}>
                  Retry loading
                </button>
              )}
            <AuthenticPane
              hostRef={editorHost}
              date={
                active?.kind === "source" ? active.label : (active?.date ?? "")
              }
              view={activeView ?? null}
              error={active?.error ?? activeView?.loadError ?? null}
              notice={
                active?.stale ? "This source requires a fresh import." : null
              }
              dispatch={(action) => model.resources.dispatch(action)}
              readOnlyReason={
                active?.kind === "source"
                  ? "Source Save is unavailable until producer support is verified."
                  : !connected
                    ? "Connect to the server to open this date."
                    : !storage
                      ? "Storage is unavailable: files are read-only."
                      : null
              }
            />
            <PaneSeparator
              label="Resize Global Messages"
              orientation="horizontal"
              value={messageHeight}
              min={80}
              max={320}
              change={setMessageHeight}
              reverse
            />
            <GlobalMessages messages={model.messages} height={messageHeight} />
          </div>
          <PaneSeparator
            label="Resize AI Chat"
            orientation="vertical"
            value={chatWidth}
            min={160}
            max={480}
            change={setChatWidth}
            reverse
          />
          <aside aria-label="AI Chat" className="chat">
            <h2>AI Chat</h2>
            <p>No agent is active.</p>
            <p>Chat and generation are unavailable in the scaffold.</p>
            <label htmlFor="chat-message">Message</label>
            <textarea
              id="chat-message"
              disabled
              aria-describedby="chat-reason"
            />
            <p id="chat-reason">Agent integration is pending.</p>
            <button disabled>Send</button>
          </aside>
        </div>
        <footer>
          Authentic files save automatically · AI and exports are unavailable.
        </footer>
      </main>
    </SymbolProvider>
  );
}

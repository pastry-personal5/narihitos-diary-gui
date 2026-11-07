import { useEffect, useRef, useState } from "react";
import type { ActivityRuntime } from "../application/activity/runtime";
import type { Bridge, Connection } from "../application/bridge";
import { rememberedChat, type ChatController } from "../application/chat";
import type { GenerationController } from "../application/generation/controller";
import type { NavigationModel } from "../application/navigation/model";
import type { ReviewController } from "../application/review/controller";
import { sourceImportAdvice } from "../application/sourceAdvice";

/** Connects in the background, wires server events to the controllers and reconnects with backoff. */
export function useConnection(
  bridge: Bridge,
  closePending: boolean,
  {
    model,
    chat,
    generator,
    activity,
    review,
  }: {
    model: NavigationModel;
    chat: ChatController;
    generator: GenerationController;
    activity: ActivityRuntime;
    review: ReviewController;
  },
) {
  const mounted = useRef(true);
  const activeGeneration = useRef<number | null>(null);
  const connectionEpoch = useRef(0);
  const dispose = useRef<(() => Promise<void>) | undefined>(undefined);
  const [version, setVersion] = useState<string>();
  const [connection, setConnection] = useState<Connection>({
    kind: "disconnected",
    message: "Server disconnected",
  });
  const connected = connection.kind === "connected";
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
  // Connect in the background after launch and after any drop: 0.5s, 1s, 2s … capped at 30s.
  const attempts = useRef(0);
  const latestConnect = useRef(() => {});
  useEffect(() => {
    latestConnect.current = () => void connect();
  });
  useEffect(() => {
    if (connection.kind === "connected") attempts.current = 0;
    if (closePending || connection.kind !== "disconnected") return;
    const delay = Math.min(30_000, 500 * 2 ** attempts.current++);
    const timer = setTimeout(() => latestConnect.current(), delay);
    return () => clearTimeout(timer);
  }, [connection, closePending]);
  useEffect(() => {
    if (!connected) return;
    const focus = () => {
      void model.browse(model.calendar?.anchor, "month");
    };
    window.addEventListener("focus", focus);
    return () => window.removeEventListener("focus", focus);
  }, [connected, model]);
  function adviseSources(sources: Parameters<typeof sourceImportAdvice>[0]) {
    const advice = sourceImportAdvice(sources);
    if (advice) model.messages.add(advice, "warning", true);
  }
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
        chat.onEvent(event);
        generator.onEvent(event);
        activity.event(event);
        if (event.kind === "authentic_changed")
          void review.onAuthenticChanged();
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
        generator.connect(true);
        void generator.loadProfiles();
        void chat.loadProfiles().then(() => {
          const id = rememberedChat();
          if (id && current()) void chat.adopt(id);
        });
        activeGeneration.current = server.config_generation;
        adviseSources(server.sources);
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
  /** After a settings reload: re-advise sources and reconnect the model if the config changed. */
  async function recheckConfiguration() {
    if (connection.kind !== "connected") return;
    try {
      const server = await bridge.serverSettings();
      adviseSources(server.sources);
      if (activeGeneration.current !== server.config_generation) {
        activeGeneration.current = server.config_generation;
        await model.connect(false, "", server.config_generation, false);
        void generator.loadProfiles();
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
  }
  return {
    version,
    connection,
    connected,
    connect,
    latestConnect,
    recheckConfiguration,
  };
}

import type { TrashCoordinator } from "../../application/trash/coordinator";
import type { GlobalMessagesStore } from "../../application/global-messages/store";
import { useEffect, useRef, useState } from "react";
import type { AIActivitiesRuntime } from "../../application/ai-activities/runtime";
import type { Bridge, Connection } from "../../application/bridge";
import {
  rememberedChat,
  type AIChatController,
} from "../../application/ai-chat/controller";
import type { GenerationController } from "../../application/generation/controller";
import type { NavigationModel } from "../../application/navigation/model";
import type { ReviewController } from "../../application/review/controller";
import { sourceImportAdvice } from "../../application/sourceAdvice";

/** Connects in the background, wires server events to the controllers and reconnects with backoff. */
export function useConnection(
  bridge: Bridge,
  closePending: boolean,
  {
    model,
    trash,
    messages,
    chat,
    generator,
    activity,
    review,
  }: {
    model: NavigationModel;
    trash: TrashCoordinator;
    messages: GlobalMessagesStore;
    chat: AIChatController;
    generator: GenerationController;
    activity: AIActivitiesRuntime;
    review: ReviewController;
  },
) {
  async function connectNavigation(
    online: boolean,
    instance: string,
    generation: number,
    writable: boolean,
  ) {
    const recovery = trash.connect(online, writable);
    await model.connect(online, instance, generation, writable);
    await recovery;
  }
  const mounted = useRef(true);
  const attempts = useRef(0);
  const activeGeneration = useRef<number | null>(null);
  const connectionEpoch = useRef(0);
  const pendingConnection = useRef<number | null>(null);
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
    attempts.current = 0;
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
      pendingConnection.current = null;
      void dispose.current?.();
      dispose.current = undefined;
    };
  }, [bridge]);
  // Connect immediately after launch/reload/drop; failed attempts wait at most one second.
  const latestConnect = useRef(() => {});
  useEffect(() => {
    latestConnect.current = () => void connect();
  });
  useEffect(() => {
    if (connection.kind === "connected") attempts.current = 0;
    if (closePending || connection.kind !== "disconnected") return;
    const attempt = attempts.current++;
    const delay =
      attempt === 0 ? 0 : Math.min(1000, 100 * 2 ** Math.min(attempt - 1, 4));
    const retry = () => latestConnect.current();
    const visible = () => {
      if (document.visibilityState === "visible") retry();
    };
    const timer = setTimeout(retry, delay);
    window.addEventListener("focus", retry);
    window.addEventListener("online", retry);
    document.addEventListener("visibilitychange", visible);
    return () => {
      clearTimeout(timer);
      window.removeEventListener("focus", retry);
      window.removeEventListener("online", retry);
      document.removeEventListener("visibilitychange", visible);
    };
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
    if (advice) messages.add(advice, "warning", true);
  }
  async function connect() {
    if (
      !mounted.current ||
      closePending ||
      pendingConnection.current === connectionEpoch.current
    )
      return;
    const epoch = ++connectionEpoch.current;
    pendingConnection.current = epoch;
    const current = () => mounted.current && connectionEpoch.current === epoch;
    let ready = false;
    setConnection({ kind: "connecting" });
    try {
      const previousStop = dispose.current;
      dispose.current = undefined;
      await previousStop?.();
      if (!current()) return;
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
          void connectNavigation(false, "", 0, false);
          setConnection({
            kind: "disconnected",
            message: "Connection interrupted. Reconnecting automatically.",
          });
        }
        if (event.kind === "resync_required" && ready)
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
                void connectNavigation(false, "", 0, false);
                setConnection({
                  kind: "disconnected",
                  message: "Server unavailable. Reconnecting automatically.",
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
        generator.connect(true);
        void generator.loadProfiles();
        void chat.loadProfiles().then(() => {
          const id = rememberedChat();
          if (id && current()) void chat.adopt(id);
        });
        activeGeneration.current = server.config_generation;
        adviseSources(server.sources);
        await connectNavigation(
          true,
          status.instance_id,
          server.config_generation,
          status.storage_ready,
        );
        if (!current()) return;
        ready = true;
        setConnection({ kind: "connected", status });
      }
    } catch (error) {
      if (current()) {
        generator.connect(false);
        const failedStop = dispose.current;
        dispose.current = undefined;
        await failedStop?.();
        if (!current()) return;
        void connectNavigation(false, "", 0, false);
        setConnection({
          kind: "disconnected",
          message:
            error === "client_busy"
              ? "Another Diary client is connected. Close it before reconnecting."
              : "Server unavailable. Retrying automatically; check the separate server process and discovery path.",
        });
      }
    } finally {
      if (pendingConnection.current === epoch) pendingConnection.current = null;
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
        await connectNavigation(false, "", server.config_generation, false);
        void generator.loadProfiles();
        await connectNavigation(
          true,
          connection.status.instance_id,
          server.config_generation,
          server.storage_ready,
        );
      }
    } catch {
      messages.add(
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

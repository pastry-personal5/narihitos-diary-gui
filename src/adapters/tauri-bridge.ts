import { streamEvent } from "../application/events";
import { Channel, invoke } from "@tauri-apps/api/core";
import type { Bridge, ClientEvent } from "../application/bridge";
import { metadataDto, statusDto } from "./validation";
/** Fire-and-forget diagnostics: a rejected or rate-limited event never disturbs the UI. */
export async function logClientEvent(event: ClientEvent): Promise<void> {
  const payload: Record<string, unknown> = {
    code: event.code,
    level: event.level,
  };
  if (event.traceId !== undefined) payload.trace_id = event.traceId;
  if (event.detail !== undefined) payload.detail = event.detail;
  await invoke("log_client_event", { event: payload }).catch(() => {});
}
export const tauriBridge: Bridge = {
  metadata: async () => metadataDto(await invoke<unknown>("get_app_metadata")),
  connect: async () => statusDto(await invoke<unknown>("connect_server")),
  status: async () => statusDto(await invoke<unknown>("get_status")),
  watch: async (observer) => {
    await invoke("open_session");
    let disposed = false;
    let pending = false;
    let recovering = false;
    let gaps = 0;
    let generation = 0;
    let activeChannel: Channel<unknown> | undefined;
    async function start() {
      const own = ++generation;
      const channel = new Channel<unknown>();
      activeChannel = channel;
      let last = 0;
      let subscription: string | undefined;
      channel.onmessage = (value) => {
        if (disposed || own !== generation) return;
        try {
          if (typeof value !== "object" || value === null)
            throw new Error("Invalid delivery");
          const packet = value as Record<string, unknown>;
          if (
            typeof packet.subscription_id !== "string" ||
            packet.subscription_id.length !== 36 ||
            !Number.isSafeInteger(packet.delivery_id) ||
            (packet.delivery_id as number) < 1
          )
            throw new Error("Invalid delivery");
          if (subscription && subscription !== packet.subscription_id)
            throw new Error("Invalid subscription");
          subscription = packet.subscription_id;
          if ((packet.delivery_id as number) <= last) return;
          if (packet.delivery_id !== last + 1) throw new Error("Delivery gap");
          const event = streamEvent(packet.event);
          last = packet.delivery_id as number;
          void invoke("subscribe_events", {
            request: {
              action: "acknowledge",
              subscription_id: subscription,
              delivery_id: last,
            },
          }).catch(() => {
            if (!disposed && own === generation)
              observer({ kind: "disconnected" });
          });
          observer(event);
          if (event.kind === "job_changed") gaps = 0;
          if (event.kind === "resync_required" && !recovering) {
            if (++gaps > 3) {
              observer({ kind: "disconnected" });
              return;
            }
            recovering = true;
            void start()
              .catch(() => {
                if (!disposed) observer({ kind: "disconnected" });
              })
              .finally(() => {
                recovering = false;
              });
          }
        } catch {
          observer({ kind: "disconnected" });
        }
      };
      await invoke("subscribe_events", {
        request: { action: "start", channel },
      });
    }
    try {
      await start();
    } catch {
      disposed = true;
      generation++;
      if (activeChannel) activeChannel.onmessage = () => {};
      await invoke("close_session").catch(() => {});
      throw new Error("Subscription unavailable");
    }
    const heartbeat = setInterval(() => {
      if (disposed || pending) return;
      pending = true;
      void invoke("heartbeat_session")
        .catch(() => {
          if (!disposed) observer({ kind: "disconnected" });
        })
        .finally(() => {
          pending = false;
        });
    }, 5000);
    return async () => {
      if (disposed) return;
      disposed = true;
      generation++;
      clearInterval(heartbeat);
      if (activeChannel) activeChannel.onmessage = () => {};
      await invoke("close_session").catch(() => {});
    };
  },
};

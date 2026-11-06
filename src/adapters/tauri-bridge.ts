import {
  calendarDto,
  importAckDto,
  importPageDto,
  sourceBodyDto,
  sourceSaved,
  importJobDto,
  resolutionDto,
} from "./navigation-validation";
import { streamEvent } from "../application/events";
import { trashAckDto, trashPageDto, trashPreviewDto } from "./trash-validation";
import { Channel, invoke } from "@tauri-apps/api/core";
import type { Bridge, ClientEvent } from "../application/bridge";
import {
  authenticErrorDto,
  authenticFileDto,
  authenticPageDto,
  operationStateDto,
  reloadResultDto,
  profileDto,
  profileListDto,
  chatDto,
  generationDto,
  generationAcceptedDto,
  candidateDto,
  chatOpenedDto,
  turnAcceptedDto,
  serverSettingsDto,
  settingsErrorDto,
  userSettingsDto,
  refreshReportDto,
  saveAckDto,
  metadataDto,
  symbolDto,
  statusDto,
} from "./validation";

async function authentic<T>(
  command: string,
  args: Record<string, unknown> | undefined,
  dto: (value: unknown) => T,
): Promise<T> {
  let value: unknown;
  try {
    value = await invoke<unknown>(command, args);
  } catch (error) {
    throw authenticErrorDto(error);
  }
  try {
    return dto(value);
  } catch {
    throw authenticErrorDto({ code: "invalid_response", retryable: false });
  }
}
async function settings<T>(
  command: string,
  args: Record<string, unknown> | undefined,
  dto: (value: unknown) => T,
): Promise<T> {
  let value: unknown;
  try {
    value = await invoke<unknown>(command, args);
  } catch (error) {
    throw settingsErrorDto(error);
  }
  try {
    return dto(value);
  } catch {
    throw settingsErrorDto("invalid_response");
  }
}
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
  previewSourceDelete: (sourceId, request) =>
    authentic("preview_source_delete", { sourceId, request }, trashPreviewDto),
  confirmSourceDelete: (sourceId, request) =>
    authentic("confirm_source_delete", { sourceId, request }, trashAckDto),
  listSourceDeletions: (cursor, limit) =>
    authentic(
      "list_source_deletions",
      { cursor: cursor ?? null, limit: limit ?? 50 },
      trashPageDto,
    ),
  getCalendar: (anchor, action = "month") =>
    authentic("get_calendar", { anchor: anchor ?? null, action }, calendarDto),
  importDate: (date, requestId) =>
    authentic("import_date", { date, requestId }, importAckDto),
  listSources: (importId, cursor) =>
    authentic(
      "list_sources",
      { importId, cursor: cursor ?? null },
      importPageDto,
    ),
  resolveSourceTime: (importId, sourceId, digest, authority) =>
    authentic(
      "resolve_source_time",
      { importId, sourceId, digest, authority },
      resolutionDto,
    ),
  readSource: (sourceId, snapshotId) =>
    authentic("read_source", { sourceId, snapshotId }, sourceBodyDto),
  saveSource: (sourceId, operationId, baseDigest, body) =>
    authentic(
      "save_source",
      { sourceId, operationId, baseDigest, body },
      sourceSaved,
    ),
  getJob: (jobId) => authentic("get_job", { id: jobId }, importJobDto),
  cancelJob: (jobId) => authentic("cancel_job", { id: jobId }, importJobDto),
  metadata: async () => metadataDto(await invoke<unknown>("get_app_metadata")),
  getSymbol: async (name, size, scale) =>
    symbolDto(await invoke<unknown>("get_symbol", { name, size, scale })),
  serverSettings: () => settings("get_settings", undefined, serverSettingsDto),
  reloadSettings: () => settings("reload_settings", undefined, reloadResultDto),
  listProfiles: () => authentic("list_agents", undefined, profileListDto),
  checkProfile: (alias) => authentic("check_agent", { alias }, profileDto),
  openChat: (profile, locale) =>
    authentic("open_chat", { profile, locale }, chatOpenedDto),
  sendChat: (chatId, requestId, message, document) =>
    authentic(
      "send_chat",
      { chatId, requestId, message, document },
      turnAcceptedDto,
    ),
  getChat: (chatId) => authentic("get_chat", { chatId }, chatDto),
  closeChat: (chatId) => authentic("close_chat", { chatId }, () => undefined),
  startGeneration: (input) =>
    authentic("start_generation", { input }, generationAcceptedDto),
  getGeneration: (jobId) =>
    authentic("get_generation", { jobId }, generationDto),
  readCandidate: (candidateId) =>
    authentic("read_candidate", { candidateId }, candidateDto),
  discardGeneration: (jobId) =>
    authentic("discard_generation", { jobId }, () => undefined),
  getUserSettings: () =>
    settings("get_user_settings", undefined, userSettingsDto),
  updateUserSettings: (u) =>
    settings(
      "update_user_settings",
      {
        section: u.section,
        value: u.value,
        expectedRevision: u.expectedRevision,
        recover: u.recover,
      },
      userSettingsDto,
    ),
  connect: async () => statusDto(await invoke<unknown>("connect_server")),
  status: async () => statusDto(await invoke<unknown>("get_status")),
  listAuthentic: (cursor) =>
    authentic("list_authentic", { cursor: cursor ?? null }, authenticPageDto),
  loadAuthentic: (date) =>
    authentic("load_authentic", { date }, authenticFileDto),
  saveAuthentic: (r) =>
    authentic(
      "save_authentic",
      {
        date: r.date,
        operationId: r.operationId,
        expectedRevision: r.expectedRevision,
        body: r.body,
        bufferGeneration: r.bufferGeneration,
      },
      saveAckDto,
    ),
  createEmptyAuthentic: (date, operationId) =>
    authentic("create_empty_authentic", { date, operationId }, saveAckDto),
  refreshAuthentic: () =>
    authentic("refresh_authentic", undefined, refreshReportDto),
  getOperation: (operationId) =>
    authentic("get_operation", { operationId }, operationStateDto),
  onCloseRequested: async (handler, failed) => {
    const channel = new Channel<unknown>();
    let busy = false;
    // After dispose the Rust side still holds the channel, so a late close must still go through.
    let current = handler;
    channel.onmessage = () => {
      if (busy) return;
      busy = true;
      void current()
        .then((proceed) => (proceed ? invoke("finish_close") : undefined))
        .catch(() => failed?.())
        .finally(() => {
          busy = false;
        });
    };
    await invoke("watch_close", { onClose: channel });
    return () => {
      current = async () => true;
    };
  },
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
          if (event.kind === "job_changed" || event.kind === "candidate_ready")
            gaps = 0;
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

import { PreviewBuffer } from "./activity/preview";
import { SessionBudget, payloadBytes } from "./navigation/budget";
import type { ChatProgress } from "./activity/progress";
import {
  AuthenticError,
  type Bridge,
  type ChatDocumentSnapshot,
  type ChatSnapshot,
  type ProfileInfo,
} from "./bridge";
import type { StreamEvent } from "./events";

/** Plain text for the closed codes a chat action or turn may report. */
export const CHAT_MESSAGE: Record<string, string> = {
  session_limit:
    "There is not enough session space. Close unused tabs or discard results, then retry reading the chat.",
  busy: "Another AI request is running. Wait for it or cancel it.",
  profile_not_ready: "That profile is not ready. Check it in Settings.",
  context:
    "Too long for this model. Shorten the document, start a new chat or choose another profile.",
  unsupported_language: "This profile does not support the Writing Language.",
  refusal: "The model declined this request.",
  rate_limited: "The profile is rate limited. Try again later.",
  timeout: "The profile took too long and was stopped.",
  invalid_output: "The profile returned an unusable reply.",
  unavailable: "AI Chat is unavailable.",
  assets_unavailable: "AI Chat prompt assets are unavailable.",
  not_found: "The chat ended. Start a new chat.",
  session_expired: "The connection expired. Reconnect and try again.",
  failed: "The request failed.",
  invalid_input: "That message cannot be sent.",
};
export const STAGE_TEXT: Record<string, string> = {
  preparing_context: "Preparing context",
  starting_agent: "Starting agent",
  waiting_for_output: "Waiting for output",
  receiving_output: "Receiving reply",
  validating_output: "Validating reply",
  between_candidates: "Between candidates",
  stopping: "Stopping",
  finished: "Finished",
};

export interface ChatState {
  profiles: ProfileInfo[];
  profile: string | null;
  chat: ChatSnapshot | null;
  /** One context chip per user message of the current chat, in order. */
  chips: string[];
  /** The message being answered; shown until the server transcript contains it. */
  pending: { text: string; chip: string } | null;
  submitting: boolean;
  uncertain: boolean;
  cancellationRequested: boolean;
  startedAt: number | null;
  error: string | null;
  /** Text to put back in the composer after a failed or cancelled turn. */
  unsent: string | null;
  /** Latest completion, for a polite live region. */
  announcement: string;
  localNotice: string | null;
}

const KEY = "diary.chat";
function remember(id: string | null) {
  try {
    if (id) sessionStorage.setItem(KEY, id);
    else sessionStorage.removeItem(KEY);
  } catch {
    // Reload recovery is a convenience; without storage a reload starts a new chat.
  }
}
export function rememberedChat(): string | null {
  try {
    return sessionStorage.getItem(KEY);
  } catch {
    return null;
  }
}
const code = (e: unknown) =>
  e instanceof AuthenticError ? e.code : "unavailable";

/** Memory-only chat client state. The server owns the transcript; this mirrors it. */
export class ChatController {
  state: ChatState = {
    profiles: [],
    profile: null,
    chat: null,
    chips: [],
    pending: null,
    submitting: false,
    uncertain: false,
    cancellationRequested: false,
    startedAt: null,
    error: null,
    unsent: null,
    announcement: "",
    localNotice: null,
  };
  private readonly output = new PreviewBuffer();
  subscribeOutput = this.output.subscribe;
  preview = this.output.snapshot;
  private readonly listeners = new Set<() => void>();
  private refreshing: Promise<void> | null = null;
  private again = false;
  private epoch = 0;
  private scope: string | null = null;
  private admission: {
    requestId: string;
    chatId: string | null;
    profile: string;
    locale: string;
    message: string;
    document: ChatDocumentSnapshot | null;
  } | null = null;
  constructor(
    private readonly bridge: Bridge,
    private readonly now: () => number = Date.now,
    private readonly newId: () => string = () => crypto.randomUUID(),
    private readonly budget = new SessionBudget(),
  ) {}

  subscribe = (fn: () => void) => {
    this.listeners.add(fn);
    return () => {
      this.listeners.delete(fn);
    };
  };
  private set(patch: Partial<ChatState>) {
    if (
      Object.entries(patch).every(
        ([key, value]) =>
          JSON.stringify(this.state[key as keyof ChatState]) ===
          JSON.stringify(value),
      )
    )
      return;
    this.state = { ...this.state, ...patch };
    for (const l of this.listeners) l();
  }
  get available() {
    return !!(
      this.bridge.openChat &&
      this.bridge.sendChat &&
      this.bridge.getChat
    );
  }
  get running() {
    return (
      this.state.submitting ||
      this.state.pending !== null ||
      this.state.chat?.turn?.state === "running"
    );
  }

  async loadProfiles() {
    this.applyProfiles(
      (await this.bridge.listProfiles?.().catch(() => [])) ?? [],
    );
  }
  /** Takes the app-wide profile list, so background checks reach the picker. */
  applyProfiles(list: ProfileInfo[]) {
    const usable = list.filter((p) => p.capabilities.chat.available);
    const keep = usable.some((p) => p.alias === this.state.profile);
    this.set({
      profiles: usable,
      profile: keep ? this.state.profile : (usable[0]?.alias ?? null),
    });
  }

  /** Choosing another profile starts a fresh chat; the old one is discarded. */
  async selectProfile(alias: string) {
    if (alias === this.state.profile || this.running) return;
    await this.discard();
    this.set({ profile: alias });
  }
  async newChat() {
    if (this.running) return;
    await this.discard();
  }
  private async discard() {
    const id = this.state.chat?.chat_id;
    this.epoch++;
    this.admission = null;
    this.output.clear();
    this.budget.release("chat-preview");
    this.budget.release("chat");
    this.set({
      chat: null,
      chips: [],
      pending: null,
      error: null,
      unsent: null,
      startedAt: null,
    });
    remember(null);
    if (id) await this.bridge.closeChat?.(id).catch(() => {});
  }

  async send(
    message: string,
    locale: string | null,
    document: ChatDocumentSnapshot | null,
    chip: string,
  ): Promise<boolean> {
    const { profile } = this.state;
    if (!this.available || this.running || !profile || !locale) return false;
    if (message.trim() === "") return false;
    const admission = {
      requestId: this.newId(),
      chatId: this.state.chat?.chat_id ?? null,
      profile,
      locale,
      message,
      document: document ? { ...document } : null,
    };
    if (
      !this.budget.reserveMany([
        ["chat-preview", payloadBytes(this.preview())],
        [
          "chat",
          payloadBytes({
            chat: this.state.chat,
            admission,
            pending: { text: message, chip },
          }),
        ],
      ])
    ) {
      this.set({ error: "session_limit", unsent: message });
      return false;
    }
    this.admission = admission;
    this.set({
      submitting: true,
      uncertain: false,
      cancellationRequested: false,
      localNotice: null,
      error: null,
      unsent: null,
      pending: { text: message, chip },
      startedAt: this.now(),
    });
    return this.submitAdmission();
  }

  async retryAdmission(): Promise<boolean> {
    if (!this.state.uncertain || this.state.submitting || !this.admission)
      return false;
    this.set({ submitting: true, error: null });
    return this.submitAdmission();
  }
  private async submitAdmission(): Promise<boolean> {
    const input = this.admission;
    if (!input) return false;
    const epoch = this.epoch;
    try {
      if (!input.chatId) {
        const opened = await this.bridge.openChat!(input.profile, input.locale);
        if (epoch !== this.epoch) return false;
        input.chatId = opened.chat_id;
        remember(opened.chat_id);
        this.set({
          chat: {
            chat_id: opened.chat_id,
            profile: input.profile,
            locale: input.locale,
            type: "",
            transcript: [],
            turn: null,
          },
          chips: [],
        });
      }
      if (this.state.cancellationRequested && !this.state.uncertain) {
        this.admission = null;
        this.set({
          submitting: false,
          pending: null,
          startedAt: null,
          unsent: input.message,
          cancellationRequested: false,
          localNotice: "AI Chat cancelled before submission.",
        });
        return false;
      }
      const accepted = await this.bridge.sendChat!(
        input.chatId,
        input.requestId,
        input.message,
        input.document,
      );
      if (epoch !== this.epoch) return false;
      this.admission = null;
      this.set({ submitting: false, uncertain: false });
      if (this.state.cancellationRequested) {
        await this.bridge
          .cancelJob(accepted.job_id)
          .catch((e) => this.set({ error: code(e) }));
      }
      await this.refresh();
      return true;
    } catch (e) {
      if (epoch !== this.epoch) return false;
      const c = code(e);
      const uncertain = [
        "unavailable",
        "disconnected",
        "server_unavailable",
        "invalid_response",
        "response_limit",
      ].includes(c);
      if (!uncertain) this.admission = null;
      this.set({
        submitting: false,
        uncertain,
        error: c,
        unsent: uncertain ? null : input.message,
        pending: uncertain ? this.state.pending : null,
        startedAt: uncertain ? this.state.startedAt : null,
      });
      return false;
    }
  }

  async cancel() {
    if (this.admission) {
      this.set({
        cancellationRequested: true,
        localNotice:
          "Waiting for the chat admission result before cancellation.",
      });
      return;
    }
    // Between admission and the first read the turn is not visible yet; read it before cancelling.
    if (this.state.chat?.turn?.state !== "running") await this.refresh();
    const job = this.state.chat?.turn;
    if (!job || job.state !== "running") return;
    this.set({ cancellationRequested: true });
    try {
      await this.bridge.cancelJob(job.job_id);
    } catch (e) {
      // A failed request must not leave Cancel disabled with nothing requested.
      this.set({ cancellationRequested: false, error: code(e) });
    }
    await this.refresh();
  }

  /** Coalesces concurrent refreshes into at most one more pass. */
  refresh(): Promise<void> {
    if (!this.state.chat || !this.bridge.getChat) return Promise.resolve();
    if (this.refreshing) {
      this.again = true;
      return this.refreshing;
    }
    const run = async () => {
      do {
        this.again = false;
        await this.pull();
      } while (this.again);
    };
    this.refreshing = run().finally(() => {
      this.refreshing = null;
    });
    return this.refreshing;
  }
  private async pull() {
    const before = this.state.chat;
    if (!before) return;
    const epoch = this.epoch;
    let next: ChatSnapshot;
    try {
      next = await this.bridge.getChat!(before.chat_id);
    } catch (e) {
      if (epoch !== this.epoch) return;
      const missing = code(e) === "not_found";
      if (missing) {
        remember(null);
        this.admission = null;
        this.output.clear();
        this.budget.release("chat");
        this.budget.release("chat-preview");
      }
      this.set({
        error: code(e),
        chat: missing ? null : this.state.chat,
        pending: missing ? null : this.state.pending,
        submitting: false,
        uncertain: missing ? false : this.state.uncertain,
        startedAt: missing ? null : this.state.startedAt,
      });
      return;
    }

    if (epoch !== this.epoch) return;
    if (this.state.uncertain) return;
    if (
      !this.budget.reserveMany([
        ["chat-preview", payloadBytes(this.preview())],
        [
          "chat",
          payloadBytes({
            chat: next,
            admission: this.admission,
            pending: this.state.pending,
          }),
        ],
      ])
    ) {
      this.set({ error: "session_limit" });
      return;
    }
    const wasRunning = before.turn?.state === "running" || !!this.state.pending;
    const turn = next.turn;
    const settled = turn !== null && turn.state !== "running";
    const users = next.transcript.filter((m) => m.role === "user").length;
    const chips = [...this.state.chips];
    while (chips.length < users) chips.push(this.state.pending?.chip ?? "");
    const failed = settled && turn.state !== "done";
    if (settled) {
      this.output.clear();
      this.budget.release("chat-preview");
    } else this.output.update(turn?.provisional ?? "");
    this.set({
      chat: {
        ...next,
        turn: turn
          ? {
              ...turn,
              provisional: null,
              revision: settled ? turn.revision : 0,
            }
          : null,
      },
      profile: next.profile,
      chips,
      pending: settled ? null : this.state.pending,
      startedAt: settled ? null : this.state.startedAt,
      cancellationRequested: settled ? false : this.state.cancellationRequested,
      unsent: failed
        ? (this.state.pending?.text ?? this.state.unsent)
        : this.state.unsent,
      error:
        failed && turn.state === "failed"
          ? (turn.error_code ?? "failed")
          : this.state.error === "session_limit"
            ? null
            : this.state.error,
      announcement:
        settled && wasRunning
          ? turn.state === "done"
            ? "AI Agent reply ready."
            : turn.state === "cancelled"
              ? "AI request cancelled."
              : "AI request failed."
          : this.state.announcement,
    });
  }

  forgetJob(id: string) {
    if (this.state.chat?.turn?.job_id !== id) return;
    this.dispose();
    remember(null);
    this.set({
      chat: null,
      pending: null,
      chips: [],
      submitting: false,
      uncertain: false,
      cancellationRequested: false,
      startedAt: null,
      unsent: null,
      error: null,
    });
  }
  dispose() {
    this.epoch++;
    this.admission = null;
    this.output.clear();
    this.budget.release("chat");
    this.budget.release("chat-preview");
  }

  /** Events carry job state only; a change while a turn is live re-reads the chat. */
  onEvent(event: StreamEvent) {
    if (event.kind === "job_snapshot" && event.first) {
      const scope = `${event.instance_id}/${event.session_id}`;
      if (this.scope !== null && this.scope !== scope) {
        this.epoch++;
        this.admission = null;
        this.output.clear();
        this.budget.release("chat-preview");
        this.budget.release("chat");
        this.refreshing = null;
        this.again = false;
        this.set({
          chat: null,
          chips: [],
          pending: null,
          submitting: false,
          uncertain: false,
          cancellationRequested: false,
          startedAt: null,
          unsent: null,
          error: null,
          announcement: "",
          localNotice: null,
        });
        remember(null);
      }
      this.scope = scope;
    }
    if (!this.state.chat) return;
    if (
      event.kind === "resync_required" ||
      event.kind === "job_snapshot" ||
      (event.kind === "job_changed" &&
        (this.state.submitting ||
          event.job_id === this.state.chat.turn?.job_id))
    )
      void this.refresh();
  }

  discover(progress: ChatProgress) {
    if (
      this.state.uncertain &&
      this.admission?.requestId === progress.request_id
    ) {
      this.admission = null;
      this.set({ uncertain: false, submitting: false, error: null });
      if (this.state.cancellationRequested)
        void this.bridge
          .cancelJob(progress.job_id)
          .catch((e) => this.set({ error: code(e) }));
      void this.adopt(progress.chat_id);
    } else if (
      !this.state.uncertain &&
      this.state.chat?.chat_id !== progress.chat_id
    ) {
      void this.adopt(progress.chat_id);
    }
  }

  /** Re-reads a chat that survived a WebView reload or reconnect. */
  adopt(chatId: string) {
    if (this.state.chat?.chat_id === chatId) return this.refresh();
    this.set({
      chat: {
        chat_id: chatId,
        profile: this.state.profile ?? "",
        type: "",
        locale: "",
        transcript: [],
        turn: null,
      },
    });
    return this.refresh();
  }
}

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
  startedAt: number | null;
  error: string | null;
  /** Text to put back in the composer after a failed or cancelled turn. */
  unsent: string | null;
  /** Latest completion, for a polite live region. */
  announcement: string;
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
    startedAt: null,
    error: null,
    unsent: null,
    announcement: "",
  };
  private readonly listeners = new Set<() => void>();
  private refreshing: Promise<void> | null = null;
  private again = false;
  private epoch = 0;
  constructor(
    private readonly bridge: Bridge,
    private readonly now: () => number = Date.now,
    private readonly newId: () => string = () => crypto.randomUUID(),
  ) {}

  subscribe = (fn: () => void) => {
    this.listeners.add(fn);
    return () => {
      this.listeners.delete(fn);
    };
  };
  private set(patch: Partial<ChatState>) {
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
    const list = (await this.bridge.listProfiles?.().catch(() => [])) ?? [];
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
    const epoch = this.epoch;
    this.set({
      submitting: true,
      error: null,
      unsent: null,
      pending: { text: message, chip },
      startedAt: this.now(),
    });
    try {
      let chat = this.state.chat;
      if (!chat) {
        const opened = await this.bridge.openChat!(profile, locale);
        if (epoch !== this.epoch) return false;
        chat = {
          chat_id: opened.chat_id,
          profile,
          type: "",
          locale,
          transcript: [],
          turn: null,
        };
        remember(chat.chat_id);
        this.set({ chat, chips: [] });
      }
      await this.bridge.sendChat!(
        chat.chat_id,
        this.newId(),
        message,
        document,
      );
      if (epoch !== this.epoch) return false;
      this.set({ submitting: false });
      await this.refresh();
      return true;
    } catch (e) {
      if (epoch === this.epoch)
        this.set({
          submitting: false,
          error: code(e),
          unsent: message,
          pending: null,
          startedAt: null,
        });
      return false;
    }
  }

  async cancel() {
    const job = this.state.chat?.turn;
    if (!job || job.state !== "running") return;
    try {
      await this.bridge.cancelJob(job.job_id);
    } catch (e) {
      this.set({ error: code(e) });
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
      if (code(e) === "not_found") remember(null);
      if (epoch === this.epoch)
        this.set({
          error: code(e),
          chat: code(e) === "not_found" ? null : this.state.chat,
          pending: null,
          submitting: false,
          startedAt: null,
        });
      return;
    }
    if (epoch !== this.epoch) return;
    const wasRunning = before.turn?.state === "running" || !!this.state.pending;
    const turn = next.turn;
    const settled = turn !== null && turn.state !== "running";
    const users = next.transcript.filter((m) => m.role === "user").length;
    const chips = [...this.state.chips];
    while (chips.length < users) chips.push(this.state.pending?.chip ?? "");
    const failed = settled && turn.state !== "done";
    this.set({
      chat: next,
      profile: next.profile,
      chips,
      pending: settled ? null : this.state.pending,
      startedAt: settled ? null : this.state.startedAt,
      unsent: failed
        ? (this.state.pending?.text ?? this.state.unsent)
        : this.state.unsent,
      error:
        failed && turn.state === "failed"
          ? (turn.error_code ?? "failed")
          : this.state.error,
      announcement:
        settled && wasRunning
          ? turn.state === "done"
            ? "AI reply ready."
            : turn.state === "cancelled"
              ? "AI request cancelled."
              : "AI request failed."
          : this.state.announcement,
    });
  }

  /** Events carry job state only; a change while a turn is live re-reads the chat. */
  onEvent(event: StreamEvent) {
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

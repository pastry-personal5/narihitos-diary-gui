import { AuthenticError, type Bridge, type ProfileInfo } from "../bridge";
import type { StreamEvent } from "../events";
import { SessionBudget, payloadBytes } from "../navigation/budget";
import type {
  CandidateView,
  GenerationInput,
  GenerationView,
} from "./contracts";
export interface SelectedSnippet {
  import_id: string;
  source_id: string;
  digest: string;
}
export interface DirtySnippet {
  id: string;
  label: string;
}
export interface Preparation {
  selected(date: string): SelectedSnippet[];
  dirty(selected: SelectedSnippet[]): DirtySnippet[];
  saveAndRefresh(
    date: string,
    selected: SelectedSnippet[],
    dirty: DirtySnippet[],
    signal: AbortSignal,
  ): Promise<SelectedSnippet[]>;
  flushAuthentic(date: string): Promise<boolean>;
  chatRunning(): boolean;
}
export interface GenerationState {
  jobs: ReadonlyMap<string, GenerationView>;
  profiles: ProfileInfo[];
  profile: string | null;
  checking: boolean;
  submitting: boolean;
  originDate: string | null;
  activeJob: string | null;
  dirty: DirtySnippet[] | null;
  error: string | null;
  uncertain: boolean;
  announcement: string;
  localNotice: string | null;
}
export const GENERATION_MESSAGE: Record<string, string> = {
  empty_selection: "Select at least one eligible snippet.",
  reimport_required:
    "The selected sources changed. Refresh the import and confirm your selection.",
  discard_required:
    "Candidate storage is full. Discard completed results before generating again.",
  session_limit:
    "There is not enough session space. Close unused tabs or discard completed results.",
  profile_not_ready: "Check a generation-capable profile before generating.",
  busy: "An AI run is active. Wait for it or cancel it.",
  context:
    "Too long for this model or the remaining candidate budget. Reduce the selection or discard completed results.",
  invalid_output: "The model returned an unusable candidate.",
  unsupported_language: "This profile does not support the Writing Language.",
  refusal: "The model declined this candidate.",
  rate_limited: "The profile is rate limited. Try again later.",
  timeout: "The profile took too long and was stopped.",
  source_save: "A selected source could not be saved. Your edits are kept.",
  authentic_save:
    "The Entry could not finish saving. Resolve its save status before generating.",
  result_unavailable: "These results are no longer available in this session.",
  cancelled: "Generation cancelled.",
  unavailable: "Generation is unavailable.",
  failed: "Generation failed.",
};
const code = (e: unknown) =>
  e instanceof AuthenticError ? e.code : "unavailable";
const terminal = (v: GenerationView) =>
  ["succeeded", "failed", "cancelled"].includes(v.job.state);
const RESERVATION = 8 * 1024 * 1024;
/** Metadata is observable separately from bounded result/preview buffers. Date/editor mounts
 * never own the run. No output is inserted into an editor or persisted here. */
export class GenerationController {
  state: GenerationState = {
    jobs: new Map(),
    profiles: [],
    profile: null,
    checking: false,
    submitting: false,
    originDate: null,
    activeJob: null,
    dirty: null,
    error: null,
    uncertain: false,
    announcement: "",
    localNotice: null,
  };
  private listeners = new Set<() => void>();
  private outputListeners = new Set<() => void>();
  outputRevision = 0;
  private previews = new Map<string, string>();
  private candidates = new Map<string, CandidateView>();
  private candidateSizes = new Map<string, number>();
  private pendingReads = new Set<string>();
  private discarded = new Set<string>();
  private timer: ReturnType<typeof setTimeout> | undefined;
  private refreshWork: Promise<void> | null = null;
  private again = false;
  private refreshAll = false;
  private urgent = false;
  private online = false;
  private scope: string | null = null;
  private epoch = 0;
  private abort: AbortController | null = null;
  private dirtyChoice:
    ((choice: "save" | "exclude" | "cancel") => void) | null = null;
  private pending: GenerationInput | null = null;
  private admission = false;
  private cancelAfterAdmission = false;
  private requestKey: string | null = null;
  constructor(
    private bridge: Bridge,
    private preparation: Preparation,
    private budget = new SessionBudget(),
    private newId = () => crypto.randomUUID(),
  ) {}
  subscribe = (fn: () => void) => {
    this.listeners.add(fn);
    return () => {
      this.listeners.delete(fn);
    };
  };
  subscribeOutput = (fn: () => void) => {
    this.outputListeners.add(fn);
    return () => {
      this.outputListeners.delete(fn);
    };
  };
  private set(p: Partial<GenerationState>) {
    this.state = { ...this.state, ...p };
    for (const fn of this.listeners) fn();
  }
  private output() {
    this.outputRevision++;
    for (const fn of this.outputListeners) fn();
  }
  get connected() {
    return this.online;
  }
  get available() {
    return !!(
      this.bridge.startGeneration &&
      this.bridge.getGeneration &&
      this.bridge.readCandidate &&
      this.bridge.discardGeneration
    );
  }
  get running() {
    return (
      this.state.submitting ||
      this.state.activeJob !== null ||
      this.state.uncertain
    );
  }
  get active() {
    return this.state.activeJob
      ? this.state.jobs.get(this.state.activeJob)
      : undefined;
  }
  preview(job: string) {
    return this.previews.get(job) ?? "";
  }
  candidate(id: string) {
    return this.candidates.get(id);
  }
  connect(online: boolean) {
    this.online = online;
    if (
      online &&
      (this.state.jobs.size || this.pendingReads.size || this.state.activeJob)
    )
      this.schedule();
    else if (!online && this.timer) {
      clearTimeout(this.timer);
      this.timer = undefined;
    }
  }
  async loadProfiles() {
    const epoch = this.epoch;
    const profiles = (await this.bridge.listProfiles?.().catch(() => [])) ?? [];
    if (epoch !== this.epoch) return;
    this.applyProfiles(profiles);
  }
  /** Takes the app-wide profile list, so background checks make Generate ready. */
  applyProfiles(profiles: ProfileInfo[]) {
    const usable = profiles.filter((p) => p.capabilities.generate.available);
    const profile = usable.some((p) => p.alias === this.state.profile)
      ? this.state.profile
      : (usable.find((p) => p.status === "ready")?.alias ??
        usable[0]?.alias ??
        null);
    this.set({ profiles: usable, profile });
  }
  selectProfile(profile: string) {
    if (!this.running) this.set({ profile });
  }
  async checkProfile() {
    const alias = this.state.profile;
    if (!alias || this.running || !this.bridge.checkProfile) return;
    const epoch = this.epoch;
    this.set({ checking: true, error: null });
    try {
      const next = await this.bridge.checkProfile(alias);
      if (epoch === this.epoch)
        this.set({
          profiles: this.state.profiles.map((p) =>
            p.alias === alias ? next : p,
          ),
        });
    } catch {
      if (epoch === this.epoch) this.set({ error: "profile_not_ready" });
    } finally {
      if (epoch === this.epoch) this.set({ checking: false });
    }
  }
  reason(
    date: string | null,
    locale: string | null,
    ineligible = false,
  ): string | null {
    if (!this.available) return "Generation is unavailable.";
    if (!this.online) return "Connect to the server first.";
    if (this.preparation.chatRunning())
      return "An AI run is active for AI Chat; wait for it or cancel it.";
    if (this.running)
      return this.state.uncertain
        ? "The submission outcome is unknown. Retry the same request."
        : `An AI run is active for ${this.state.originDate ?? this.active?.date ?? "this date"}; wait for it or cancel it.`;
    if (!date) return "Select a date first.";
    if (!locale) return "Choose a Writing Language in Settings.";
    if (
      this.state.checking ||
      !this.state.profiles.some(
        (p) =>
          p.alias === this.state.profile &&
          p.status === "ready" &&
          p.capabilities.generate.available,
      )
    )
      return "No ready generation profile. Check a profile first.";
    if (!this.preparation.selected(date).length)
      return ineligible
        ? "Timestamp conflicts and unknown-date snippets are ineligible. Select an eligible snippet."
        : "Select at least one eligible snippet.";
    return null;
  }
  async start(date: string, locale: string, count: number) {
    if (this.reason(date, locale)) return;
    const profile = this.state.profile!;
    const abort = new AbortController();
    this.abort = abort;
    const epoch = this.epoch;
    const current = () => epoch === this.epoch && !abort.signal.aborted;
    let selected = this.preparation.selected(date).map((s) => ({ ...s }));
    this.set({
      submitting: true,
      originDate: date,
      error: null,
      announcement: `Preparing generation for ${date}.`,
      localNotice: null,
    });
    try {
      const dirty = this.preparation.dirty(selected);
      if (dirty.length) {
        const choice = await new Promise<"save" | "exclude" | "cancel">(
          (resolve) => {
            this.dirtyChoice = resolve;
            this.set({ dirty });
          },
        );
        if (epoch !== this.epoch) return;
        this.dirtyChoice = null;
        this.set({ dirty: null });
        if (choice === "cancel" || !current()) return;
        if (choice === "exclude") {
          const ids = new Set(dirty.map((d) => d.id));
          selected = selected.filter((s) => !ids.has(s.source_id));
        } else
          selected = await this.preparation.saveAndRefresh(
            date,
            selected,
            dirty,
            abort.signal,
          );
      }
      if (!current()) return;
      if (!selected.length) throw new AuthenticError("empty_selection", false);
      if (!(await this.preparation.flushAuthentic(date)))
        throw new AuthenticError("authentic_save", false);
      if (!current()) return;
      if (this.preparation.dirty(selected).length)
        throw new AuthenticError("source_save", false);
      if (this.preparation.chatRunning())
        throw new AuthenticError("busy", false);
      const request_id = this.newId();
      this.requestKey = `generation-request:${request_id}`;
      if (!this.budget.reserve(this.requestKey, RESERVATION))
        throw new AuthenticError("session_limit", false);
      this.pending = {
        request_id,
        date,
        profile,
        locale,
        count,
        selections: selected.map(({ import_id, source_id }) => ({
          import_id,
          source_id,
        })),
      };
      await this.admit();
    } catch (e) {
      if (current()) this.set({ error: code(e) });
    } finally {
      if (epoch === this.epoch) {
        this.abort = null;
        this.set({ submitting: false, dirty: null });
      }
    }
  }
  chooseDirty(choice: "save" | "exclude" | "cancel") {
    this.dirtyChoice?.(choice);
  }
  private async admit() {
    const input = this.pending;
    if (!input || !this.bridge.startGeneration) return;
    const epoch = this.epoch;
    this.admission = true;
    try {
      const { job } = await this.bridge.startGeneration(input);
      if (epoch !== this.epoch) return;
      const key = `generation:${job.id}`;
      if (
        !this.budget.reserveMany([
          [this.requestKey!, 0],
          [key, RESERVATION],
        ])
      )
        throw new AuthenticError("session_limit", false);
      this.requestKey = null;
      this.pending = null;
      this.set({ activeJob: job.id, uncertain: false, error: null });
      if (this.cancelAfterAdmission) {
        this.cancelAfterAdmission = false;
        await this.bridge.cancelJob(job.id);
      }
      await this.refresh();
    } catch (e) {
      if (epoch !== this.epoch) return;
      const c = code(e),
        uncertain =
          this.pending !== null &&
          [
            "unavailable",
            "disconnected",
            "server_unavailable",
            "invalid_response",
            "response_limit",
          ].includes(c);
      if (!uncertain) {
        this.pending = null;
        if (this.requestKey) this.budget.release(this.requestKey);
        this.requestKey = null;
      }
      this.set({ error: c, uncertain });
    } finally {
      if (epoch === this.epoch) {
        this.admission = false;
        if (!this.pending) this.cancelAfterAdmission = false;
      }
    }
  }
  async retry() {
    if (!this.pending || this.state.submitting || !this.online) return;
    const epoch = this.epoch;
    this.set({ submitting: true, error: null });
    try {
      await this.admit();
    } finally {
      if (epoch === this.epoch) this.set({ submitting: false });
    }
  }
  async cancel() {
    const epoch = this.epoch;
    const id = this.state.activeJob;
    if (id) {
      try {
        await this.bridge.cancelJob(id);
        if (epoch !== this.epoch) return;
        await this.refresh();
      } catch (e) {
        if (epoch === this.epoch) this.set({ error: code(e) });
      }
      return;
    }
    if (this.state.submitting || this.state.uncertain) {
      if (this.pending) {
        this.cancelAfterAdmission = true;
        this.set({
          announcement: "Waiting for the admission result before cancellation.",
          localNotice: "Waiting for the admission result before cancellation.",
        });
        return;
      }
      if (this.admission) return;
      this.abort?.abort();
      this.chooseDirty("cancel");
      this.dirtyChoice = null;
      this.abort = null;
      this.epoch++;
      this.set({
        submitting: false,
        dirty: null,
        originDate: null,
        announcement: "Generation cancelled before submission.",
        localNotice: "Generation cancelled before submission.",
      });
    }
  }
  private schedule(all = false) {
    if (all) this.refreshAll = true;
    if (!this.online || this.timer) return;
    this.timer = setTimeout(() => {
      this.timer = undefined;
      void this.refresh();
    }, 100);
  }
  async refresh(all = false): Promise<void> {
    if (all) this.refreshAll = true;
    if (this.refreshWork) {
      this.again = true;
      return this.refreshWork;
    }
    const epoch = this.epoch;
    const work = this.fetch(epoch).finally(() => {
      if (this.refreshWork === work) this.refreshWork = null;
      const urgent = this.urgent;
      this.urgent = false;
      if (
        epoch === this.epoch &&
        (this.state.activeJob || this.pendingReads.size || this.again || urgent)
      ) {
        this.again = false;
        if (urgent) void this.refresh();
        else this.schedule();
      }
    });
    this.refreshWork = work;
    return work;
  }
  private async fetch(epoch: number) {
    const ids = new Set([
      ...this.pendingReads,
      ...(this.refreshAll ? this.state.jobs.keys() : []),
    ]);
    this.refreshAll = false;
    if (this.state.activeJob) ids.add(this.state.activeJob);
    for (const id of ids) {
      const current = () => epoch === this.epoch && !this.discarded.has(id);
      try {
        const view = await this.bridge.getGeneration!(id);
        if (!current()) continue;
        const metadata: GenerationView = {
          ...view,
          provisional: null,
          progress: {
            ...view.progress,
            elapsed_ms: Math.floor(view.progress.elapsed_ms / 1000) * 1000,
          },
        };
        const retainedBytes =
          payloadBytes(metadata) +
          payloadBytes(view.provisional) +
          view.candidates.reduce(
            (n, s) => n + (s.id ? (this.candidateSizes.get(s.id) ?? 0) : 0),
            0,
          );
        const needsRoom =
          !terminal(view) ||
          view.candidates.some((s) => s.id && !this.candidates.has(s.id));
        if (
          !this.budget.reserve(
            `generation:${id}`,
            needsRoom ? Math.max(RESERVATION, retainedBytes) : retainedBytes,
          )
        )
          throw new AuthenticError("session_limit", false);
        this.previews.set(id, view.provisional ?? "");
        const old = this.state.jobs.get(id),
          jobs = new Map(this.state.jobs);
        jobs.set(id, metadata);
        const done = terminal(view),
          firstDone = done && (!old || !terminal(old));
        if (JSON.stringify(old) !== JSON.stringify(metadata) || firstDone) {
          const c = view.progress;
          const announcement = firstDone
            ? `${c.ready_count} candidates ready; ${c.failed_count} failed${c.cancelled_count ? `; ${c.cancelled_count} cancelled` : ""}${c.not_started_count ? `; ${c.not_started_count} not started` : ""}.`
            : this.state.announcement;
          this.set({
            jobs,
            activeJob: done
              ? this.state.activeJob === id
                ? null
                : this.state.activeJob
              : id,
            originDate: done ? this.state.originDate : view.date,
            announcement,
            error: null,
          });
        }
        let readError: string | null = null;
        this.pendingReads.add(id);
        for (const slot of view.candidates) {
          if (slot.id && !this.candidates.has(slot.id)) {
            try {
              const candidate = await this.bridge.readCandidate!(slot.id);
              if (!current()) break;
              const candidateBytes = payloadBytes(candidate);
              const bytes =
                payloadBytes(metadata) +
                payloadBytes(view.provisional) +
                candidateBytes +
                view.candidates.reduce(
                  (n, s) =>
                    n + (s.id ? (this.candidateSizes.get(s.id) ?? 0) : 0),
                  0,
                );
              // Serialized text can exceed the server's raw-text accounting (e.g. escapes).
              if (
                !this.budget.reserve(
                  `generation:${id}`,
                  Math.max(RESERVATION, bytes),
                )
              )
                throw new AuthenticError("session_limit", false);
              this.candidates.set(slot.id, candidate);
              this.candidateSizes.set(slot.id, candidateBytes);
            } catch (e) {
              if (!current()) break;
              readError = code(e);
            }
          }
        }
        if (!current()) continue;
        const missing = view.candidates.some(
          (s) => s.id && !this.candidates.has(s.id),
        );
        if (!missing || readError === "session_limit")
          this.pendingReads.delete(id);
        if (this.state.error !== readError) this.set({ error: readError });
        if (done) {
          const bytes =
            payloadBytes(metadata) +
            view.candidates.reduce(
              (n, s) => n + (s.id ? (this.candidateSizes.get(s.id) ?? 0) : 0),
              0,
            );
          if (!missing) this.budget.reserve(`generation:${id}`, bytes);
        }
        this.output();
      } catch (e) {
        if (current()) this.set({ error: code(e) });
      }
    }
  }
  async discard(id: string) {
    const v = this.state.jobs.get(id);
    if (!v || !terminal(v)) return;
    const epoch = this.epoch;
    try {
      await this.bridge.discardGeneration!(id);
      if (epoch !== this.epoch) return;
      this.forgetResult(id);
    } catch (e) {
      if (epoch === this.epoch) this.set({ error: code(e) });
    }
  }
  /** Authoritative result loss releases only this payload and invalidates pending reads. */
  forgetResult(id: string) {
    const v = this.state.jobs.get(id);
    if (!v) return;
    this.discarded.add(id);
    this.pendingReads.delete(id);
    for (const c of v.candidates)
      if (c.id) {
        this.candidates.delete(c.id);
        this.candidateSizes.delete(c.id);
      }
    this.previews.delete(id);
    this.budget.release(`generation:${id}`);
    const jobs = new Map(this.state.jobs);
    jobs.delete(id);
    this.set({
      jobs,
      error: null,
      activeJob: this.state.activeJob === id ? null : this.state.activeJob,
    });
    this.output();
  }
  /** Adopt a discovered retained result without starting another inference. */
  discover(id: string, requestId?: string) {
    if (
      this.state.uncertain &&
      !this.admission &&
      requestId === this.pending?.request_id
    ) {
      this.pending = null;
      if (this.requestKey) this.budget.release(this.requestKey);
      this.requestKey = null;
      this.set({ uncertain: false, activeJob: id, error: null });
      if (this.cancelAfterAdmission) {
        this.cancelAfterAdmission = false;
        void this.cancel();
      }
    }
    if (
      this.discarded.has(id) ||
      this.state.jobs.has(id) ||
      this.pendingReads.has(id)
    )
      return;
    this.pendingReads.add(id);
    this.schedule();
  }
  onEvent(e: StreamEvent) {
    if (e.kind === "disconnected") {
      this.connect(false);
      return;
    }
    const scope = `${e.instance_id}:${e.session_id}`;
    if (this.scope && this.scope !== scope) this.clear();
    this.scope = scope;
    if (e.kind === "job_changed" || e.kind === "candidate_ready") {
      if (e.job_id === this.state.activeJob || this.state.jobs.has(e.job_id)) {
        if (!["queued", "running"].includes(e.payload.state)) {
          if (this.timer) clearTimeout(this.timer);
          this.timer = undefined;
          this.urgent = this.refreshWork !== null;
          this.pendingReads.add(e.job_id);
          void this.refresh();
        } else this.schedule(e.job_id !== this.state.activeJob);
      }
    } else if (
      e.kind === "resync_required" ||
      e.kind === "authentic_changed" ||
      e.kind === "job_snapshot"
    )
      this.schedule(true);
  }
  clear() {
    this.epoch++;
    this.abort?.abort();
    this.chooseDirty("cancel");
    if (this.timer) clearTimeout(this.timer);
    this.timer = undefined;
    for (const id of this.state.jobs.keys())
      this.budget.release(`generation:${id}`);
    if (this.state.activeJob)
      this.budget.release(`generation:${this.state.activeJob}`);
    if (this.requestKey) this.budget.release(this.requestKey);
    this.pending = null;
    this.requestKey = null;
    this.abort = null;
    this.dirtyChoice = null;
    this.cancelAfterAdmission = false;
    this.admission = false;
    this.refreshAll = false;
    this.urgent = false;
    this.again = false;
    this.refreshWork = null;
    this.previews.clear();
    this.candidates.clear();
    this.candidateSizes.clear();
    this.pendingReads.clear();
    this.discarded.clear();
    this.set({
      jobs: new Map(),
      submitting: false,
      activeJob: null,
      dirty: null,
      uncertain: false,
      error: null,
      originDate: null,
      announcement: "",
      localNotice: null,
    });
    this.output();
  }
  dispose() {
    this.connect(false);
    this.clear();
    this.listeners.clear();
    this.outputListeners.clear();
  }
}

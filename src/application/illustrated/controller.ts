import { AuthenticError, type Bridge, type ProfileInfo } from "../bridge";
import type { AIActivitiesState } from "../ai-activities/store";
import type { GlobalMessagesStore } from "../global-messages/store";
import type { NavigationModel } from "../navigation/model";
import { payloadBytes, type SessionBudget } from "../navigation/budget";
import {
  illustratedRunning,
  type ImageEngines,
  type IllustratedInput,
} from "./contracts";

export interface IllustratedState {
  dates: readonly string[];
  loading: boolean;
  error: string | null;
  engines: ImageEngines | null;
  checking: string | null;
  engineError: boolean;
  submitting: boolean;
  uncertain: boolean;
  deleting: boolean;
  confirmation: { date: string; version: string } | null;
}
/** Catalog/admission only: durable tab payloads belong to ResourceCoordinator and jobs to AIActivitiesRuntime. */
export class IllustratedController {
  state: IllustratedState = {
    dates: [],
    loading: false,
    error: null,
    engines: null,
    checking: null,
    engineError: false,
    submitting: false,
    uncertain: false,
    deleting: false,
    confirmation: null,
  };
  private online = false;
  private writable = false;
  private epoch = 0;
  private disposed = false;
  private listeners = new Set<() => void>();
  private pending: {
    date: string;
    input: IllustratedInput;
    scope: string | null;
    settled: boolean;
  } | null = null;
  private admission: string | null = null;
  private engineRequest = 0;
  get running() {
    return (
      this.state.submitting ||
      this.state.uncertain ||
      !!this.admission ||
      [...this.activities().jobs.values()].some(({ job }) =>
        illustratedRunning(job),
      )
    );
  }
  get canCancel() {
    return (
      this.online &&
      !this.activities().stale &&
      [...this.activities().jobs.values()].some(
        ({ job }) =>
          illustratedRunning(job) && job.progress?.phase !== "stopping",
      )
    );
  }
  async cancel() {
    if (!this.canCancel) return;
    const job = [...this.activities().jobs.values()].find(({ job }) =>
      illustratedRunning(job),
    )?.job;
    if (!job) return;
    try {
      await this.bridge.cancelJob(job.id);
      this.messages.add(
        "Illustrated cancellation requested; waiting for acknowledgement.",
      );
    } catch {
      this.messages.add(
        "Illustrated cancellation could not be confirmed. Check AI Activities after reconnecting.",
        "warning",
      );
    }
  }
  private completed = new Set<string>();
  private sourceNotices = new Map<string, string>();
  private scope: string | null = null;
  private loading: Promise<void> | null = null;
  private enginesLoading: Promise<void> | null = null;
  private engineChecks: Promise<void> | null = null;
  private engineProbe: Promise<void> | null = null;
  private engineFailures = new Set<string>();
  private engineProblems = new Map<string, string>();
  private engineProblem(id: string, problem: string | null) {
    if (!problem) {
      this.engineProblems.delete(id);
    } else if (this.engineProblems.get(id) !== problem) {
      this.engineProblems.set(id, problem);
      this.messages.add(problem, "warning");
    }
  }
  constructor(
    private bridge: Bridge,
    private model: NavigationModel,
    private messages: GlobalMessagesStore,
    private budget: SessionBudget,
    private activities: () => AIActivitiesState,
    private admitted: (id: string) => void,
    private flush: (date: string) => Promise<boolean>,
  ) {}
  subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  };
  private update(value: Partial<IllustratedState>) {
    if (this.disposed) return;
    this.state = { ...this.state, ...value };
    this.listeners.forEach((l) => l());
  }
  connect(online: boolean, writable: boolean) {
    this.online = online;
    this.writable = writable;
    this.epoch++;
    this.loading = null;
    this.enginesLoading = null;
    this.engineChecks = null;
    this.engineProbe = null;
    this.engineProblems.clear();
    this.engineFailures.clear();
    this.update({
      loading: false,
      checking: null,
      engineError: false,
      engines: null,
      confirmation: null,
      submitting: false,
      deleting: false,
      uncertain: !!this.pending,
    });
    if (online) {
      void this.refresh();
      void this.checkEngines();
    }
  }
  async refresh(fresh = false): Promise<void> {
    if (!this.online || !this.bridge.listIllustrated) return;
    if (this.loading) {
      await this.loading;
      if (fresh) return this.refresh();
      return;
    }
    const epoch = this.epoch;
    this.update({ loading: true, error: null });
    const work = (async () => {
      try {
        const dates = await this.bridge.listIllustrated!();
        if (!this.current(epoch)) return;
        if (!this.budget.reserve("illustrated-catalog", payloadBytes(dates)))
          throw new Error("session_limit");
        this.update({ dates });
      } catch {
        if (this.current(epoch))
          this.update({
            error:
              "Illustrated dates could not be loaded. Retry when connected.",
          });
      } finally {
        if (this.current(epoch)) {
          this.loading = null;
          this.update({ loading: false });
        }
      }
    })();
    this.loading = work;
    return work;
  }
  async refreshEngines() {
    if (!this.online || !this.bridge.listImageEngines) return;
    if (this.enginesLoading) return this.enginesLoading;
    const epoch = this.epoch,
      request = ++this.engineRequest;
    const work = (async () => {
      try {
        const engines = await this.bridge.listImageEngines!();
        if (this.current(epoch) && request === this.engineRequest) {
          this.update({ engines, engineError: this.engineFailures.size > 0 });
          this.engineProblem("discovery", null);
        }
      } catch {
        if (this.current(epoch) && request === this.engineRequest) {
          this.update({ engines: null, engineError: true });
          this.engineProblem(
            "discovery",
            "Image engines could not be loaded. Retry in Settings > AI Agents when connected.",
          );
        }
      } finally {
        if (this.current(epoch) && request === this.engineRequest)
          this.enginesLoading = null;
      }
    })();
    this.enginesLoading = work;
    return work;
  }
  async checkEngines(): Promise<void> {
    if (!this.online || this.disposed) return;
    if (this.engineChecks) return this.engineChecks;
    const epoch = this.epoch;
    const work = (async () => {
      try {
        await this.refreshEngines();
        if (!this.current(epoch)) return;
        const engines = this.state.engines?.image_engines ?? [];
        for (const engine of engines) {
          if (!this.current(epoch)) return;
          await this.checkEngine(engine.id);
        }
      } finally {
        if (this.current(epoch)) this.engineChecks = null;
      }
    })();
    this.engineChecks = work;
    return work;
  }
  async checkEngine(id: string): Promise<void> {
    if (!this.online || this.disposed || !this.bridge.checkImageEngine) return;
    const epoch = this.epoch;
    if (this.engineProbe) {
      const same = this.state.checking === id;
      await this.engineProbe;
      if (!same && this.current(epoch)) return this.checkEngine(id);
      return;
    }
    this.update({ checking: id });
    const work = (async () => {
      try {
        const engine = await this.bridge.checkImageEngine!(id);
        if (!this.current(epoch)) return;
        this.engineFailures.delete(id);
        this.engineProblem(
          `engine:${id}`,
          engine.status === "ready"
            ? null
            : engine.status === "unavailable"
              ? `Image engine ${id}: prerequisites are unavailable. Check the local engine setup and retry in Settings > AI Agents.`
              : `Image engine ${id}: readiness could not be confirmed (${engine.status}). Retry in Settings > AI Agents.`,
        );
        this.enginesLoading = null;
        await this.refreshEngines();
      } catch (error) {
        if (!this.current(epoch)) return;
        this.engineFailures.add(id);
        this.update({ engineError: true });
        this.engineProblem(
          `engine:${id}`,
          error instanceof AuthenticError && error.code === "busy"
            ? `Image engine ${id}: the engine is busy. Retry its readiness check after the active image operation finishes.`
            : `Image engine ${id}: readiness check failed. Retry in Settings > AI Agents when connected.`,
        );
      } finally {
        if (this.current(epoch)) {
          this.engineProbe = null;
          this.update({ checking: null });
        }
      }
    })();
    this.engineProbe = work;
    return work;
  }
  reason(
    date: string | null,
    profile: ProfileInfo | undefined,
    locale: string | null,
    otherBusy = false,
  ) {
    if (!this.online) return "Connect to the server first.";
    if (!this.writable) return "Storage is unavailable or read-only.";
    if (!this.bridge.createIllustrated)
      return "Illustrated creation is unavailable.";
    if (!date) return "Choose a date.";
    if (this.state.submitting || this.admission)
      return "Preparing Illustrated diary or checking its admission.";
    if (this.state.uncertain)
      return "Creation admission is uncertain. Retry the same request to check its outcome.";
    if (
      otherBusy ||
      this.activities().stale ||
      [...this.activities().jobs.values()].some(({ job }) =>
        ["queued", "running"].includes(job.state),
      )
    )
      return "Wait for the active AI request or reconnect to refresh activity.";
    const source = this.model.resources.resources.get(`authentic:${date}`);
    if (
      !(source?.session
        ? source.session.view().exists
        : this.model.catalog.items.some((i) => i.date === date && !i.problem))
    )
      return "Save an Authentic diary for this date first.";
    if (source?.loading || source?.error || source?.session?.view().loadError)
      return "Finish loading the Authentic diary first.";
    if (
      source?.session &&
      ["save_failed", "external_conflict"].includes(
        source.session.view().queue.state,
      )
    )
      return "Resolve the Authentic save problem before creating Illustrated diary.";
    if (source?.buffer.composing || source?.buffer.reviewing)
      return "Finish composition or review before creating Illustrated diary.";
    if (!locale) return "Choose a Writing Language in Settings.";
    if (!profile) return "Choose a Generation profile in Settings.";
    if (
      profile.status !== "ready" ||
      !profile.capabilities.illustrated?.available
    )
      return "Check the Generation profile and its Illustrated capability in Settings.";
    if (this.state.engineError)
      return "Refresh image engine status in Settings.";
    const engines = this.state.engines;
    if (!engines?.illustrated_available)
      return "Illustrated creation is unavailable. Check server configuration and restart if needed.";
    const engine = engines.image_engines.find(
      (e) => e.id === engines.illustrated_engine,
    );
    if (this.state.checking)
      return "Wait for image engine readiness checks to finish.";
    if (engine?.status !== "ready")
      return "Check the selected image engine in Settings before creating Illustrated diary.";
    return null;
  }
  async create(date: string, profile: ProfileInfo, locale: string) {
    if (this.reason(date, profile, locale)) return;
    this.pending = {
      date,
      input: {
        request_id: crypto.randomUUID(),
        profile: profile.alias,
        locale,
        engine: null,
      },
      scope: this.activities().scope,
      settled: false,
    };
    await this.submit(true);
  }
  async retryAdmission() {
    if (
      !this.pending ||
      this.state.submitting ||
      !this.online ||
      this.pending.scope !== this.activities().scope
    )
      return;
    await this.submit(false);
  }
  private async submit(settle: boolean) {
    const request = this.pending;
    if (!request || !this.bridge.createIllustrated) return;
    const epoch = this.epoch;
    this.update({ submitting: true, uncertain: false });
    try {
      if ((settle || !request.settled) && !(await this.flush(request.date)))
        throw new AuthenticError("authentic_save", false);
      if (
        !this.current(epoch) ||
        this.pending !== request ||
        request.scope !== this.activities().scope
      )
        return;
      // Only a submitted request may skip settling on an admission retry.
      // A disconnected preparation can finish while newer edits keep arriving.
      request.settled = true;
      const ack = await this.bridge.createIllustrated(
        request.date,
        request.input,
      );
      if (!this.current(epoch) || this.pending !== request) return;
      this.pending = null;
      const known = this.activities().jobs.get(ack.job_id)?.job;
      this.admission = known ? null : ack.job_id;
      this.admitted(ack.job_id);
      if (!known || ["queued", "running"].includes(known.state))
        this.messages.add(
          `${request.date}: Illustrated creation started. Follow progress in AI Activities.`,
        );
    } catch (error) {
      if (!this.current(epoch)) return;
      const code = error instanceof AuthenticError ? error.code : "unavailable";
      const uncertain = [
        "unavailable",
        "disconnected",
        "invalid_response",
        "uncertain",
      ].includes(code);
      if (!uncertain) this.pending = null;
      this.update({ uncertain });
      this.messages.add(
        uncertain
          ? `${request.date}: Illustrated creation could not be confirmed. Retry the same request to check admission.`
          : code === "authentic_save"
            ? "Finish saving the Authentic diary before creating Illustrated diary."
            : `${request.date}: Illustrated creation could not start. Check the selected profile, engine and source.`,
        "warning",
      );
    } finally {
      if (this.current(epoch)) this.update({ submitting: false });
    }
  }
  observeActivity() {
    const activity = this.activities();
    if (activity.stale || !this.online) return;
    if (this.scope !== activity.scope) {
      this.scope = activity.scope;
      this.admission = null;
      this.completed.clear();
      if (this.pending && this.pending.scope !== activity.scope) {
        this.pending = null;
        this.update({ submitting: false, uncertain: false });
        this.messages.add(
          "The Illustrated creation session ended. Refresh its catalog to check published results.",
          "warning",
        );
      }
    }
    if (this.admission && activity.jobs.has(this.admission)) {
      this.admission = null;
      this.update({});
    }
    for (const { job } of activity.jobs.values()) {
      if (job.progress?.kind !== "illustrated" || illustratedRunning(job))
        continue;
      const key = `${activity.scope}/${job.id}`;
      if (this.completed.has(key)) continue;
      this.completed.add(key);
      if (this.completed.size > 512)
        this.completed.delete(this.completed.values().next().value!);
      if (job.state === "succeeded")
        void this.published(job.progress.origin_date, this.epoch);
      else
        this.messages.add(
          `${job.progress.origin_date}: Illustrated creation ${job.state === "cancelled" ? "cancelled" : "failed"}. Any previous published version is preserved.`,
          job.state === "cancelled" ? "info" : "error",
        );
    }
    this.update({});
  }
  async resync() {
    if (!this.online) return;
    const epoch = this.epoch;
    await this.refresh(true);
    if (!this.current(epoch)) return;
    for (const r of this.model.resources.resources.values()) {
      if (r.kind === "illustrated")
        await this.model.resources.openIllustrated(r.date!, false, true);
      if (!this.current(epoch)) return;
    }
    await this.checkSources();
  }
  private async published(date: string, epoch: number) {
    await this.refresh(true);
    if (!this.current(epoch)) return;
    const open = this.model.resources.resources.has(`illustrated:${date}`);
    const selected = this.model.resources.selectedDate === date;
    if (open || selected)
      await this.model.resources.openIllustrated(date, selected, true);
    if (!this.current(epoch)) return;
    this.messages.add(
      `${date}: Illustrated diary created.${selected ? "" : " Open it from the Illustrated tab in Explorer."}`,
    );
    void this.checkSources();
  }
  async open(date: string) {
    const epoch = this.epoch;
    if (
      this.model.resources.selectedDate !== date &&
      !(await this.model.chooseDate(date))
    )
      return false;
    if (!this.current(epoch)) return false;
    const opened = await this.model.resources.openIllustrated(date);
    if (this.current(epoch) && opened) void this.checkSources();
    return opened;
  }
  observeSources() {
    for (const r of this.model.resources.resources.values()) {
      const entry = r.illustrated;
      if (!entry) continue;
      const source = this.model.resources.resources
        .get(`authentic:${entry.date}`)
        ?.session?.view();
      const revision = source?.queue.acknowledged;
      // An older retained source is not evidence that the current saved source changed.
      if (
        !source?.exists ||
        !revision ||
        revision.sequence <= entry.source_revision
      )
        continue;
      this.sourceChanged(
        entry.date,
        entry.version,
        revision.sequence,
        revision.sha256,
      );
    }
  }
  private sourceChanged(
    date: string,
    version: string,
    sequence: number,
    sha256: string,
  ) {
    const key = `${version}/${sequence}/${sha256}`;
    if (this.sourceNotices.get(date) === key) return;
    this.sourceNotices.delete(date);
    this.sourceNotices.set(date, key);
    if (this.sourceNotices.size > 512)
      this.sourceNotices.delete(this.sourceNotices.keys().next().value!);
    this.messages.add(
      `${date}: The Authentic diary has changed since this Illustrated diary was created.`,
    );
  }
  async checkSources() {
    const epoch = this.epoch;
    if (!this.online) return;
    for (const r of this.model.resources.resources.values()) {
      const entry = r.illustrated;
      if (!entry || r.stale) continue;
      try {
        const source = await this.bridge.loadAuthentic(entry.date);
        if (
          !this.current(epoch) ||
          this.model.resources.resources.get(r.key) !== r ||
          r.illustrated?.version !== entry.version
        )
          continue;
        if (!source.exists || !source.revision) continue;
        if (
          source.revision.sequence === entry.source_revision &&
          source.revision.sha256 === entry.source_sha256
        )
          continue;
        this.sourceChanged(
          entry.date,
          entry.version,
          source.revision.sequence,
          source.revision.sha256,
        );
      } catch {
        /* Source unavailability is not evidence of a changed revision. */
      }
    }
  }
  deleteReason() {
    if (!this.online || !this.writable)
      return "Connect to writable storage first.";
    const r = this.model.resources.active,
      entry = r?.illustrated;
    if (!entry || r?.stale) return "Open a current Illustrated diary first.";
    if (this.state.deleting) return "Deletion is in progress.";
    if (this.activities().stale)
      return "Reconnect to refresh activity before deletion.";
    if (
      this.pending?.date === entry.date ||
      this.admission ||
      [...this.activities().jobs.values()].some(
        ({ job }) =>
          illustratedRunning(job) && job.progress?.origin_date === entry.date,
      )
    )
      return "Wait for this day's Illustrated creation to finish.";
    return null;
  }
  requestDelete() {
    if (this.deleteReason()) return;
    const entry = this.model.resources.active!.illustrated!;
    this.update({ confirmation: { date: entry.date, version: entry.version } });
  }
  dismissDelete() {
    if (!this.state.deleting) this.update({ confirmation: null });
  }
  async confirmDelete() {
    const request = this.state.confirmation,
      epoch = this.epoch;
    if (
      !request ||
      !this.online ||
      !this.writable ||
      this.state.deleting ||
      !this.bridge.deleteIllustrated
    )
      return;
    this.update({ deleting: true });
    try {
      await this.bridge.deleteIllustrated(request.date, request.version);
      if (!this.current(epoch)) return;
      const r = this.model.resources.resources.get(
        `illustrated:${request.date}`,
      );
      if (r?.illustrated?.version === request.version)
        await this.model.resources.closeTab(r.key);
      await this.refresh(true);
      this.messages.add(`${request.date}: Illustrated diary deleted.`);
    } catch (error) {
      if (!this.current(epoch)) return;
      this.messages.add(
        "Illustrated deletion was not confirmed. Refresh and confirm the displayed version again.",
        "warning",
      );
      if (error instanceof AuthenticError && error.code === "stale_version")
        await this.model.resources.openIllustrated(request.date, false, true);
      await this.refresh();
    } finally {
      if (this.current(epoch))
        this.update({ deleting: false, confirmation: null });
    }
  }
  onSourceChanged() {
    void this.checkSources();
  }
  private current(epoch: number) {
    return !this.disposed && this.online && epoch === this.epoch;
  }
  dispose() {
    this.disposed = true;
    this.epoch++;
    this.listeners.clear();
    this.pending = null;
    this.budget.release("illustrated-catalog");
  }
}

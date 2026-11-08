import { SessionBudget, payloadBytes } from "../navigation/budget";
import type { SnapshotJob, StreamEvent } from "../events";
export interface AIActivityRecord {
  job: SnapshotJob;
  unread: boolean;
}
export interface AIActivitiesState {
  scope: string | null;
  stale: boolean;
  jobs: ReadonlyMap<string, AIActivityRecord>;
  announcement: string;
  notice: number;
  capacity: boolean;
}
const terminal = (job: SnapshotJob) =>
  !["queued", "running"].includes(job.state);
/** Metadata only. Controllers retain sole ownership of transcript/candidate payloads. */
export class AIActivitiesStore {
  state: AIActivitiesState = {
    scope: null,
    stale: true,
    jobs: new Map(),
    announcement: "",
    notice: 0,
    capacity: false,
  };
  private listeners = new Set<() => void>();
  private staged: Map<string, SnapshotJob> | null = null;
  private initialized = false;
  private terminals = new Map<string, number>();
  constructor(private budget = new SessionBudget()) {}
  private reserve(
    jobs: ReadonlyMap<string, AIActivityRecord>,
    terminals: Map<string, number>,
    staged: Map<string, SnapshotJob> | null,
  ) {
    if (
      this.budget.reserveMany([
        [
          "activity:live",
          jobs.size || terminals.size
            ? payloadBytes({ jobs: [...jobs], terminals: [...terminals] })
            : 0,
        ],
        ["activity:stage", staged?.size ? payloadBytes([...staged]) : 0],
      ])
    )
      return true;
    this.staged = null;
    this.budget.release("activity:stage");
    this.publish({ capacity: true, stale: true });
    return false;
  }
  subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  };
  private publish(update: Partial<AIActivitiesState>) {
    this.state = { ...this.state, ...update };
    this.listeners.forEach((listener) => listener());
  }
  event(event: StreamEvent) {
    if (event.kind === "disconnected") {
      this.staged = null;
      this.budget.release("activity:stage");
      this.publish({ stale: true });
      return;
    }
    const scope = `${event.instance_id}/${event.session_id}`;
    if (event.kind === "job_snapshot") {
      if (event.first) {
        if (this.state.scope !== scope) {
          const interrupted = this.state.scope !== null;
          this.initialized = false;
          this.terminals.clear();
          this.budget.release("activity:live");
          this.budget.release("activity:stage");
          this.publish({
            scope,
            notice: this.state.notice + (interrupted ? 1 : 0),
            jobs: new Map(),
            stale: true,
            announcement: interrupted
              ? "The AI session ended. Transient results are unavailable."
              : "",
          });
        }
        this.staged = new Map();
        this.budget.release("activity:stage");
        this.publish({ stale: true });
      }
      if (this.state.scope !== scope || !this.staged) return;
      const staged = new Map(this.staged);
      for (const job of event.jobs) {
        if (job.progress) staged.set(job.id, job);
        else staged.delete(job.id);
      }
      if (!event.complete) {
        if (this.reserve(this.state.jobs, this.terminals, staged))
          this.staged = staged;
        return;
      }
      const terminals = new Map(this.terminals);
      const jobs = new Map<string, AIActivityRecord>();
      let announcement = this.state.announcement;
      let notice = this.state.notice;
      for (const [id, job] of staged) {
        if (!job.progress) continue;
        const previous = this.state.jobs.get(id);
        const latest =
          previous && previous.job.sequence > job.sequence ? previous.job : job;
        const completed = terminal(latest);
        const newCompletion =
          completed && latest.sequence > (this.terminals.get(id) ?? 0);
        if (completed) terminals.set(id, latest.sequence);
        if (this.initialized && newCompletion) {
          announcement = completion(latest);
          notice++;
        }
        jobs.set(id, {
          job: latest,
          unread: newCompletion || (previous?.unread ?? false),
        });
      }
      if (!this.reserve(jobs, terminals, null)) return;
      this.terminals = terminals;
      this.staged = null;
      this.initialized = true;
      this.publish({
        jobs,
        stale: false,
        capacity: false,
        announcement,
        notice,
      });
      return;
    }
    if (scope !== this.state.scope) return;
    if (event.kind === "resync_required") this.publish({ stale: true });
    // Lifecycle hints intentionally require an authoritative progress read by the owner.
  }
  observe(job: SnapshotJob) {
    const p = job.progress;
    if (!p) {
      this.remove(job.id);
      return true;
    }
    if (`${p.instance_id}/${p.session_id}` !== this.state.scope) return false;
    const previous = this.state.jobs.get(job.id);
    // An older observation is a benign race, not a read failure.
    if (previous && previous.job.sequence > job.sequence) return true;
    const completed =
      terminal(job) && job.sequence > (this.terminals.get(job.id) ?? 0);
    const terminals = new Map(this.terminals);
    if (terminal(job)) terminals.set(job.id, job.sequence);
    const jobs = new Map(this.state.jobs);
    jobs.set(job.id, { job, unread: completed || (previous?.unread ?? false) });
    if (!this.reserve(jobs, terminals, this.staged)) return false;
    this.terminals = terminals;
    this.publish({
      jobs,
      notice: this.state.notice + (completed ? 1 : 0),
      announcement: completed ? completion(job) : this.state.announcement,
    });
    return true;
  }
  markStale() {
    this.publish({ stale: true });
  }
  /** Clears staleness caused by failed single-job reads; never during a baseline or at capacity. */
  markFresh() {
    if (this.state.stale && !this.state.capacity && this.staged === null)
      this.publish({ stale: false });
  }
  remove(id: string) {
    const jobs = new Map(this.state.jobs);
    jobs.delete(id);
    if (!this.reserve(jobs, this.terminals, this.staged)) return;
    this.publish({ jobs });
  }
  dismiss(id: string) {
    const record = this.state.jobs.get(id);
    if (!record) return;
    const jobs = new Map(this.state.jobs);
    jobs.set(id, { ...record, unread: false });
    if (!this.reserve(jobs, this.terminals, this.staged)) return;
    this.publish({ jobs });
  }
  dispose() {
    this.initialized = false;
    this.staged = null;
    this.terminals.clear();
    this.budget.release("activity:live");
    this.budget.release("activity:stage");
    this.publish({
      jobs: new Map(),
      scope: null,
      stale: true,
      capacity: false,
      announcement: "",
    });
  }
}

function completion(job: SnapshotJob) {
  const p = job.progress!;
  return `${p.kind === "chat" ? "AI Chat" : "Generation"}${p.origin_date ? ` for ${p.origin_date}` : ""} ${job.state}. View results in activity details.`;
}

import { SessionBudget } from "../navigation/budget";
import type { Bridge } from "../bridge";
import type { SnapshotJob, StreamEvent } from "../events";
import { AIActivitiesStore } from "./store";
/** Uses the native paginated baseline/replay owner; never starts inference. */
export class AIActivitiesRuntime {
  readonly store: AIActivitiesStore;
  private epoch = 0;
  private connected = false;
  private timer: ReturnType<typeof setTimeout> | undefined;
  private pending = new Set<string>();
  private discarded = new Set<string>();
  private inFlight = new Set<string>();
  private retryAt = new Map<string, number>();
  private nextReadAt = new Map<string, number>();
  private terminalHints = new Map<string, number>();
  private failed = new Set<string>();
  constructor(
    private bridge: Bridge,
    private discovered: (job: SnapshotJob) => void = () => {},
    private unavailable: (
      id: string,
      remaining: readonly SnapshotJob[],
    ) => void = () => {},
    budget = new SessionBudget(),
  ) {
    this.store = new AIActivitiesStore(budget);
  }
  event(event: StreamEvent) {
    const before = this.store.state.scope;
    const retained = this.store.state.jobs;
    this.store.event(event);
    if (
      before !== this.store.state.scope ||
      event.kind === "disconnected" ||
      event.kind === "resync_required" ||
      (event.kind === "job_snapshot" && event.first)
    ) {
      this.epoch++;
      if (before !== this.store.state.scope) this.discarded.clear();
      this.pending.clear();
      this.inFlight.clear();
      this.retryAt.clear();
      this.nextReadAt.clear();
      this.terminalHints.clear();
      this.failed.clear();
    }
    if (
      event.kind === "disconnected" ||
      event.kind === "resync_required" ||
      (event.kind === "job_snapshot" && event.first)
    ) {
      this.connected = false;
      if (this.timer) clearTimeout(this.timer);
      if (event.kind !== "job_snapshot" || !event.complete) return;
    }
    if (
      event.kind === "job_snapshot" &&
      event.complete &&
      !this.store.state.stale
    ) {
      for (const id of this.discarded) this.store.remove(id);
      for (const id of retained.keys())
        if (!this.store.state.jobs.has(id))
          this.unavailable(
            id,
            [...this.store.state.jobs.values()].map((r) => r.job),
          );
      this.connected = true;
      for (const { job } of this.store.state.jobs.values())
        this.discovered(job);
      this.tick();
    } else if (
      (event.kind === "job_changed" || event.kind === "candidate_ready") &&
      this.connected &&
      `${event.instance_id}/${event.session_id}` === this.store.state.scope
    ) {
      const terminal = !["queued", "running"].includes(event.payload.state);
      if (
        terminal &&
        event.job_sequence > (this.terminalHints.get(event.job_id) ?? 0)
      ) {
        this.terminalHints.set(event.job_id, event.job_sequence);
        this.nextReadAt.delete(event.job_id);
        this.retryAt.delete(event.job_id);
      }
      void this.read(event.job_id);
    }
  }
  private tick = () => {
    if (this.timer) clearTimeout(this.timer);
    if (!this.connected) return;
    for (const { job } of this.store.state.jobs.values()) {
      if (["queued", "running"].includes(job.state)) void this.read(job.id);
    }
    this.timer = setTimeout(this.tick, 1000);
  };
  private async read(id: string) {
    if (this.discarded.has(id)) return;
    if (this.inFlight.has(id)) {
      this.pending.add(id);
      return;
    }
    if (
      Date.now() <
      Math.max(this.retryAt.get(id) ?? 0, this.nextReadAt.get(id) ?? 0)
    )
      return;
    this.nextReadAt.set(id, Date.now() + 1000);
    const epoch = this.epoch;
    this.inFlight.add(id);
    try {
      const job = await this.bridge.getJob(id);
      if (epoch !== this.epoch || this.discarded.has(id)) return;
      this.retryAt.delete(id);
      this.failed.delete(id);
      const known = this.store.state.jobs.has(id);
      if (!this.store.observe(job)) {
        this.retryAt.set(id, Date.now() + 30_000);
        return;
      }
      if (known && !job.progress)
        this.unavailable(
          id,
          [...this.store.state.jobs.values()].map((r) => r.job),
        );
      if (!this.failed.size && this.connected) this.store.markFresh();
      if (job.progress) this.discovered(job);
    } catch {
      if (epoch !== this.epoch || this.discarded.has(id)) return;
      this.retryAt.set(id, Date.now() + 30_000);
      this.failed.add(id);
      this.store.markStale();
    } finally {
      if (epoch === this.epoch) {
        this.inFlight.delete(id);
        if (this.pending.delete(id)) void this.read(id);
      }
    }
  }
  refresh(id: string) {
    if (this.connected) void this.read(id);
  }
  forget(id: string) {
    this.discarded.add(id);
    this.pending.delete(id);
    this.store.remove(id);
  }
  dispose() {
    this.store.dispose();
    this.epoch++;
    this.connected = false;
    if (this.timer) clearTimeout(this.timer);
    this.pending.clear();
    this.inFlight.clear();
  }
}

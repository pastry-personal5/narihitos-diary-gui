import { expect, it } from "vitest";
import { AIActivitiesStore } from "../src/application/ai-activities/store";
import { generationView } from "./generation-fixtures";
import type { SnapshotJob } from "../src/application/events";
const view = generationView();
const active: SnapshotJob = { ...view.job, progress: view.progress };
const done: SnapshotJob = { ...active, sequence: 4, state: "succeeded" };
function snapshot(
  store: AIActivitiesStore,
  jobs: SnapshotJob[],
  first = true,
  complete = true,
) {
  store.event({
    kind: "job_snapshot",
    instance_id: "i",
    session_id: "s",
    jobs,
    first,
    complete,
  });
}
it("restores historical results silently and dismisses attention without deleting metadata", () => {
  const store = new AIActivitiesStore();
  snapshot(store, [done]);
  expect(store.state.jobs.get("j")?.unread).toBe(true);
  expect(store.state.announcement).toBe("");
  store.dismiss("j");
  snapshot(store, [done]);
  expect(store.state.jobs.get("j")?.unread).toBe(false);
  expect(store.state.jobs.size).toBe(1);
});
it("keeps partial baseline staged and announces a new completion after disconnect once", () => {
  const store = new AIActivitiesStore();
  snapshot(store, [active]);
  store.event({ kind: "disconnected" });
  snapshot(store, [done], true, false);
  expect(store.state.jobs.get("j")?.job.state).toBe("running");
  expect(store.state.stale).toBe(true);
  snapshot(store, [], false, true);
  expect(store.state.jobs.get("j")?.job.state).toBe("succeeded");
  expect(store.state.announcement).toContain("succeeded");
  const announcement = store.state.announcement;
  snapshot(store, [active]);
  expect(store.state.jobs.get("j")?.job.state).toBe("succeeded");
  expect(store.state.announcement).toBe(announcement);
});

it("counts each completion once even when consecutive outcomes have identical text", () => {
  const store = new AIActivitiesStore();
  snapshot(store, [active]);
  store.observe(done);
  expect(store.state.notice).toBe(1);
  store.observe(done);
  expect(store.state.notice).toBe(1);
  snapshot(store, [done]);
  expect(store.state.notice).toBe(1);
});

it("retains scoped terminal deduplication after authoritative result removal", () => {
  const store = new AIActivitiesStore();
  snapshot(store, [active]);
  store.observe(done);
  expect(store.state.notice).toBe(1);
  store.observe({ ...done, progress: undefined });
  expect(store.state.jobs.size).toBe(0);
  store.observe(done);
  expect(store.state.notice).toBe(1);
  expect(store.state.jobs.get("j")?.unread).toBe(false);
});

it("accounts staged metadata before retention and preserves the previous baseline on exhaustion", async () => {
  const { SessionBudget } =
    await import("../src/application/navigation/budget");
  const budget = new SessionBudget(4096);
  const store = new AIActivitiesStore(budget);
  snapshot(store, [active]);
  const retained = budget.used;
  expect(retained).toBeGreaterThan(0);
  expect(budget.reserve("other", 4096 - retained)).toBe(true);
  const added = {
    ...active,
    id: "other",
    progress: { ...active.progress!, job_id: "other", candidate_ids: [] },
  };
  snapshot(store, [active, added], true, false);
  expect(store.state.capacity).toBe(true);
  expect(store.state.jobs.size).toBe(1);
  expect(budget.used).toBe(4096);
  budget.release("other");
  snapshot(store, [active]);
  expect(store.state.capacity).toBe(false);
  expect(store.state.stale).toBe(false);
  store.dispose();
  expect(budget.used).toBe(0);
});

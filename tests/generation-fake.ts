import { vi } from "vitest";
import type { Bridge, ProfileInfo } from "../src/application/bridge";
import type {
  Preparation,
  SelectedSnippet,
} from "../src/application/generation/controller";
import type {
  GenerationInput,
  GenerationView,
} from "../src/application/generation/contracts";
import { generationView, candidateView } from "./generation-fixtures";
export const readyProfile: ProfileInfo = {
  alias: "main",
  type: "codex",
  status: "ready",
  reason: null,
  capabilities: {
    chat: { available: true, reason: null },
    generate: { available: true, reason: null },
  },
};
export function runningView(count = 2): GenerationView {
  const v = generationView();
  v.count = count;
  v.candidates = Array.from({ length: count }, (_, i) => ({
    index: i + 1,
    id: null,
    status: i === 0 ? "running" : "not_started",
    error_code: null,
  }));
  v.progress = {
    ...v.progress,
    requested_count: count,
    finished_count: 0,
    ready_count: 0,
    not_started_count: count - 1,
    candidate_index: 1,
    candidate_ids: [],
  };
  return v;
}
export function generationServer() {
  let snapshot = runningView();
  let dirty = false;
  const input: GenerationInput[] = [];
  let inference = 0;
  const admitted = new Map<string, string>();
  const bridge = {
    listProfiles: vi.fn(async () => [readyProfile]),
    checkProfile: vi.fn(async () => readyProfile),
    startGeneration: vi.fn(async (i: GenerationInput) => {
      input.push(structuredClone(i));
      if (!admitted.has(i.request_id)) {
        admitted.set(i.request_id, "j");
        inference++;
      }
      snapshot = runningView(i.count);
      snapshot.date = i.date;
      snapshot.locale = i.locale;
      snapshot.profile = i.profile;
      snapshot.progress.origin_date = i.date;
      snapshot.progress.locale = i.locale;
      snapshot.progress.profile_id = i.profile;
      return { job: snapshot.job };
    }),
    getGeneration: vi.fn(async () => structuredClone(snapshot)),
    readCandidate: vi.fn(async (id: string) => ({
      ...candidateView(),
      id,
      index: Number(id.split("-").at(-1)),
      date: snapshot.date,
    })),
    discardGeneration: vi.fn(async () => {}),
    cancelJob: vi.fn(async () => {
      snapshot.job.state = "cancelled";
      snapshot.candidates[0].status = "cancelled";
      snapshot.progress.cancelled_count = 1;
      snapshot.progress.finished_count = 1;
      snapshot.provisional = null;
      return { id: "j", state: "cancelled", sequence: 5 };
    }),
  } as unknown as Bridge;
  const sources: SelectedSnippet[] = [
    { import_id: "i", source_id: "a", digest: "d" },
    { import_id: "i", source_id: "b", digest: "d" },
  ];
  const preparation: Preparation = {
    selected: vi.fn(() => structuredClone(sources)),
    dirty: vi.fn((selected) =>
      dirty && selected.some((s) => s.source_id === "a")
        ? [{ id: "a", label: "Source A" }]
        : [],
    ),
    saveAndRefresh: vi.fn(async (_date, selected) => {
      dirty = false;
      return selected.map((s) => ({ ...s, import_id: "fresh" }));
    }),
    flushAuthentic: vi.fn(async () => true),
    chatRunning: vi.fn(() => false),
  };
  return {
    bridge,
    preparation,
    input,
    sources,
    inference: () => inference,
    dirty: (value: boolean) => {
      dirty = value;
    },
    snapshot: () => snapshot,
    finish() {
      snapshot.job.state = "succeeded";
      snapshot.candidates = snapshot.candidates.map((s, i) => ({
        ...s,
        status: i === 1 ? "failed" : "ready",
        id: i === 1 ? null : `j-${i + 1}`,
        error_code: i === 1 ? "timeout" : null,
      }));
      snapshot.progress.ready_count = snapshot.count - 1;
      snapshot.progress.failed_count = 1;
      snapshot.progress.finished_count = snapshot.count;
      snapshot.progress.not_started_count = 0;
      snapshot.progress.candidate_ids = snapshot.candidates.flatMap((s) =>
        s.id ? [s.id] : [],
      );
      snapshot.provisional = null;
    },
  };
}

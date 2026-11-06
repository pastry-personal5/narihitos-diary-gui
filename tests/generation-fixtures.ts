import type {
  CandidateView,
  GenerationView,
} from "../src/application/generation/contracts";
export function generationView(): GenerationView {
  return {
    job: { id: "j", sequence: 3, state: "running" },
    date: "2026-11-06",
    profile: "main",
    locale: "ko-KR",
    count: 2,
    stale_baseline: false,
    candidates: [
      { index: 1, id: "j-1", status: "ready", error_code: null },
      { index: 2, id: null, status: "running", error_code: null },
    ],
    progress: {
      kind: "generation",
      job_id: "j",
      request_id: "r",
      instance_id: "i",
      session_id: "s",
      origin_date: "2026-11-06",
      profile_id: "main",
      locale: "ko-KR",
      baseline_revision: null,
      candidate_index: 2,
      requested_count: 2,
      finished_count: 1,
      ready_count: 1,
      failed_count: 0,
      cancelled_count: 0,
      not_started_count: 0,
      phase: "receiving_output",
      elapsed_ms: 10,
      last_agent_activity_at: 10,
      cancellation_requested: false,
      candidate_ids: ["j-1"],
      error_code: null,
    },
    provisional: "preview",
  };
}
export function candidateView(): CandidateView {
  return {
    id: "j-1",
    job_id: "j",
    index: 1,
    date: "2026-11-06",
    draft_markdown: "draft",
    claim_support: [{ claim: "claim", snippet_ids: ["snippet-1"] }],
    warnings: [],
  };
}

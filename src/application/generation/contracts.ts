import type { Revision } from "../bridge";
import type { SnapshotJob } from "../events";
export interface GenerationInput {
  request_id: string;
  date: string;
  selections: { import_id: string; source_id: string }[];
  profile: string;
  locale: string;
  count: number;
}
export interface GenerationProgress {
  kind: "generation";
  job_id: string;
  request_id: string;
  instance_id: string;
  session_id: string;
  origin_date: string;
  profile_id: string;
  locale: string;
  baseline_revision: Revision | null;
  candidate_index: number | null;
  requested_count: number;
  finished_count: number;
  ready_count: number;
  failed_count: number;
  cancelled_count: number;
  not_started_count: number;
  phase: string | null;
  elapsed_ms: number;
  silence_ms?: number;
  last_agent_activity_at: number | null;
  cancellation_requested: boolean;
  candidate_ids: string[];
  error_code: string | null;
}
export interface CandidateSlot {
  index: number;
  id: string | null;
  status: "not_started" | "running" | "ready" | "failed" | "cancelled";
  error_code: string | null;
}
export interface GenerationView {
  job: SnapshotJob;
  date: string;
  profile: string;
  locale: string;
  count: number;
  stale_baseline: boolean;
  candidates: CandidateSlot[];
  progress: GenerationProgress;
  provisional: string | null;
}
export interface CandidateView {
  id: string;
  job_id: string;
  index: number;
  date: string;
  draft_markdown: string;
  claim_support: { claim: string; snippet_ids: string[] }[];
  warnings: string[];
}
export interface GenerationBridge {
  startGeneration?(input: GenerationInput): Promise<{ job: SnapshotJob }>;
  getGeneration?(jobId: string): Promise<GenerationView>;
  readCandidate?(candidateId: string): Promise<CandidateView>;
  discardGeneration?(jobId: string): Promise<void>;
}

export const candidateKey = (job: string, index: number) =>
  `candidate:${job}:${index}`;

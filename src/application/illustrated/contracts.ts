import type { SnapshotJob } from "../events";

export interface IllustratedEntry {
  date: string;
  version: string;
  markdown: string;
  source_authentic_id: string;
  source_revision: number;
  source_sha256: string;
  agent_profile: string;
  image_engine: string;
  theme_count: number;
  photos: { file: string; seed: number; prompt: string }[];
}
export interface IllustratedInput {
  request_id: string;
  profile: string;
  locale: string;
  engine: null;
}
export interface ImageEngine {
  id: string;
  label: string;
  status: "unchecked" | "ready" | "unavailable" | "busy";
  reason: "prerequisites_unavailable" | null;
}
export interface ImageEngines {
  image_engines: ImageEngine[];
  illustrated_engine: string | null;
  illustrated_available: boolean;
}
export interface IllustratedProgress {
  kind: "illustrated";
  job_id: string;
  instance_id: string;
  session_id: string;
  origin_date: string;
  phase: "text" | "photo" | "stopping" | "finished";
  photo_index: number;
  photo_count: number;
  error_code: string | null;
}
/** Optional in older/fake bridges; absence leaves the feature unavailable. */
export interface IllustratedBridge {
  listIllustrated?(): Promise<string[]>;
  readIllustrated?(date: string): Promise<IllustratedEntry>;
  createIllustrated?(
    date: string,
    input: IllustratedInput,
  ): Promise<{ job_id: string }>;
  deleteIllustrated?(date: string, version: string): Promise<void>;
  readIllustratedPhoto?(
    date: string,
    version: string,
    n: number,
  ): Promise<Uint8Array>;
  listImageEngines?(): Promise<ImageEngines>;
  checkImageEngine?(id: string): Promise<ImageEngine>;
}
export const illustratedRunning = (job: SnapshotJob) =>
  job.progress?.kind === "illustrated" &&
  ["queued", "running"].includes(job.state);

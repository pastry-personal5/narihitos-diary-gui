import type {
  IllustratedEntry,
  IllustratedProgress,
  ImageEngines,
} from "../src/application/illustrated/contracts";
import type { ProfileInfo } from "../src/application/bridge";
export const illustratedDate = "2026-11-13";
export const illustratedEntry = (
  patch: Partial<IllustratedEntry> = {},
): IllustratedEntry => ({
  date: illustratedDate,
  version: "11111111-1111-4111-8111-111111111111",
  markdown: `# A day\n\nA **quiet** morning.\n\n![Morning tea](${illustratedDate}/01.png)\n`,
  source_authentic_id: "22222222-2222-4222-8222-222222222222",
  source_revision: 1,
  source_sha256: "a".repeat(64),
  agent_profile: "main",
  image_engine: "local",
  theme_count: 1,
  photos: [{ file: "01.png", seed: 0, prompt: "private prompt" }],
  ...patch,
});
export const illustratedProfile: ProfileInfo = {
  alias: "main",
  type: "codex",
  status: "ready",
  reason: null,
  capabilities: {
    chat: { available: true, reason: null },
    generate: { available: true, reason: null },
    illustrated: { available: true, reason: null },
  },
};
export const imageEngines = (): ImageEngines => ({
  image_engines: [
    { id: "local", label: "Local", status: "ready", reason: null },
  ],
  illustrated_engine: "local",
  illustrated_available: true,
});
export const illustratedProgress = (
  patch: Partial<IllustratedProgress> = {},
): IllustratedProgress => ({
  kind: "illustrated",
  job_id: "job",
  instance_id: "i",
  session_id: "s",
  origin_date: illustratedDate,
  phase: "text",
  photo_index: 0,
  photo_count: 0,
  error_code: null,
  ...patch,
});
export function deferred<T>() {
  let resolve!: (v: T) => void;
  const promise = new Promise<T>((r) => {
    resolve = r;
  });
  return { resolve, promise };
}
export function png(width = 1, height = 1, depth = 8) {
  const bytes = new Uint8Array(33);
  bytes.set([137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82]);
  bytes[24] = depth;
  const view = new DataView(bytes.buffer);
  view.setUint32(16, width);
  view.setUint32(20, height);
  return bytes;
}

import { importJobDto } from "./navigation-validation";
import type {
  IllustratedEntry,
  ImageEngine,
  ImageEngines,
} from "../application/illustrated/contracts";
import { civilDate } from "./navigation-validation";
const bad = (): never => {
  throw new Error("Invalid Illustrated response");
};
const obj = (v: unknown): Record<string, unknown> =>
  v !== null && typeof v === "object" && !Array.isArray(v)
    ? (v as Record<string, unknown>)
    : bad();
const id = (v: unknown): string =>
  typeof v === "string" && /^[a-zA-Z0-9_-]{1,64}$/u.test(v) ? v : bad();
const uuid = (v: unknown): string =>
  typeof v === "string" &&
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/iu.test(v)
    ? v
    : bad();
const integer = (
  v: unknown,
  min: number,
  max = Number.MAX_SAFE_INTEGER,
): number =>
  Number.isSafeInteger(v) && (v as number) >= min && (v as number) <= max
    ? (v as number)
    : bad();
function text(v: unknown, max: number, empty = false): string {
  return typeof v === "string" &&
    (empty || v.trim().length > 0) &&
    new TextEncoder().encode(v).length <= max
    ? v
    : bad();
}
export function illustratedDatesDto(value: unknown): string[] {
  const v = obj(value);
  if (!Array.isArray(v.dates) || v.dates.length > 3660000) return bad();
  const dates = v.dates.map(civilDate);
  if (dates.some((d, i) => i > 0 && dates[i - 1] >= d)) return bad();
  return dates;
}
export function illustratedEntryDto(
  value: unknown,
  date: string,
): IllustratedEntry {
  const v = obj(value);
  if (
    civilDate(v.date) !== date ||
    !Array.isArray(v.photos) ||
    !v.photos.length ||
    v.photos.length > 99
  )
    return bad();
  const photos = v.photos.map((value, i) => {
    const p = obj(value);
    if (p.file !== `${String(i + 1).padStart(2, "0")}.png`) return bad();
    return {
      file: p.file as string,
      seed: integer(p.seed, 0, 0xffffffff),
      prompt: text(p.prompt, 32768),
    };
  });
  const sha = text(v.source_sha256, 64);
  if (!/^[a-f0-9]{64}$/iu.test(sha)) return bad();
  return {
    date,
    version: uuid(v.version),
    markdown: text(v.markdown, 1024 * 1024, true),
    source_authentic_id: uuid(v.source_authentic_id),
    source_revision: integer(v.source_revision, 1),
    source_sha256: sha,
    agent_profile: id(v.agent_profile),
    image_engine: id(v.image_engine),
    theme_count: integer(v.theme_count, photos.length, 1024),
    photos,
  };
}
export function imageEngineDto(value: unknown): ImageEngine {
  const v = obj(value);
  if (
    !["unchecked", "ready", "unavailable", "busy"].includes(
      v.status as string,
    ) ||
    v.reason !==
      (v.status === "unavailable" ? "prerequisites_unavailable" : null)
  )
    return bad();
  const label = text(v.label, 256);
  if (
    [...label].some((c) => c.codePointAt(0)! < 32 || c.codePointAt(0) === 127)
  )
    return bad();
  return {
    id: id(v.id),
    label,
    status: v.status as ImageEngine["status"],
    reason: v.reason as ImageEngine["reason"],
  };
}
export function imageEnginesDto(value: unknown): ImageEngines {
  const v = obj(value);
  if (
    !Array.isArray(v.image_engines) ||
    v.image_engines.length > 64 ||
    typeof v.illustrated_available !== "boolean"
  )
    return bad();
  const engines = v.image_engines.map(imageEngineDto);
  const selected =
    v.illustrated_engine === null ? null : id(v.illustrated_engine);
  if (
    new Set(engines.map((e) => e.id)).size !== engines.length ||
    (selected !== null && !engines.some((e) => e.id === selected)) ||
    (v.illustrated_available && selected === null)
  )
    return bad();
  return {
    image_engines: engines,
    illustrated_engine: selected,
    illustrated_available: v.illustrated_available,
  };
}
export function illustratedPhotoDto(value: unknown): Uint8Array {
  if (
    !Array.isArray(value) ||
    value.length < 33 ||
    value.length > 32 * 1024 * 1024 ||
    value.some((v) => !Number.isInteger(v) || v < 0 || v > 255)
  )
    return bad();
  const bytes = Uint8Array.from(value);
  if (![137, 80, 78, 71, 13, 10, 26, 10].every((v, i) => bytes[i] === v))
    return bad();
  return bytes;
}

export function illustratedAcceptedDto(value: unknown): { job_id: string } {
  return { job_id: importJobDto(obj(value).job).id };
}

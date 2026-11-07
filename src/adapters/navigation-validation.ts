import { aiProgress } from "../application/activity/progress";
import type {
  Calendar,
  ImportPage,
  ImportJob,
  ImportAck,
  Source,
  SourceBody,
  SourceDating,
  SourceInstant,
  LocalTime,
  SourceCapabilities,
  SourceSaved,
  SourceSaveReason,
  SourceTrashReason,
  SourceItem,
  SourceFailure,
  SourceGroup,
} from "../application/navigation/contracts";

function bad(): never {
  throw new Error("Invalid navigation response");
}
function obj(v: unknown): Record<string, unknown> {
  if (!v || typeof v !== "object" || Array.isArray(v)) return bad();
  return v as Record<string, unknown>;
}
function str(v: unknown, max = 64, empty = false): string {
  if (
    typeof v !== "string" ||
    v.length > max ||
    (!empty && !v.length) ||
    [...v].some((char) => {
      const code = char.codePointAt(0)!;
      return code < 32 || code === 127;
    })
  )
    return bad();
  return v;
}
function bool(v: unknown): boolean {
  if (typeof v !== "boolean") return bad();
  return v;
}
function num(v: unknown, min = 0, max = Number.MAX_SAFE_INTEGER): number {
  if (!Number.isSafeInteger(v) || (v as number) < min || (v as number) > max)
    return bad();
  return v as number;
}
function arr(v: unknown, max: number): unknown[] {
  if (!Array.isArray(v) || v.length > max) return bad();
  return v;
}
function id(v: unknown): string {
  const s = str(v);
  if (!/^[a-zA-Z0-9_-]+$/u.test(s)) return bad();
  return s;
}
export function civilDate(v: unknown): string {
  const s = str(v, 10);
  if (!/^\d{4}-\d{2}-\d{2}$/u.test(s)) return bad();
  const [y, m, d] = s.split("-").map(Number);
  const leap = y % 4 === 0 && (y % 100 !== 0 || y % 400 === 0);
  if (
    y < 1 ||
    m < 1 ||
    m > 12 ||
    d < 1 ||
    d > [31, leap ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][m - 1]
  )
    return bad();
  return s;
}
const maybeDate = (v: unknown) => (v === null ? null : civilDate(v));
function monthLength(year: number, month: number): number {
  const leap = year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0);
  return [31, leap ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][
    month - 1
  ];
}
function firstWeekday(year: number, month: number): number {
  const previous = year - 1;
  let days =
    365 * previous +
    Math.floor(previous / 4) -
    Math.floor(previous / 100) +
    Math.floor(previous / 400) +
    1;
  for (let m = 1; m < month; m++) days += monthLength(year, m);
  return days % 7;
}
function adjacentMonth(anchor: string, direction: -1 | 1): string | null {
  let year = Number(anchor.slice(0, 4)),
    month = Number(anchor.slice(5, 7)) + direction;
  if (month === 0) {
    year--;
    month = 12;
  }
  if (month === 13) {
    year++;
    month = 1;
  }
  if (year < 1 || year > 9999) return null;
  const day = Math.min(Number(anchor.slice(8, 10)), monthLength(year, month));
  return `${String(year).padStart(4, "0")}-${String(month).padStart(2, "0")}-${String(day).padStart(2, "0")}`;
}
export function calendarDto(v: unknown): Calendar {
  const d = obj(v),
    anchor = civilDate(d.anchor),
    year = num(d.year, 1, 9999),
    month = num(d.month, 1, 12);
  if (
    Number(anchor.slice(0, 4)) !== year ||
    Number(anchor.slice(5, 7)) !== month
  )
    return bad();
  const days = arr(d.days, 31).map((v, i) => {
    const x = obj(v),
      date = civilDate(x.date),
      day = num(x.day, 1, 31),
      weekday = num(x.weekday, 0, 6);
    if (
      day !== i + 1 ||
      date !== `${anchor.slice(0, 7)}-${String(day).padStart(2, "0")}`
    )
      return bad();
    return { date, day, weekday };
  });
  const previous = maybeDate(d.previous_month),
    next = maybeDate(d.next_month);
  if (
    days.length !== monthLength(year, month) ||
    days.some(
      (day, i) => day.weekday !== (firstWeekday(year, month) + i) % 7,
    ) ||
    previous !== adjacentMonth(anchor, -1) ||
    next !== adjacentMonth(anchor, 1)
  )
    return bad();
  return {
    anchor,
    today: civilDate(d.today),
    year,
    month,
    days,
    previous_month: previous,
    next_month: next,
  };
}
function instant(v: unknown): SourceInstant {
  const d = obj(v);
  if (d.precision !== "second" && d.precision !== "milli") return bad();
  return {
    unix_ms: num(d.unix_ms, -Number.MAX_SAFE_INTEGER),
    precision: d.precision,
  };
}
function dating(v: unknown): SourceDating {
  const d = obj(v);
  switch (d.state) {
    case "agreed":
    case "timestamp_conflict":
      return {
        state: d.state,
        header: instant(d.header),
        filename: instant(d.filename),
      };
    case "header_only":
      return { state: d.state, header: instant(d.header) };
    case "filename_only":
      return { state: d.state, filename: instant(d.filename) };
    case "undated":
      return { state: d.state };
    default:
      return bad();
  }
}
function local(v: unknown): LocalTime | null {
  if (v === null) return null;
  const d = obj(v),
    date = civilDate(d.date),
    datetime = str(d.datetime, 64);
  if (!datetime.startsWith(`${date}T`)) return bad();
  return {
    date,
    datetime,
    offset_seconds: num(d.offset_seconds, -86400, 86400),
  };
}
const SAVE_REASONS: string[] = [
  "ready",
  "producer_unverified",
  "writer_unverified",
  "unsupported_format",
  "checked_on_open",
  "reimport_required",
];
export function sourceSaved(v: unknown): SourceSaved {
  const d = obj(v),
    digest = str(d.digest);
  if (!/^[0-9a-f]{64}$/u.test(digest)) return bad();
  return {
    operation_id: id(d.operation_id),
    digest,
    size: num(d.size, 0, 9 * 1024 * 1024),
    unchanged: bool(d.unchanged),
  };
}
function caps(v: unknown): SourceCapabilities {
  const d = obj(v);
  const save = bool(d.save);
  const trash = bool(d.trash);
  if (
    typeof d.trash_reason !== "string" ||
    ![
      "ready",
      "producer_unverified",
      "trash_unverified",
      "unsupported_format",
      "checked_on_preview",
      "reimport_required",
    ].includes(d.trash_reason) ||
    trash !== (d.trash_reason === "ready") ||
    typeof d.reason !== "string" ||
    !SAVE_REASONS.includes(d.reason as SourceSaveReason) ||
    save !== (d.reason === "ready")
  )
    return bad();
  return {
    read: bool(d.read),
    save,
    trash,
    trash_reason: d.trash_reason as SourceTrashReason,
    reason: d.reason as SourceSaveReason,
  };
}
function nullableReason(v: unknown): string | null {
  if (v === null) return null;
  const s = str(v, 32);
  if (!["format", "offset", "calendar", "directory_mismatch"].includes(s))
    return bad();
  return s;
}
function source(v: unknown): Source {
  const d = obj(v),
    n = d.navigation === null ? null : obj(d.navigation),
    authority = n?.authority;
  if (authority != null && authority !== "header" && authority !== "filename")
    return bad();
  const digest = str(d.digest);
  if (!/^[0-9a-f]{64}$/u.test(digest)) return bad();
  const result: Source = {
    id: id(d.id),
    relative_key: str(d.relative_key, 4096),
    digest,
    import_id: id(d.import_id),
    dating: dating(d.dating),
    duplicate_producer_id: bool(d.duplicate_producer_id),
    header_invalid: nullableReason(d.header_invalid),
    filename_invalid: nullableReason(d.filename_invalid),
    eligible: bool(d.eligible),
    capabilities: caps(d.capabilities),
    navigation: n
      ? {
          group_id: id(n.group_id),
          header: local(n.header),
          filename: local(n.filename),
          included: local(n.included),
          authority: authority ?? null,
        }
      : null,
  };
  if (
    result.capabilities.read !== !!result.navigation ||
    (result.eligible && !result.navigation?.included)
  )
    return bad();
  return result;
}
export function importPageDto(v: unknown): ImportPage {
  const d = obj(v),
    importId = id(d.import_id);
  const groups =
    d.groups === null
      ? null
      : arr(d.groups, 64).map((v): SourceGroup => {
          const g = obj(v);
          if (g.type !== "ayumi" && g.type !== "narihito") return bad();
          return { id: id(g.id), alias: str(g.alias), type: g.type };
        });
  const reimport = bool(d.reimport_required);
  if (
    reimport !== (groups === null) ||
    (groups && new Set(groups.map((g) => g.id)).size !== groups.length)
  )
    return bad();
  const group = (v: unknown) => {
    if (v === null) {
      if (!reimport) return bad();
      return null;
    }
    const s = id(v);
    if (!groups?.some((g) => g.id === s)) return bad();
    return s;
  };
  const readSource = (v: unknown) => {
    const s = source(v);
    if (s.import_id !== importId) return bad();
    if (s.navigation) group(s.navigation.group_id);
    return s;
  };
  const items = arr(d.items, 100).map((v): SourceItem => {
    const row = obj(v);
    switch (row.kind) {
      case "included": {
        const s = readSource(row.source);
        if (!reimport && !s.eligible) return bad();
        return {
          kind: row.kind,
          source: s,
          instant: instant(row.instant),
          misplaced: bool(row.misplaced),
        };
      }
      case "warning": {
        const s = readSource(row.source);
        if (s.eligible) return bad();
        return { kind: row.kind, source: s, misplaced: bool(row.misplaced) };
      }
      case "failure": {
        const code = str(row.code, 32);
        if (
          ![
            "missing",
            "denied",
            "not_downloaded",
            "not_regular",
            "too_large",
            "source_changing",
            "invalid_path",
            "unavailable",
            "malformed",
            "unsupported",
            "conflict",
          ].includes(code)
        )
          return bad();
        const s = row.source === null ? null : readSource(row.source);
        if (s?.eligible) return bad();
        return {
          kind: row.kind,
          relative_key: str(row.relative_key, 4096, true),
          code: code as SourceFailure,
          group_id: group(row.group_id),
          source: s,
        };
      }
      case "path_collision":
        return {
          kind: row.kind,
          relative_keys: arr(row.relative_keys, 100000).map((x) =>
            str(x, 4096),
          ),
          group_id: group(row.group_id),
        };
      default:
        return bad();
    }
  });
  return {
    import_id: importId,
    date: civilDate(d.date),
    zone: str(d.zone, 128),
    timezone_database: str(d.timezone_database, 32),
    config_generation: num(d.config_generation, 1),
    completed_at: num(d.completed_at, -Number.MAX_SAFE_INTEGER),
    items,
    next_cursor: d.next_cursor === null ? null : num(d.next_cursor, 1, 30000),
    incomplete: bool(d.incomplete),
    groups,
    reimport_required: reimport,
  };
}
export function sourceBodyDto(v: unknown): SourceBody {
  const d = obj(v);
  if (
    typeof d.body !== "string" ||
    new TextEncoder().encode(d.body).byteLength > 1024 * 1024
  )
    return bad();
  const front = d.front_matter ?? "";
  if (typeof front !== "string" || front.length > 64 * 1024) return bad();
  const s = source(d.source),
    snapshot = id(d.snapshot_id);
  if (s.import_id !== snapshot || !s.capabilities.read) return bad();
  return {
    source: s,
    snapshot_id: snapshot,
    body: d.body,
    front_matter: front,
    capabilities: caps(d.capabilities),
  };
}
export function importJobDto(v: unknown): ImportJob {
  const d = obj(v),
    state = str(d.state, 16);
  if (
    !["queued", "running", "succeeded", "failed", "cancelled"].includes(state)
  )
    return bad();
  const error = d.error_code === undefined ? undefined : str(d.error_code, 40);
  if (error && !/^[a-z_]+$/u.test(error)) return bad();
  return {
    id: id(d.id),
    sequence: num(d.sequence, 1),
    state: state as ImportJob["state"],
    ...(d.progress === undefined
      ? {}
      : { progress: aiProgress(d.progress, id(d.id)) }),
    ...(error ? { error_code: error } : {}),
  };
}
export function importAckDto(v: unknown): ImportAck {
  const d = obj(v),
    job = importJobDto(d.job),
    importId = id(d.import_id);
  if (job.id !== importId) return bad();
  return { import_id: importId, job };
}
export function resolutionDto(v: unknown): { resolved: true } {
  if (obj(v).resolved !== true) return bad();
  return { resolved: true };
}

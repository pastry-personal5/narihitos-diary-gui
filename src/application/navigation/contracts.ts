/** Public navigation DTOs. Native observations and source roots stay in Rust. */
export type CalendarAction =
  | "month"
  | "previous_month"
  | "next_month"
  | "previous_day"
  | "next_day"
  | "previous_week"
  | "next_week"
  | "today";
export interface CalendarDay {
  date: string;
  day: number;
  weekday: number;
}
export interface Calendar {
  anchor: string;
  today: string;
  year: number;
  month: number;
  days: CalendarDay[];
  previous_month: string | null;
  next_month: string | null;
}
export type TimeAuthority = "header" | "filename";
export interface SourceInstant {
  unix_ms: number;
  precision: "second" | "milli";
}
export type SourceDating =
  | {
      state: "agreed" | "timestamp_conflict";
      header: SourceInstant;
      filename: SourceInstant;
    }
  | { state: "header_only"; header: SourceInstant }
  | { state: "filename_only"; filename: SourceInstant }
  | { state: "undated" };
export interface LocalTime {
  date: string;
  datetime: string;
  offset_seconds: number;
}
export interface SourceDisplay {
  group_id: string;
  header: LocalTime | null;
  filename: LocalTime | null;
  included: LocalTime | null;
  authority: TimeAuthority | null;
}
export interface SourceCapabilities {
  read: boolean;
  save: boolean;
  trash: boolean;
  /** Adapter readiness; a fresh preview must still prove exact member ownership. */
  trash_reason: SourceTrashReason;
  /** `ready` exactly when `save` is true. */
  reason: SourceSaveReason;
}
export type SourceTrashReason =
  | "ready"
  | "producer_unverified"
  | "trash_unverified"
  | "unsupported_format"
  | "checked_on_preview"
  | "reimport_required";
export type SourceSaveReason =
  | "ready"
  | "producer_unverified"
  | "writer_unverified"
  | "unsupported_format"
  | "checked_on_open"
  | "reimport_required";
export interface SourceSaved {
  operation_id: string;
  digest: string;
  size: number;
  unchanged: boolean;
}
export interface Source {
  id: string;
  relative_key: string;
  digest: string;
  import_id: string;
  dating: SourceDating;
  duplicate_producer_id: boolean;
  header_invalid: string | null;
  filename_invalid: string | null;
  navigation: SourceDisplay | null;
  eligible: boolean;
  capabilities: SourceCapabilities;
}
export type SourceFailure =
  | "missing"
  | "denied"
  | "not_downloaded"
  | "not_regular"
  | "too_large"
  | "source_changing"
  | "invalid_path"
  | "unavailable"
  | "malformed"
  | "unsupported"
  | "conflict";
export type SourceItem =
  | {
      kind: "included";
      source: Source;
      instant: SourceInstant;
      misplaced: boolean;
    }
  | { kind: "warning"; source: Source; misplaced: boolean }
  | {
      kind: "failure";
      relative_key: string;
      code: SourceFailure;
      group_id: string | null;
      source: Source | null;
    }
  | {
      kind: "path_collision";
      relative_keys: string[];
      group_id: string | null;
    };
export interface SourceGroup {
  id: string;
  alias: string;
  type: "ayumi" | "narihito";
}
export interface ImportPage {
  import_id: string;
  date: string;
  zone: string;
  timezone_database: string;
  config_generation: number;
  completed_at: number;
  items: SourceItem[];
  next_cursor: number | null;
  incomplete: boolean;
  groups: SourceGroup[] | null;
  reimport_required: boolean;
}
export interface SourceBody {
  source: Source;
  snapshot_id: string;
  body: string;
  capabilities: SourceCapabilities;
}
export interface ImportJob {
  progress?: import("../activity/progress").AiProgress;
  id: string;
  sequence: number;
  state: "queued" | "running" | "succeeded" | "failed" | "cancelled";
  error_code?: string;
}
export interface ImportAck {
  import_id: string;
  job: ImportJob;
}
export interface NavigationBridge {
  getCalendar(anchor?: string, action?: CalendarAction): Promise<Calendar>;
  importDate(date: string, requestId: string): Promise<ImportAck>;
  listSources(importId: string, cursor?: number): Promise<ImportPage>;
  resolveSourceTime(
    importId: string,
    sourceId: string,
    digest: string,
    authority: TimeAuthority,
  ): Promise<{ resolved: true }>;
  readSource(sourceId: string, snapshotId: string): Promise<SourceBody>;
  saveSource(
    sourceId: string,
    operationId: string,
    baseDigest: string,
    body: string,
  ): Promise<SourceSaved>;
  getJob(jobId: string): Promise<ImportJob>;
  cancelJob(jobId: string): Promise<ImportJob>;
}

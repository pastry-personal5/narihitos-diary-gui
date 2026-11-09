import { exportResultDto } from "./export-validation";
import type {
  GenerationView,
  CandidateView,
  CandidateSlot,
} from "../application/generation/contracts";
import { trashResultDto } from "./trash-validation";
import {
  AuthenticError,
  type AppMetadata,
  type SymbolResult,
  type AuthenticFile,
  type AuthenticPage,
  type Availability,
  type ProfileInfo,
  type ProfileCapability,
  type ChatSnapshot,
  type ChatTurn,
  type ReloadResult,
  type ServerSettings,
  SettingsError,
  type UserSettings,
  type OperationState,
  type RefreshReport,
  type Revision,
  type SaveAck,
  type Status,
} from "../application/bridge";
function record(value: unknown): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value))
    throw new Error("Invalid server response");
  return value as Record<string, unknown>;
}
function text(value: unknown, max: number): string {
  if (typeof value !== "string" || value.length === 0 || value.length > max)
    throw new Error("Invalid server response");
  return value;
}
export function metadataDto(value: unknown): AppMetadata {
  const dto = record(value);
  const version = text(dto.version, 32);
  if (!/^\d+\.\d+\.\d+$/.test(version)) throw new Error("Invalid app version");
  return { version };
}
export function symbolDto(value: unknown): SymbolResult {
  const dto = record(value);
  if (dto.status === "unsupported") return { status: "unsupported" };
  // 32 KiB of PNG is at most 43,692 base64 characters; the Rust side enforces the byte cap.
  if (
    dto.status !== "ok" ||
    typeof dto.png_base64 !== "string" ||
    dto.png_base64.length === 0 ||
    dto.png_base64.length > 43_700 ||
    !/^[A-Za-z0-9+/]+={0,2}$/.test(dto.png_base64)
  )
    throw new Error("Invalid symbol response");
  return { status: "ok", png_base64: dto.png_base64 };
}
export function statusDto(value: unknown): Status {
  const dto = record(value);
  if (
    dto.api_version !== 1 ||
    !Number.isSafeInteger(dto.api_minor) ||
    (dto.api_minor as number) < 0 ||
    typeof dto.storage_ready !== "boolean"
  )
    throw new Error("Incompatible server protocol");
  if (
    !Array.isArray(dto.capabilities) ||
    dto.capabilities.length > 64 ||
    !dto.capabilities.every(
      (item: unknown) => typeof item === "string" && item.length <= 64,
    )
  )
    throw new Error("Invalid server capabilities");
  const capabilities: string[] = dto.capabilities;
  if (
    !["status", "sessions", "jobs", "events"].every((name) =>
      capabilities.includes(name),
    )
  )
    throw new Error("Incompatible server capabilities");
  // Reconstruct an allowlisted DTO rather than forwarding additive private fields.
  return {
    instance_id: text(dto.instance_id, 64),
    workspace_key: text(dto.workspace_key, 64),
    server_version: text(dto.server_version, 32),
    api_version: 1,
    api_minor: dto.api_minor as number,
    storage_ready: dto.storage_ready,
    capabilities: [...capabilities],
  };
}
const dateText = /^\d{4}-\d{2}-\d{2}$/;
const hex64 = /^[0-9a-f]{64}$/i;
function count(value: unknown): number {
  if (!Number.isSafeInteger(value) || (value as number) < 0)
    throw new Error("Invalid server response");
  return value as number;
}
function date(value: unknown): string {
  if (typeof value !== "string" || !dateText.test(value))
    throw new Error("Invalid server response");
  return value;
}
function revision(value: unknown): Revision {
  const dto = record(value);
  if (
    !Number.isSafeInteger(dto.sequence) ||
    (dto.sequence as number) < 1 ||
    typeof dto.sha256 !== "string" ||
    !hex64.test(dto.sha256)
  )
    throw new Error("Invalid server response");
  return { sequence: dto.sequence as number, sha256: dto.sha256 };
}
const maybeRevision = (value: unknown) =>
  value === null ? null : revision(value);
export function authenticFileDto(value: unknown): AuthenticFile {
  const dto = record(value);
  if (
    typeof dto.exists !== "boolean" ||
    typeof dto.body !== "string" ||
    dto.body.length > 1024 * 1024 * 3
  )
    throw new Error("Invalid server response");
  const rev = maybeRevision(dto.revision);
  if (dto.exists !== (rev !== null)) throw new Error("Invalid server response");
  return {
    date: date(dto.date),
    exists: dto.exists,
    revision: rev,
    body: dto.body,
  };
}
export function saveAckDto(value: unknown): SaveAck {
  const dto = record(value);
  return {
    operation_id: text(dto.operation_id, 64),
    revision: revision(dto.revision),
    buffer_generation: count(dto.buffer_generation),
  };
}
export function authenticPageDto(value: unknown): AuthenticPage {
  const dto = record(value);
  if (!Array.isArray(dto.items) || dto.items.length > 200)
    throw new Error("Invalid server response");
  return {
    items: dto.items.map((item: unknown) => {
      const row = record(item);
      return {
        date: date(row.date),
        revision: maybeRevision(row.revision ?? null),
        problem: row.problem == null ? null : text(row.problem, 256),
      };
    }),
    next_cursor: dto.next_cursor == null ? null : date(dto.next_cursor),
  };
}
export function refreshReportDto(value: unknown): RefreshReport {
  const dto = record(value);
  return { files: count(dto.files), problems: count(dto.problems) };
}
export function operationStateDto(value: unknown): OperationState {
  const dto = record(value);
  if (typeof dto.terminal !== "boolean" || typeof dto.succeeded !== "boolean")
    throw new Error("Invalid server response");
  const trash = dto.trash == null ? undefined : trashResultDto(dto.trash);
  if (
    (dto.kind === "source_trash") !== !!trash ||
    (trash && dto.checkpoint !== trash.checkpoint)
  )
    throw new Error("Invalid server response");
  if (
    trash &&
    (trash.operation_id !== dto.operation_id ||
      dto.terminal !== (trash.state === "succeeded") ||
      dto.succeeded !== (trash.state === "succeeded") ||
      dto.state !==
        (trash.state === "needs_confirmation" ? "paused" : trash.state))
  )
    throw new Error("Invalid server response");
  return {
    operation_id: text(dto.operation_id, 64),
    ...(dto.kind == null ? {} : { kind: text(dto.kind, 64) }),
    ...(dto.checkpoint == null ? {} : { checkpoint: count(dto.checkpoint) }),
    state: text(dto.state, 32),
    terminal: dto.terminal,
    succeeded: dto.succeeded,
    result: dto.result == null ? null : text(dto.result, 256),
    ...(trash ? { trash } : {}),
    ...(dto.export == null ? {} : { export: exportResultDto(dto.export) }),
  };
}
/** Anything that is not the bridge's allowlisted error becomes `unavailable`. */
export function authenticErrorDto(value: unknown): AuthenticError {
  if (typeof value === "object" && value !== null) {
    const dto = value as Record<string, unknown>;
    if (
      typeof dto.code === "string" &&
      /^[a-z_]{1,40}$/.test(dto.code) &&
      typeof dto.retryable === "boolean"
    )
      return new AuthenticError(dto.code, dto.retryable);
  }
  return new AuthenticError("unavailable", false);
}

function list(value: unknown, max: number): unknown[] {
  if (!Array.isArray(value) || value.length > max)
    throw new Error("Invalid server response");
  return value;
}
function availabilityDto(value: unknown): Availability {
  const dto = record(value);
  if (typeof dto.available !== "boolean")
    throw new Error("Invalid server response");
  return {
    alias: text(dto.alias, 64),
    available: dto.available,
    reason: text(dto.reason, 64),
  };
}
export function reloadResultDto(value: unknown): ReloadResult {
  const dto = record(value);
  if (
    dto.outcome !== "applied" &&
    dto.outcome !== "restart_required" &&
    dto.outcome !== "rejected" &&
    dto.outcome !== "busy"
  )
    throw new Error("Invalid server response");
  const generation = dto.config_generation;
  if (
    generation != null &&
    (!Number.isSafeInteger(generation) || (generation as number) < 0)
  )
    throw new Error("Invalid server response");
  return {
    outcome: dto.outcome,
    config_generation: (generation as number | null | undefined) ?? null,
    issues: list(dto.issues ?? [], 16).map((item) => {
      const issue = record(item);
      return { path: text(issue.path, 64), code: text(issue.code, 64) };
    }),
  };
}
export function serverSettingsDto(value: unknown): ServerSettings {
  const dto = record(value);
  if (
    !Number.isSafeInteger(dto.config_generation) ||
    typeof dto.storage_ready !== "boolean"
  )
    throw new Error("Invalid server response");
  return {
    config_generation: dto.config_generation as number,
    last_reload:
      dto.last_reload == null ? null : reloadResultDto(dto.last_reload),
    storage_ready: dto.storage_ready,
    sources: list(dto.sources, 64).map(availabilityDto),
    profiles: list(dto.profiles, 64).map(availabilityDto),
  };
}
export function userSettingsDto(value: unknown): UserSettings {
  const dto = record(value);
  const agentProgress =
    dto.agent_progress == null ? {} : record(dto.agent_progress);
  if (
    Object.keys(agentProgress).length > 512 ||
    Object.entries(agentProgress).some(
      ([alias, enabled]) =>
        !/^[a-zA-Z0-9_-]{1,64}$/u.test(alias) || typeof enabled !== "boolean",
    )
  )
    throw new Error("Invalid settings response");
  const statusLine =
    dto.status_line == null ? undefined : record(dto.status_line);
  if (
    statusLine &&
    (Object.keys(statusLine).length !== 3 ||
      ["generate_ready", "review_ready", "chat_ready"].some(
        (key) => typeof statusLine[key] !== "boolean",
      ))
  )
    throw new Error("Invalid settings response");
  if (
    typeof dto.read_only !== "boolean" ||
    typeof dto.recoverable !== "boolean" ||
    !Number.isInteger(dto.generation_count) ||
    (dto.generation_count as number) < 1 ||
    (dto.generation_count as number) > 5 ||
    (dto.writing_locale != null && typeof dto.writing_locale !== "string")
  )
    throw new Error("Invalid settings response");
  return {
    revision: text(dto.revision, 64),
    read_only: dto.read_only,
    recoverable: dto.recoverable,
    warnings: list(dto.warnings, 16).map((item) => {
      const warning = record(item);
      return {
        section: text(warning.section, 32),
        code: text(warning.code, 32),
      };
    }),
    writing_locale:
      dto.writing_locale == null ? null : text(dto.writing_locale, 35),
    generation_count: dto.generation_count as number,
    status_line: statusLine as unknown as UserSettings["status_line"],
    agent_progress: Object.fromEntries(Object.entries(agentProgress)) as Record<
      string,
      boolean
    >,
    keybindings: dto.keybindings ?? {},
    effective_keybindings: Object.fromEntries(
      Object.entries(record(dto.effective_keybindings)).map(([id, keys]) => [
        text(id, 64),
        list(keys, 8).map((k) => text(k, 64)),
      ]),
    ),
    // Issue ids echo hand-edited file keys (any length, even empty): clip, never reject.
    keybinding_issues: list(dto.keybinding_issues, 1024).map((item) => {
      const issue = record(item);
      if (typeof issue.id !== "string")
        throw new Error("Invalid server response");
      return { id: issue.id.slice(0, 128), code: text(issue.code, 32) };
    }),
  };
}
/** The bridge answers `{code}` or a bare code string; anything else becomes `unavailable`. */
export function settingsErrorDto(value: unknown): SettingsError {
  const code =
    typeof value === "string"
      ? value
      : typeof value === "object" && value !== null
        ? (value as Record<string, unknown>).code
        : undefined;
  return new SettingsError(
    typeof code === "string" && /^[a-z_]{1,40}$/.test(code)
      ? code
      : "unavailable",
  );
}

const PROFILE_STATES = [
  "unchecked",
  "missing",
  "unsupported_version",
  "unavailable",
  "available_auth_unknown",
  "unauthenticated",
  "ready",
  "busy",
  "failed",
];
const PROFILE_REASONS = [
  "assets_missing",
  "assets_invalid",
  "adapter_unimplemented",
];
/** Reasons the server may report for a profile status (shared with the Rust test). */
export const PROFILE_STATUS_REASONS: readonly string[] = [
  "executable_missing",
  "profile_home_missing",
  "profile_home_not_directory",
  "profile_unknown",
  "kind_mismatch",
  "version_unsupported",
  "version_unreadable",
  "probe_timed_out",
  "probe_failed",
  "spawn_failed",
  "host_busy",
  "run_dir_unavailable",
  "assets_missing",
  "assets_invalid",
  "prompt_pin_rejected",
  "packet_too_large",
  "os_unsupported",
  "device_ineligible",
  "apple_intelligence_disabled",
  "model_not_ready",
  "model_unavailable_unknown",
  "locale_unsupported",
  "helper_missing",
  "helper_untrusted",
  "helper_failed",
];
function profileCapabilityDto(value: unknown): ProfileCapability {
  const dto = record(value);
  const reason = dto.reason ?? null;
  if (
    typeof dto.available !== "boolean" ||
    (reason !== null &&
      (typeof reason !== "string" || !PROFILE_REASONS.includes(reason))) ||
    dto.available !== (reason === null)
  )
    throw new Error("Invalid server response");
  return {
    available: dto.available,
    reason: reason as ProfileCapability["reason"],
  };
}
function profileStatusReason(value: unknown): string | null {
  if (value === undefined || value === null) return null;
  if (typeof value !== "string" || !PROFILE_STATUS_REASONS.includes(value))
    throw new Error("Invalid server response");
  return value;
}
export function profileDto(value: unknown): ProfileInfo {
  const dto = record(value);
  const caps = record(dto.capabilities);
  const status = text(dto.status, 32);
  if (!PROFILE_STATES.includes(status))
    throw new Error("Invalid server response");
  return {
    alias: text(dto.alias, 64),
    type: text(dto.type, 32),
    status: status as ProfileInfo["status"],
    reason: profileStatusReason(dto.reason),
    capabilities: {
      chat: profileCapabilityDto(caps.chat),
      generate: profileCapabilityDto(caps.generate),
    },
  };
}
export function profileListDto(value: unknown): ProfileInfo[] {
  return list(record(value).profiles, 64).map(profileDto);
}

const CHAT_STATES = ["running", "done", "failed", "cancelled"];
const CHAT_STAGES = [
  "preparing_context",
  "starting_agent",
  "waiting_for_output",
  "receiving_output",
  "validating_output",
  "between_candidates",
  "stopping",
  "finished",
];
const AGENT_ERRORS = [
  "busy",
  "cancelled",
  "timeout",
  "context",
  "invalid_output",
  "unavailable",
  "assets_unavailable",
  "unsupported_language",
  "refusal",
  "rate_limited",
  "failed",
];
const MAX_REPLY = 1024 * 1024;
export function chatOpenedDto(value: unknown): { chat_id: string } {
  return { chat_id: text(record(value).chat_id, 64) };
}
export function turnAcceptedDto(value: unknown): { job_id: string } {
  return { job_id: text(record(record(value).job).id, 64) };
}
function chatTurnDto(value: unknown): ChatTurn | null {
  if (value === null || value === undefined) return null;
  const dto = record(value);
  const state = text(dto.state, 16);
  const stage = dto.stage == null ? null : text(dto.stage, 32);
  const code = dto.error_code == null ? null : text(dto.error_code, 32);
  if (
    !CHAT_STATES.includes(state) ||
    (stage !== null && !CHAT_STAGES.includes(stage)) ||
    (code !== null && !AGENT_ERRORS.includes(code)) ||
    !Number.isSafeInteger(dto.revision) ||
    (dto.revision as number) < 0 ||
    (dto.provisional != null &&
      (typeof dto.provisional !== "string" ||
        dto.provisional.length > MAX_REPLY))
  )
    throw new Error("Invalid server response");
  return {
    job_id: text(dto.job_id, 64),
    state: state as ChatTurn["state"],
    error_code: code,
    stage,
    revision: dto.revision as number,
    provisional: (dto.provisional as string | null | undefined) ?? null,
  };
}
export function chatDto(value: unknown): ChatSnapshot {
  const dto = record(value);
  const transcript = list(dto.transcript, 102).map((item) => {
    const m = record(item);
    if (m.role !== "user" && m.role !== "assistant")
      throw new Error("Invalid server response");
    if (typeof m.text !== "string" || m.text.length > MAX_REPLY)
      throw new Error("Invalid server response");
    return { role: m.role as "user" | "assistant", text: m.text };
  });
  return {
    chat_id: text(dto.chat_id, 64),
    profile: text(dto.profile, 64),
    type: text(dto.type, 32),
    locale: text(dto.locale, 35),
    transcript,
    turn: chatTurnDto(dto.turn),
  };
}

const SLOT_STATES = ["not_started", "running", "ready", "failed", "cancelled"];
function generationCode(value: unknown): string | null {
  if (value === null) return null;
  const c = text(value, 40);
  if (!AGENT_ERRORS.includes(c)) throw new Error("Invalid generation response");
  return c;
}
function generationId(value: unknown): string {
  const id = text(value, 64);
  if (!/^[a-zA-Z0-9_-]+$/u.test(id))
    throw new Error("Invalid generation response");
  return id;
}
function generationText(value: unknown, max: number): string {
  if (typeof value !== "string" || new TextEncoder().encode(value).length > max)
    throw new Error("Invalid generation response");
  return value;
}
export function generationAcceptedDto(value: unknown): {
  job: GenerationView["job"];
} {
  const v = record(record(value).job);
  if (
    !["queued", "running", "succeeded", "failed", "cancelled"].includes(
      String(v.state),
    ) ||
    count(v.sequence) < 1
  )
    throw new Error("Invalid generation response");
  return {
    job: {
      id: generationId(v.id),
      sequence: count(v.sequence),
      state: v.state as GenerationView["job"]["state"],
    },
  };
}
export function generationDto(value: unknown): GenerationView {
  const d = record(value),
    p = record(d.progress);
  const job = generationAcceptedDto(d).job;
  const n = count(d.count);
  const day = date(d.date),
    profile = generationId(d.profile),
    locale = text(d.locale, 35);
  const slot = (value: unknown): CandidateSlot => {
    const c = record(value),
      status = text(c.status, 16);
    if (!SLOT_STATES.includes(status))
      throw new Error("Invalid candidate status");
    return {
      index: count(c.index),
      id: c.id === null ? null : generationId(c.id),
      status: status as CandidateSlot["status"],
      error_code: generationCode(c.error_code),
    };
  };
  const candidates = list(d.candidates, 5).map(slot);
  const fields = [
    "requested_count",
    "finished_count",
    "ready_count",
    "failed_count",
    "cancelled_count",
    "not_started_count",
  ] as const;
  for (const k of fields)
    if (count(p[k]) > 5) throw new Error("Invalid generation count");
  const progress: GenerationView["progress"] = {
    kind: "generation",
    job_id: generationId(p.job_id),
    request_id: generationId(p.request_id),
    instance_id: generationId(p.instance_id),
    session_id: generationId(p.session_id),
    origin_date: date(p.origin_date),
    profile_id: generationId(p.profile_id),
    locale: text(p.locale, 35),
    baseline_revision: maybeRevision(p.baseline_revision),
    candidate_index:
      p.candidate_index === null ? null : count(p.candidate_index),
    requested_count: count(p.requested_count),
    finished_count: count(p.finished_count),
    ready_count: count(p.ready_count),
    failed_count: count(p.failed_count),
    cancelled_count: count(p.cancelled_count),
    not_started_count: count(p.not_started_count),
    phase: p.phase === null ? null : text(p.phase, 32),
    elapsed_ms: count(p.elapsed_ms),
    ...(p.silence_ms === undefined ? {} : { silence_ms: count(p.silence_ms) }),
    last_agent_activity_at:
      p.last_agent_activity_at === null
        ? null
        : count(p.last_agent_activity_at),
    cancellation_requested: p.cancellation_requested as boolean,
    candidate_ids: list(p.candidate_ids, 5).map(generationId),
    error_code: generationCode(p.error_code),
  };
  if (
    n < 1 ||
    n > 5 ||
    candidates.length !== n ||
    candidates.some(
      (s, i) =>
        s.index !== i + 1 ||
        (s.status === "ready" ? s.id !== `${job.id}-${i + 1}` : s.id !== null),
    ) ||
    typeof d.stale_baseline !== "boolean" ||
    typeof p.cancellation_requested !== "boolean" ||
    p.kind !== "generation" ||
    progress.job_id !== job.id ||
    progress.origin_date !== day ||
    progress.profile_id !== profile ||
    progress.locale !== locale ||
    progress.requested_count !== n ||
    progress.finished_count !==
      progress.ready_count + progress.failed_count + progress.cancelled_count ||
    progress.finished_count + progress.not_started_count > n ||
    (progress.candidate_index !== null &&
      (progress.candidate_index < 1 || progress.candidate_index > n)) ||
    (progress.phase !== null && !CHAT_STAGES.includes(progress.phase)) ||
    ["ready", "failed", "cancelled", "not_started"].some(
      (status) =>
        candidates.filter((s) => s.status === status).length !==
        p[`${status}_count`],
    ) ||
    JSON.stringify(progress.candidate_ids) !==
      JSON.stringify(candidates.flatMap((s) => (s.id ? [s.id] : [])))
  )
    throw new Error("Invalid generation response");
  return {
    job,
    date: day,
    profile,
    locale,
    count: n,
    stale_baseline: d.stale_baseline,
    candidates,
    progress,
    provisional:
      d.provisional === null ? null : generationText(d.provisional, MAX_REPLY),
  };
}
export function candidateDto(value: unknown): CandidateView {
  const d = record(value),
    id = generationId(d.id),
    job = generationId(d.job_id),
    index = count(d.index);
  if (index < 1 || index > 5 || id !== `${job}-${index}`)
    throw new Error("Invalid candidate response");
  const claim_support = list(d.claim_support, 2000).map((value) => {
    const c = record(value);
    const ids = list(c.snippet_ids, 2048).map((value) => text(value, 32));
    if (!ids.length || ids.some((id) => !/^snippet-[1-9][0-9]*$/u.test(id)))
      throw new Error("Invalid candidate support");
    return { claim: generationText(c.claim, 2048), snippet_ids: ids };
  });
  const candidate = {
    id,
    job_id: job,
    index,
    date: date(d.date),
    draft_markdown: generationText(d.draft_markdown, MAX_REPLY),
    claim_support,
    warnings: list(d.warnings, 100).map((w) => generationText(w, 2048)),
  };
  if (
    new TextEncoder().encode(JSON.stringify(candidate)).length >
    8 * 1024 * 1024
  )
    throw new Error("Invalid candidate response");
  return candidate;
}

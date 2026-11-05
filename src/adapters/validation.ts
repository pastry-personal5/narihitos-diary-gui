import {
  AuthenticError,
  type AppMetadata,
  type SymbolResult,
  type AuthenticFile,
  type AuthenticPage,
  type Availability,
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
  return {
    operation_id: text(dto.operation_id, 64),
    state: text(dto.state, 32),
    terminal: dto.terminal,
    succeeded: dto.succeeded,
    result: dto.result == null ? null : text(dto.result, 256),
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
  if (
    typeof dto.read_only !== "boolean" ||
    typeof dto.recoverable !== "boolean" ||
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

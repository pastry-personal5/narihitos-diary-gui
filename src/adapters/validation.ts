import type { AppMetadata, Status } from "../application/bridge";
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

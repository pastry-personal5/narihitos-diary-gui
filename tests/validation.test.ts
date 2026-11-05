import { describe, expect, it } from "vitest";
import { metadataDto, statusDto } from "../src/adapters/validation";
const status = {
  instance_id: "instance",
  workspace_key: "workspace",
  server_version: "1.0.0",
  api_version: 1,
  api_minor: 0,
  storage_ready: false,
  capabilities: ["status", "sessions", "jobs", "events"],
};
describe("redacted DTO boundary", () => {
  it("rejects malformed, incompatible and unknown required variants", () => {
    for (const value of [
      null,
      [],
      {},
      { ...status, api_version: 2 },
      { ...status, api_minor: -1 },
      { ...status, storage_ready: "yes" },
      { ...status, capabilities: ["status"] },
      { ...status, capabilities: Array(65).fill("status") },
    ])
      expect(() => statusDto(value)).toThrow();
  });
  it("drops fields outside the allowlist", () => {
    expect(
      statusDto({
        ...status,
        token: "invented",
        profile_home: "/private/invented",
      }),
    ).toEqual(status);
  });
  it("validates app-derived version metadata", () => {
    expect(metadataDto({ version: "1.0.0", extra: true })).toEqual({
      version: "1.0.0",
    });
    expect(() => metadataDto({ version: "unknown" })).toThrow();
  });
});

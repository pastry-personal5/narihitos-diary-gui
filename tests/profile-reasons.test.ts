import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { PROFILE_STATUS_REASONS, profileDto } from "../src/adapters/validation";
import { PROFILE_STATUS_REASON_TEXT } from "../src/components/useProfiles";

// The Rust test `api_visible_reasons_match_the_shared_list` ties this file to the server's set.
const shared: string[] = JSON.parse(
  readFileSync("src/application/profile-reasons.json", "utf8"),
);
const cap = { available: true, reason: null };
const dto = (reason: unknown) => ({
  alias: "a",
  type: "apple-foundation-models",
  status: "unavailable",
  reason,
  capabilities: { chat: cap, generate: cap },
});

describe("profile status reasons", () => {
  it("validator, wording and the shared list name the same reasons", () => {
    expect([...PROFILE_STATUS_REASONS]).toEqual(shared);
    expect(Object.keys(PROFILE_STATUS_REASON_TEXT)).toEqual(shared);
  });
  it("has distinct, non-empty wording for every reason", () => {
    const texts = Object.values(PROFILE_STATUS_REASON_TEXT);
    expect(texts.every((t) => t.length > 0)).toBe(true);
    expect(new Set(texts).size).toBe(texts.length);
    expect(PROFILE_STATUS_REASON_TEXT.apple_intelligence_disabled).toBe(
      "Apple Intelligence is turned off in System Settings",
    );
  });
  it("accepts every reason, null and absent, and rejects anything else", () => {
    for (const r of shared) expect(profileDto(dto(r)).reason).toBe(r);
    expect(profileDto(dto(null)).reason).toBeNull();
    expect(profileDto(dto(undefined)).reason).toBeNull();
    for (const bad of ["escaped_descendants", "/Users/x/secret", 3, ""])
      expect(() => profileDto(dto(bad))).toThrow();
  });
});

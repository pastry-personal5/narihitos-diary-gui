import { describe, expect, it } from "vitest";
import { sourceImportAdvice } from "../src/application/sourceAdvice";

describe("source import advice", () => {
  it("explains how to add a source when none is configured", () => {
    const text = sourceImportAdvice([]);
    expect(text).toContain("Source import is off");
    expect(text).toContain("ayumi or narihito");
    expect(text).toContain("Reload configuration");
  });

  it("names each unusable source with a plain reason", () => {
    const text = sourceImportAdvice([
      { alias: "old", available: false, reason: "source_root_missing" },
      { alias: "locked", available: false, reason: "source_root_denied" },
    ]);
    expect(text).toContain("Source import is off");
    expect(text).toContain("old: folder not found");
    expect(text).toContain("locked: folder not readable");
    expect(text).toContain("Reload configuration");
  });

  it("is silent when any source is available", () => {
    expect(
      sourceImportAdvice([
        { alias: "old", available: false, reason: "source_root_missing" },
        { alias: "ayumi", available: true, reason: "ready" },
      ]),
    ).toBeNull();
  });

  it("never echoes a reason code it does not know", () => {
    const text = sourceImportAdvice([
      { alias: "odd", available: false, reason: "token=SECRET /private/path" },
    ]);
    expect(text).toContain("odd: unavailable");
    expect(text).not.toMatch(/SECRET|\/private/);
  });
});

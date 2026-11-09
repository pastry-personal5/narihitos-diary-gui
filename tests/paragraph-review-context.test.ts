import { describe, expect, it } from "vitest";
import { snippetCreationTime } from "../src/application/paragraph-review/context";
import type { SourceDating } from "../src/application/navigation/contracts";

const header = {
  unix_ms: Date.parse("2026-11-08T23:30:00.123Z"),
  precision: "milli" as const,
};
const local = {
  date: "2026-11-09",
  datetime: "2026-11-09T08:30:00.123",
  offset_seconds: 32400,
};

describe("paragraph snippet creation evidence", () => {
  it("preserves the header instant, precision and diary-local day across UTC midnight", () => {
    expect(
      snippetCreationTime({
        dating: { state: "agreed", header, filename: header },
        navigation: {
          group_id: "snippets",
          header: local,
          filename: local,
          included: local,
          authority: "header",
        },
      }),
    ).toEqual({
      utc: "2026-11-08T23:30:00.123Z",
      precision: "milli",
      diary_local: {
        datetime: local.datetime,
        offset_seconds: 32400,
      },
      conflicts_with_filename: false,
    });
  });

  it("flags disagreement without substituting a chosen filename timestamp", () => {
    expect(
      snippetCreationTime({
        dating: {
          state: "timestamp_conflict",
          header: {
            ...header,
            unix_ms: header.unix_ms - 123,
            precision: "second",
          },
          filename: { ...header, unix_ms: header.unix_ms + 86400000 },
        },
        navigation: null,
      }),
    ).toEqual({
      utc: "2026-11-08T23:30:00.000Z",
      precision: "second",
      diary_local: null,
      conflicts_with_filename: true,
    });
  });

  it.each<SourceDating>([
    { state: "filename_only", filename: header },
    { state: "undated" },
  ])("omits creation evidence for $state", (dating) => {
    expect(snippetCreationTime({ dating, navigation: null })).toBeNull();
  });

  it("retains header-only evidence when no local projection exists", () => {
    expect(
      snippetCreationTime({
        dating: { state: "header_only", header },
        navigation: null,
      }),
    ).toMatchObject({
      utc: "2026-11-08T23:30:00.123Z",
      diary_local: null,
      conflicts_with_filename: false,
    });
  });
});

import { describe, expect, it } from "vitest";
import { snippetLabels } from "../src/application/navigation/snippet-label";
import type { SourceItem } from "../src/application/navigation/contracts";

const item = (id: string, datetime: string | null) =>
  ({
    kind: "warning",
    misplaced: false,
    source: {
      id,
      navigation: datetime
        ? {
            included: {
              date: datetime.slice(0, 10),
              datetime,
              offset_seconds: 0,
            },
            header: null,
            filename: null,
          }
        : null,
    },
  }) as unknown as SourceItem;

describe("snippetLabels", () => {
  it("uses minutes, widening only on collision", () => {
    const l = snippetLabels([
      item("a", "2026-11-02T07:47:32"),
      item("b", "2026-11-02T08:10:05"),
      item("c", "2026-11-02T08:10:40"),
      item("d", "2026-11-02T09:00:01.250"),
      item("e", "2026-11-02T09:00:01.750"),
      item("f", "2026-11-03T07:47:59"),
      item("g", null),
      item("h", "2026-11-04T10:00:00"),
      item("i", "2026-11-04T10:00:00"),
    ]);
    expect(l.get("a")).toBe("07:47");
    expect(l.get("b")).toBe("08:10:05");
    expect(l.get("c")).toBe("08:10:40");
    expect(l.get("d")).toBe("09:00:01.250");
    expect(l.get("e")).toBe("09:00:01.750");
    expect(l.get("f")).toBe("07:47");
    expect(l.has("g")).toBe(false);
    expect(l.has("h")).toBe(false);
    expect(l.has("i")).toBe(false);
  });
});

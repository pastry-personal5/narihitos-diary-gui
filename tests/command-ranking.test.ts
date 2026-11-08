import { describe, expect, it } from "vitest";
import { rankCommands } from "../src/application/actions/rank";
const entry = (label: string, reason: string | null = null, id = label) => ({
  id,
  label,
  reason,
});
const labels = (entries: ReturnType<typeof entry>[], query: string) =>
  rankCommands(entries, query).map((e) => e.label);
describe("command recommendations", () => {
  it("orders exact, prefix, ordered word prefixes and substring matches", () => {
    const entries = ["Autosave", "Open Save", "Save source", "Save"].map((s) =>
      entry(s),
    );
    expect(labels(entries, "save")).toEqual([
      "Save",
      "Save source",
      "Open Save",
      "Autosave",
    ]);
    entries[3].reason = "Unavailable";
    expect(labels(entries, "save")[0]).toBe("Save");
    expect(
      labels(
        [entry("Open recent AI activity"), entry("Open AI activity")],
        "open ai",
      ),
    ).toEqual(["Open AI activity", "Open recent AI activity"]);
  });
  it("normalizes whitespace, case and NFC; tokens use distinct ordered words", () => {
    expect(labels([entry("Open Settings")], "  OPEN   SET  ")).toEqual([
      "Open Settings",
    ]);
    expect(labels([entry("Café")], "Cafe\u0301")).toEqual(["Café"]);
    expect(labels([entry("Save")], "sa sa")).toEqual([]);
    expect(labels([entry("Open Settings")], "set open")).toEqual([]);
  });
  it("sorts empty searches by availability, code points and stable ID", () => {
    const entries = [
      entry("Export", "Connect"),
      entry("Save", null, "b"),
      entry("Save", null, "a"),
    ];
    expect(rankCommands(entries, "  ").map((e) => e.id)).toEqual([
      "a",
      "b",
      "Export",
    ]);
    expect(rankCommands([...entries].reverse(), "").map((e) => e.id)).toEqual([
      "a",
      "b",
      "Export",
    ]);
    expect(labels(entries, "zzzz")).toEqual([]);
  });
});

import type { Source, SourceItem } from "./contracts";

const TIME = /^(\d{4}-\d\d-\d\d)T(\d\d:\d\d)(:\d\d)?(\.\d+)?/;
const timeOf = (s: Source) =>
  (s.navigation?.included ?? s.navigation?.header ?? s.navigation?.filename)
    ?.datetime;

/** Time-only labels (under the date heading): minutes, widened to seconds then ms only on collision. */
export function snippetLabels(items: SourceItem[]): Map<string, string> {
  const parsed: [string, RegExpMatchArray][] = [];
  for (const item of items) {
    const source = "source" in item ? item.source : null;
    const m = source && timeOf(source)?.match(TIME);
    if (source && m) parsed.push([source.id, m]);
  }
  const count = (key: (m: RegExpMatchArray) => string) => {
    const c = new Map<string, number>();
    for (const [, m] of parsed) c.set(key(m), (c.get(key(m)) ?? 0) + 1);
    return c;
  };
  const min = count((m) => m[1] + m[2]);
  const sec = count((m) => m[1] + m[2] + (m[3] ?? ""));
  const out = new Map<string, string>();
  for (const [id, m] of parsed) {
    let label = m[2];
    if (min.get(m[1] + m[2])! > 1) {
      label += m[3] ?? "";
      if (sec.get(m[1] + m[2] + (m[3] ?? ""))! > 1) label += m[4] ?? "";
    }
    out.set(id, label);
  }
  // Still identical at the finest precision on one day: the filename is the only way to tell them apart.
  const seen = new Map<string, number>();
  const dates = new Map(parsed.map(([id, m]) => [id, m[1]]));
  const key = (id: string) => dates.get(id) + out.get(id)!;
  for (const id of out.keys()) seen.set(key(id), (seen.get(key(id)) ?? 0) + 1);
  for (const id of [...out.keys()]) if (seen.get(key(id))! > 1) out.delete(id);
  return out;
}

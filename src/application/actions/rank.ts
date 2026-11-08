/** Local deterministic recommendations; scores are never presentation data. */
const normalize = (text: string) =>
  text.normalize("NFC").toLowerCase().trim().replace(/\s+/gu, " ");
const compare = (a: string, b: string) => (a < b ? -1 : a > b ? 1 : 0);
export function rankCommands<
  T extends { id: string; label: string; reason: string | null },
>(entries: T[], query: string): T[] {
  const needle = normalize(query);
  return entries
    .map((entry) => {
      const label = normalize(entry.label);
      let tier = 0;
      if (needle) {
        const words = label.split(" ");
        let position = 0;
        const ordered = needle.split(" ").every((token) => {
          while (position < words.length && !words[position].startsWith(token))
            position++;
          return position++ < words.length;
        });
        tier =
          label === needle
            ? 0
            : label.startsWith(needle)
              ? 1
              : ordered
                ? 2
                : label.includes(needle)
                  ? 3
                  : 4;
      }
      return { entry, label, tier };
    })
    .filter(({ tier }) => tier < 4)
    .sort(
      (a, b) =>
        a.tier - b.tier ||
        Number(a.entry.reason !== null) - Number(b.entry.reason !== null) ||
        compare(a.label, b.label) ||
        compare(a.entry.id, b.entry.id),
    )
    .map(({ entry }) => entry);
}

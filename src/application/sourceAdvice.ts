import type { Availability } from "./bridge";

/** Plain wording for the redacted reason codes; any other code is never echoed. */
const WHY: Record<string, string> = {
  source_root_missing: "folder not found",
  source_root_denied: "folder not readable",
};
const HOW =
  "Add or fix a source in the server's private config file (type ayumi or narihito, with the absolute path of a readable folder), then choose Reload configuration in Settings.";

/** The Global Messages text for a server where no source can be imported, else null. */
export function sourceImportAdvice(sources: Availability[]): string | null {
  if (sources.some((source) => source.available)) return null;
  if (sources.length === 0)
    return `Source import is off: no source is configured. ${HOW}`;
  const list = sources
    .map((source) => `${source.alias}: ${WHY[source.reason] ?? "unavailable"}`)
    .join("; ");
  return `Source import is off: no source folder is usable (${list}). ${HOW}`;
}

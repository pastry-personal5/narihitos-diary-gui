import type { Source } from "../navigation/contracts";
import type { ParagraphReviewSnippet } from "./contracts";

/** Header created_at evidence only; filename and filesystem times are not substitutes. */
export function snippetCreationTime(
  source: Pick<Source, "dating" | "navigation">,
): ParagraphReviewSnippet["creation_time"] {
  if (!("header" in source.dating)) return null;
  const { header } = source.dating;
  const timestamp = new Date(header.unix_ms);
  if (!Number.isFinite(timestamp.getTime())) return null;
  const local = source.navigation?.header;
  return {
    utc: timestamp.toISOString(),
    precision: header.precision,
    diary_local: local
      ? { datetime: local.datetime, offset_seconds: local.offset_seconds }
      : null,
    conflicts_with_filename: source.dating.state === "timestamp_conflict",
  };
}

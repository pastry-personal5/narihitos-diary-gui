const TRASH_ERROR: Record<string, string> = {
  preview_expired: "This preview has expired. Refresh it and confirm again.",
  preview_stale:
    "The connection or configuration changed. Refresh the preview and confirm again.",
  member_changed:
    "A file changed after preview. Refresh the preview before deciding again.",
  ownership_ambiguous:
    "Related audio ownership could not be verified. No further file will move until it can be checked safely.",
  source_stale:
    "The original source or its root is unavailable or has changed. Refresh snippets to check its state.",
  cloud_unavailable:
    "A required file is unavailable. Make it available locally, then refresh the preview.",
  trash_unsupported: "Move to Trash is unavailable for this source.",
  resource_reserved:
    "An unfinished operation reserves this source. Inspect its saved result before retrying.",
  checkpoint_mismatch:
    "The saved operation advanced. Check its current result before preparing another retry.",
  outcome_uncertain:
    "The outcome is uncertain. Automatic retries are disabled.",
  session_limit:
    "The session limit is reached. Close completed results or unused tabs before trying again.",
  limit:
    "The server is busy or preparation exceeded its limit. Try a new preview later.",
};
export const trashError = (code: string) =>
  TRASH_ERROR[code] ??
  "The result could not be confirmed. Check the saved operation when connected.";

import type { ProfileInfo } from "../bridge";

/** Plain wording for the statuses; "unchecked" is the state before any explicit check. */
export const PROFILE_STATUS: Record<ProfileInfo["status"], string> = {
  unchecked: "not checked yet",
  missing: "command not found",
  unsupported_version: "unsupported version",
  unavailable: "unavailable",
  available_auth_unknown: "installed, sign-in unknown",
  unauthenticated: "not signed in",
  ready: "ready",
  busy: "busy",
  failed: "check failed",
};
export const PROFILE_REASON: Record<string, string> = {
  assets_missing: "prompt assets missing",
  assets_invalid: "prompt assets invalid",
  adapter_unimplemented: "no adapter yet",
};
export const PROFILE_STATUS_REASON_TEXT: Record<string, string> = {
  executable_missing: "command not found",
  profile_home_missing: "profile folder not found",
  profile_home_not_directory: "profile folder is not a folder",
  profile_unknown: "profile is no longer configured",
  kind_mismatch: "profile is configured for another agent",
  version_unsupported: "installed version is not supported",
  version_unreadable: "version could not be read",
  probe_timed_out: "the check timed out",
  probe_failed: "the check failed",
  spawn_failed: "the program could not be started",
  host_busy: "another agent task is running",
  run_dir_unavailable: "working folder could not be created",
  assets_missing: "prompt files are missing",
  assets_invalid: "prompt files are invalid",
  prompt_pin_rejected: "prompt model setting is not accepted",
  packet_too_large: "the request is too large",
  os_unsupported: "Apple Intelligence needs macOS 26 or later",
  device_ineligible: "this Mac does not support Apple Intelligence",
  apple_intelligence_disabled:
    "Apple Intelligence is turned off in System Settings",
  model_not_ready:
    "the Apple Intelligence model is still downloading or not ready",
  model_unavailable_unknown: "the Apple Intelligence model is unavailable",
  locale_unsupported: "Apple Intelligence does not support this language",
  helper_missing: "the Apple helper was not built into this app",
  helper_untrusted: "the Apple helper failed its integrity check",
  helper_failed: "the Apple helper could not be set up or did not answer",
};
export const PROFILE_ERROR = "The profile status could not be read.";

import { useCallback, useEffect, useRef, useState } from "react";
import {
  AuthenticError,
  type Bridge,
  type ProfileInfo,
} from "../application/bridge";

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
/** Distinct wording for a status reason; falls back to nothing for an unknown id. */
export const PROFILE_ERROR = "The profile status could not be read.";
const CHECK_ERROR = "The check could not be completed.";

/** Read-only profile status for Settings. It never starts a run and never blocks Settings. */
export function useProfiles(
  bridge: Bridge,
  connected: boolean,
  configGeneration?: number,
) {
  const [profiles, setProfiles] = useState<ProfileInfo[]>();
  const [checking, setChecking] = useState<string[]>([]);
  const [error, setError] = useState<string>();
  const alive = useRef(true);
  const supported = bridge.listProfiles !== undefined;

  useEffect(() => {
    alive.current = true;
    if (!connected || !bridge.listProfiles) return;
    // Per-effect flag: a slow list from before a reload must not overwrite the newer one.
    let current = true;
    bridge.listProfiles().then(
      (next) => {
        if (!current) return;
        setProfiles(next);
        setError(undefined);
      },
      () => current && setError(PROFILE_ERROR),
    );
    return () => {
      current = false;
      alive.current = false;
    };
    // A reload that changes profiles bumps the generation; refetch so the list is not stale.
  }, [bridge, connected, configGeneration]);

  const check = useCallback(
    async (alias: string) => {
      if (!bridge.checkProfile) return;
      setChecking((c) => [...c, alias]);
      try {
        const next = await bridge.checkProfile(alias);
        if (!alive.current) return;
        setProfiles((all) => all?.map((p) => (p.alias === alias ? next : p)));
        setError(undefined);
      } catch (e) {
        // A removed alias (reload) is a stale list, not a failed check.
        if (alive.current)
          setError(
            e instanceof AuthenticError && e.code === "not_found"
              ? PROFILE_ERROR
              : CHECK_ERROR,
          );
      } finally {
        if (alive.current) setChecking((c) => c.filter((a) => a !== alias));
      }
    },
    [bridge],
  );
  return {
    supported,
    profiles: connected ? profiles : undefined,
    checking,
    error,
    check,
  };
}

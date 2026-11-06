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

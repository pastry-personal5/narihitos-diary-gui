import type { ProfileStatusModel } from "../../application/ai-agents/contracts";
import { useCallback, useEffect, useRef, useState } from "react";
import {
  AuthenticError,
  type Bridge,
  type ProfileInfo,
} from "../../application/bridge";

import { PROFILE_ERROR } from "../../application/ai-agents/presentation";
export const CHECK_BASE_MS = 15_000;
export const CHECK_MAX_MS = 3_600_000;
/** Wait before background check number n+1: 15 s, 30 s, 60 s ... capped at one hour. */
export const checkDelay = (n: number) =>
  Math.min(CHECK_BASE_MS * 2 ** n, CHECK_MAX_MS);
const CHECK_ERROR = "The check could not be completed.";

/** Read-only profile status for Settings. It never starts a run and never blocks Settings. */
export function useProfiles(
  bridge: Bridge,
  connected: boolean,
  configGeneration?: number,
): ProfileStatusModel {
  const [profiles, setProfiles] = useState<ProfileInfo[]>();
  const [checking, setChecking] = useState<string[]>([]);
  const [error, setError] = useState<string>();
  const alive = useRef(true);
  const inFlight = useRef(new Set<string>());
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
      // One probe per alias at a time: background ticks and manual checks must not overlap.
      if (!bridge.checkProfile || inFlight.current.has(alias)) return;
      inFlight.current.add(alias);
      setChecking((c) => [...c, alias]);
      try {
        const next = await bridge.checkProfile(alias);
        if (!alive.current) return;
        setProfiles((all) => all?.map((p) => (p.alias === alias ? next : p)));
        setError(undefined);
        return next.status;
      } catch (e) {
        // A removed alias (reload) is a stale list, not a failed check.
        if (alive.current)
          setError(
            e instanceof AuthenticError && e.code === "not_found"
              ? PROFILE_ERROR
              : CHECK_ERROR,
          );
      } finally {
        inFlight.current.delete(alias);
        if (alive.current) setChecking((c) => c.filter((a) => a !== alias));
      }
    },
    [bridge],
  );
  // Background checks: one at launch, then 15 s, 30 s, 60 s ... capped at 1 h, per profile.
  const aliasKey = profiles?.map((p) => p.alias).join("\n");
  useEffect(() => {
    if (!connected || !bridge.checkProfile || !aliasKey) return;
    const timers: ReturnType<typeof setTimeout>[] = [];
    let live = true;
    aliasKey.split("\n").forEach((alias, i) => {
      let n = 0;
      const tick = async () => {
        const status = await check(alias);
        if (live)
          timers[i] = setTimeout(
            tick,
            status === "busy" ? CHECK_BASE_MS : checkDelay(n++),
          );
      };
      void tick();
    });
    return () => {
      live = false;
      timers.forEach(clearTimeout);
    };
    // Not keyed on configGeneration: a reload refetches the list and aliasKey follows it.
  }, [bridge, connected, aliasKey, check]);
  return {
    supported,
    profiles: connected ? profiles : undefined,
    checking,
    error,
    check: async (alias: string) => {
      await check(alias);
    },
  };
}

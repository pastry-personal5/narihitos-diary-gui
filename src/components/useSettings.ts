import { useCallback, useEffect, useRef, useState } from "react";
import {
  SettingsError,
  type Bridge,
  type ReloadResult,
  type ServerSettings,
  type UserSettings,
} from "../application/bridge";

/** Plain messages for the stable codes the bridge may return. */
export const SETTINGS_MESSAGE: Record<string, string> = {
  conflict: "Settings changed elsewhere. They were reloaded; try again.",
  read_only: "These settings come from a newer app version and are read-only.",
  file_invalid: "The settings file is invalid. Recover it to continue.",
  invalid_input: "That value is not valid.",
  language_unresolved:
    "The system language could not be resolved. Choose a Writing Language.",
  uncertain: "The save may not have completed. Settings were reloaded.",
  unavailable: "Settings are unavailable.",
  permission_denied: "Settings are not available to this window.",
  invalid_response: "Settings returned an unexpected response.",
};
export const RELOAD_MESSAGE: Record<ReloadResult["outcome"], string> = {
  applied: "Configuration reloaded.",
  restart_required:
    "Some changes need a restart of the server and were not applied.",
  rejected: "The configuration was rejected; the previous one stays in use.",
  busy: "An operation is using the changed profile or export; try again later.",
};
const code = (e: unknown) =>
  e instanceof SettingsError ? e.code : "unavailable";

export function useSettings(bridge: Bridge, connected: boolean) {
  const [user, setUser] = useState<UserSettings>();
  const [server, setServer] = useState<ServerSettings>();
  const [error, setError] = useState<string>();
  const [serverError, setServerError] = useState<string>();
  const [reloading, setReloading] = useState(false);
  const alive = useRef(true);
  const revision = useRef<string | undefined>(undefined);

  const adopt = useCallback((next: UserSettings) => {
    revision.current = next.revision;
    setUser(next);
  }, []);
  const update = useCallback(
    async (
      section: "writing" | "writing_init" | "keybindings",
      value: unknown,
    ) => {
      if (revision.current === undefined) return false;
      try {
        const next = await bridge.updateUserSettings({
          section,
          value,
          expectedRevision: revision.current,
          recover: false,
        });
        if (!alive.current) return false;
        adopt(next);
        setError(undefined);
        return true;
      } catch (e) {
        if (!alive.current) return false;
        setError(code(e));
        // A stale or uncertain write is resolved by reloading what is on disk.
        if (["conflict", "uncertain"].includes(code(e)))
          await bridge.getUserSettings().then(adopt, () => {});
        return false;
      }
    },
    [adopt, bridge],
  );

  useEffect(() => {
    alive.current = true;
    // Per-run flag: the shared ref is reset by a StrictMode remount, which would let two loads race.
    let live = true;
    void (async () => {
      let loaded: UserSettings;
      try {
        loaded = await bridge.getUserSettings();
      } catch (e) {
        if (live) setError(code(e));
        return;
      }
      if (!live) return;
      adopt(loaded);
      const blocked =
        loaded.read_only ||
        loaded.recoverable ||
        loaded.warnings.some((w) => w.section === "writing");
      if (loaded.writing_locale === null && !blocked)
        await update("writing_init", [...(navigator.languages ?? [])]);
    })();
    return () => {
      live = false;
      alive.current = false;
    };
  }, [adopt, bridge, update]);

  const refreshServer = useCallback(async () => {
    try {
      const next = await bridge.serverSettings();
      if (alive.current) {
        setServer(next);
        setServerError(undefined);
      }
    } catch (e) {
      if (alive.current) setServerError(code(e));
    }
  }, [bridge]);
  useEffect(() => {
    if (!connected) return;
    let live = true;
    bridge.serverSettings().then(
      (next) => {
        if (!live) return;
        setServer(next);
        setServerError(undefined);
      },
      (e) => {
        if (live) setServerError(code(e));
      },
    );
    return () => {
      live = false;
    };
  }, [connected, bridge]);

  return {
    user,
    server: connected ? server : undefined,
    error,
    serverError,
    reloading,
    setWriting: (tag: string) => update("writing", tag),
    setKeybindings: (value: unknown) => update("keybindings", value),
    recover: async () => {
      if (revision.current === undefined) return;
      try {
        adopt(
          await bridge.updateUserSettings({
            section: "writing",
            value: null,
            expectedRevision: revision.current,
            recover: true,
          }),
        );
        setError(undefined);
      } catch (e) {
        setError(code(e));
      }
    },
    reload: async () => {
      setReloading(true);
      let failed: string | undefined;
      try {
        await bridge.reloadSettings();
      } catch (e) {
        failed = code(e);
      }
      await refreshServer();
      if (!alive.current) return;
      // Set after the refresh, which clears serverError on success.
      if (failed) setServerError(failed);
      setReloading(false);
    },
  };
}

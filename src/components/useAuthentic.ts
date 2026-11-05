import { useCallback, useEffect, useRef, useState } from "react";
import type { Bridge } from "../application/bridge";
import type { StreamEvent } from "../application/events";
import {
  AuthenticSession,
  type SessionView,
} from "../application/authentic/session";
import {
  mountAuthenticEditor,
  type AuthenticEditor,
} from "../editor/authentic-editor";

export function today(now = new Date()): string {
  const p = (n: number) => String(n).padStart(2, "0");
  return `${now.getFullYear()}-${p(now.getMonth() + 1)}-${p(now.getDate())}`;
}
export const SAVE_LABEL = {
  clean: "Saved",
  dirty: "Unsaved changes",
  saving: "Saving…",
  save_failed: "Save failed",
  external_conflict: "Changed on disk",
} as const;

/** Owns the single Authentic editor, the session of the selected date and its transitions. */
export function useAuthentic(
  bridge: Bridge,
  connected: boolean,
  writable: boolean,
) {
  const [host, hostRef] = useState<HTMLDivElement | null>(null);
  const editor = useRef<AuthenticEditor | null>(null);
  const session = useRef<AuthenticSession | null>(null);
  const changing = useRef(false);
  const [date, setDate] = useState(() => today());
  const [view, setView] = useState<SessionView | null>(null);
  const [dates, setDates] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const refreshDates = useCallback(async () => {
    try {
      const found: string[] = [];
      let cursor: string | undefined;
      for (let page = 0; page < 5; page++) {
        const result = await bridge.listAuthentic(cursor);
        found.push(...result.items.map((item) => item.date));
        if (!result.next_cursor) break;
        cursor = result.next_cursor;
      }
      setDates(found);
    } catch {
      /* The list is a convenience; loading and saving report their own errors. */
    }
  }, [bridge]);

  // One editor for the component's lifetime; Draft and Entry both show it.
  useEffect(() => {
    if (!host) return;
    const mounted = mountAuthenticEditor(host, {
      onEdit: () => session.current?.dispatch({ kind: "edit" }),
      onCompositionStart: () =>
        session.current?.dispatch({ kind: "composition_start" }),
      onCompositionEnd: () =>
        session.current?.dispatch({ kind: "composition_end" }),
      onSaveShortcut: () => session.current?.dispatch({ kind: "flush" }),
    });
    editor.current = mounted;
    return () => {
      editor.current = null;
      mounted.destroy();
    };
  }, [host]);

  useEffect(() => {
    editor.current?.setEditable(writable && view !== null);
  }, [writable, view]);

  // Open the selected date once the server is reachable.
  useEffect(() => {
    if (!connected) return;
    let cancelled = false;
    const port = {
      getText: () => editor.current?.getText() ?? "",
      setText: (text: string) => editor.current?.setText(text),
    };
    void AuthenticSession.open(bridge, date, port)
      .then((opened) => {
        if (cancelled) return opened.dispose();
        session.current = opened;
        opened.subscribe(setView);
        setView(opened.view());
        setError(null);
        void refreshDates();
      })
      .catch((e: unknown) => {
        if (!cancelled)
          setError(
            e && typeof e === "object" && "code" in e
              ? String((e as { code: unknown }).code)
              : "unavailable",
          );
      });
    return () => {
      cancelled = true;
      session.current?.dispose();
      session.current = null;
      setView(null);
    };
  }, [bridge, date, connected, refreshDates]);

  // Window activation is a rescan trigger.
  useEffect(() => {
    const onFocus = () => session.current?.invalidated();
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, []);

  // Closing waits for pending saves and stays open when one failed.
  useEffect(() => {
    let off: (() => void) | undefined;
    let gone = false;
    void bridge
      .onCloseRequested?.(async () => {
        const s = session.current;
        if (!s) return true;
        if ((await s.settle()) === "proceed") return true;
        setNotice("Resolve the save problem before closing.");
        return false;
      })
      .then((dispose) => {
        if (gone) dispose();
        else off = dispose;
      });
    return () => {
      gone = true;
      off?.();
    };
  }, [bridge]);

  const changeDate = useCallback(
    async (next: string) => {
      if (next === date || changing.current) return;
      changing.current = true;
      try {
        const s = session.current;
        if (s && (await s.settle()) === "blocked") {
          setNotice("Resolve the save problem before changing the date.");
          return;
        }
        setNotice(null);
        setDate(next);
      } finally {
        changing.current = false;
      }
    },
    [date],
  );

  const createEmpty = useCallback(
    () => void session.current?.createEmpty().then(refreshDates),
    [refreshDates],
  );
  const dispatch = useCallback(
    (action: Parameters<AuthenticSession["dispatch"]>[0]) =>
      session.current?.dispatch(action),
    [],
  );

  const onEvent = useCallback(
    (event: StreamEvent) => {
      if (
        event.kind === "authentic_changed" ||
        event.kind === "resync_required"
      ) {
        session.current?.invalidated();
        void refreshDates();
      }
    },
    [refreshDates],
  );

  return {
    hostRef,
    date,
    dates,
    view,
    error,
    notice,
    changeDate,
    onEvent,
    createEmpty,
    dispatch,
  };
}

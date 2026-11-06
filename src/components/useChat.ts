import { useEffect, useState, useSyncExternalStore } from "react";
import type { ChatDocumentSnapshot } from "../application/bridge";
import { STAGE_TEXT, type ChatController } from "../application/chat";

export interface ChatContext {
  document: ChatDocumentSnapshot | null;
  /** What this message will carry, for example "Entry 2026-11-06, unsaved edits included". */
  chip: string;
}

/** One-line AI activity text for the toolbar; empty when nothing is running. */
export function activitySummary(c: ChatController, now: number): string {
  const { chat, startedAt } = c.state;
  if (!c.running) return "";
  const stage = chat?.turn?.stage ? STAGE_TEXT[chat.turn.stage] : "Submitting";
  const secs =
    startedAt === null ? 0 : Math.max(0, Math.floor((now - startedAt) / 1000));
  return `AI Chat: ${stage ?? "Working"} · ${secs}s`;
}

export function useChat(c: ChatController) {
  return useSyncExternalStore(c.subscribe, () => c.state);
}
/** Re-renders each second while a turn runs, so elapsed time stays truthful. */
export function useTick(active: boolean) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!active) return;
    const t = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(t);
  }, [active]);
  return now;
}

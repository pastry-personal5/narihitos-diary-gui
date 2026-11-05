import registry from "./registry.json";
import type { Scope } from "./shortcut";

export interface ActionInfo {
  id: string;
  label: string;
  info: string;
  scope: Scope;
  defaults: string[];
}
export const ACTIONS = registry.actions as ActionInfo[];
export const DEFAULT_BINDINGS: Record<string, string[]> = Object.fromEntries(
  ACTIONS.map((a) => [a.id, a.defaults]),
);
/** `reason` is why the action cannot run now (`null` when it can). */
export interface Handler {
  reason: string | null;
  run(): void | Promise<void>;
}
export type Handlers = Record<string, Handler>;

/** The action already using `shortcut` in a scope overlapping `id`'s (`app` overlaps all). */
export function conflictFor(
  id: string,
  shortcut: string,
  bindings: Record<string, string[]>,
): ActionInfo | undefined {
  const own = ACTIONS.find((a) => a.id === id);
  return ACTIONS.find(
    (a) =>
      a.id !== id &&
      (a.scope === own?.scope || a.scope === "app" || own?.scope === "app") &&
      bindings[a.id]?.includes(shortcut),
  );
}

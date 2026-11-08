import { ACTIONS, type Handlers } from "./actions";
import { composing, fromEvent, validate, type ShortcutEvent } from "./shortcut";

export interface DispatchState {
  bindings(): Record<string, string[]>;
  handlers(): Handlers;
  /** Capture mode or a modal dialog is open. */
  suspended(): boolean;
  /** Focus is on a snippet in the Explorer's Snippet tab. */
  inSnippetTree(): boolean;
  notice(text: string): void;
}
/** The window `keydown` policy: IME and suspension first, then shortcut + scope → action. */
export function createDispatcher(state: DispatchState) {
  return (e: ShortcutEvent) => {
    if (composing(e) || state.suspended()) return;
    const shortcut = fromEvent(e);
    if (!shortcut) return;
    const bindings = state.bindings();
    const tree = state.inSnippetTree();
    const action = ACTIONS.find(
      (a) => (a.scope === "app" || tree) && bindings[a.id]?.includes(shortcut),
    );
    if (!action || validate(action.scope, shortcut)) return;
    e.preventDefault();
    if (e.repeat) return; // holding a key must not re-run the action or repeat its notice
    const handler = state.handlers()[action.id];
    if (!handler) state.notice(`${action.label} is not available.`);
    else if (handler.reason) state.notice(handler.reason);
    else void handler.run();
  };
}

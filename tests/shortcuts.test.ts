// @vitest-environment jsdom
import { describe, expect, it, vi } from "vitest";
import registry from "../src/application/actions/registry.json";
import { ACTIONS, conflictFor } from "../src/application/actions/actions";
import {
  canonicalize,
  fromEvent,
  validate,
} from "../src/application/actions/shortcut";
import { createDispatcher } from "../src/application/actions/dispatcher";

describe("registry and canonical form", () => {
  it("registry_parity: 12 actions with the contract's scopes and defaults", () => {
    expect(ACTIONS.map((a) => [a.id, a.scope, a.defaults.length])).toEqual([
      ["palette.open", "app", 1],
      ["editor.save", "app", 1],
      ["authentic.create_empty", "app", 0],
      ["source.trash", "snippetTree", 4],
      ["source.refresh", "app", 0],
      ["generate.start", "app", 0],
      ["ai.activity.open", "app", 0],
      ["ai.run.cancel", "app", 0],
      ["ai.results.open", "app", 0],
      ["ai.results.discard", "app", 0],
      ["export.start", "app", 0],
      ["settings.open", "app", 0],
    ]);
    expect(ACTIONS[0].defaults).toEqual(["Shift+Command+P"]);
  });
  it("canonical_fixtures_match and factory defaults validate", () => {
    for (const [raw, want] of registry.fixtures.canonical)
      expect(canonicalize(raw)).toBe(want);
    for (const bad of registry.fixtures.invalid)
      expect(canonicalize(bad)).toBeNull();
    for (const [scope, s, want] of registry.fixtures.validate)
      expect(validate(scope as "app", s) ?? "ok").toBe(want);
    for (const a of ACTIONS)
      for (const d of a.defaults) expect(validate(a.scope, d)).toBeNull();
  });
  it("derives the key from code, not key", () => {
    const e = new KeyboardEvent("keydown", {
      code: "KeyP",
      key: "π",
      metaKey: true,
      shiftKey: true,
    });
    expect(fromEvent(e)).toBe("Shift+Command+P");
    expect(
      fromEvent(
        new KeyboardEvent("keydown", { code: "Delete", metaKey: true }),
      ),
    ).toBe("Command+ForwardDelete");
    expect(
      fromEvent(new KeyboardEvent("keydown", { code: "ShiftLeft" })),
    ).toBeNull();
  });
  it("finds conflicts across overlapping scopes only", () => {
    const b = { "editor.save": ["Command+S"], "source.trash": ["Command+J"] };
    expect(conflictFor("settings.open", "Command+S", b)?.id).toBe(
      "editor.save",
    );
    expect(conflictFor("settings.open", "Command+J", b)?.id).toBe(
      "source.trash",
    );
    expect(conflictFor("source.trash", "Command+S", b)?.id).toBe("editor.save");
    expect(conflictFor("editor.save", "Command+S", b)).toBeUndefined();
  });
});

function setup(over: { suspended?: boolean; tree?: boolean } = {}) {
  const run = vi.fn();
  const notice = vi.fn();
  const handlers = {
    "palette.open": { reason: null, run },
    "editor.save": { reason: "Source saving is not available yet.", run },
    "source.trash": { reason: null, run },
  };
  const dispatch = createDispatcher({
    bindings: () => ({
      "palette.open": ["Shift+Command+P"],
      "editor.save": ["Command+S"],
      "source.trash": ["Command+ForwardDelete"],
      "settings.open": ["Command+C"],
    }),
    handlers: () => handlers,
    suspended: () => !!over.suspended,
    inSnippetTree: () => !!over.tree,
    notice,
  });
  const key = (init: KeyboardEventInit) => {
    const e = new KeyboardEvent("keydown", { cancelable: true, ...init });
    dispatch(e);
    return e;
  };
  return { run, notice, key };
}
describe("dispatcher", () => {
  it("runs a matching app shortcut and prevents default", () => {
    const t = setup();
    const e = t.key({ code: "KeyP", metaKey: true, shiftKey: true });
    expect(t.run).toHaveBeenCalledOnce();
    expect(e.defaultPrevented).toBe(true);
  });
  it("ignores_composition_and_229", () => {
    const t = setup();
    t.key({ code: "KeyP", metaKey: true, shiftKey: true, isComposing: true });
    t.key({ code: "KeyP", metaKey: true, shiftKey: true, keyCode: 229 });
    expect(t.run).not.toHaveBeenCalled();
  });
  it("ignores_reserved even if a binding file listed one", () => {
    const t = setup();
    const e = t.key({ code: "KeyC", metaKey: true });
    expect(t.run).not.toHaveBeenCalled();
    expect(e.defaultPrevented).toBe(false);
  });
  it("snippet scope requires tree focus; editor delete is unaffected", () => {
    const outside = setup();
    const e = outside.key({ code: "Delete", metaKey: true });
    expect(outside.run).not.toHaveBeenCalled();
    expect(e.defaultPrevented).toBe(false);
    const inside = setup({ tree: true });
    inside.key({ code: "Delete", metaKey: true });
    expect(inside.run).toHaveBeenCalledOnce();
  });
  it("suspended_during_capture_and_dialogs", () => {
    const t = setup({ suspended: true });
    const e = t.key({ code: "KeyP", metaKey: true, shiftKey: true });
    expect(t.run).not.toHaveBeenCalled();
    expect(e.defaultPrevented).toBe(false);
  });
  it("disabled action is consumed and posts its reason; save never runs on a source", () => {
    const t = setup();
    const e = t.key({ code: "KeyS", metaKey: true });
    expect(e.defaultPrevented).toBe(true);
    expect(t.run).not.toHaveBeenCalled();
    expect(t.notice).toHaveBeenCalledWith(
      "Source saving is not available yet.",
    );
  });
});

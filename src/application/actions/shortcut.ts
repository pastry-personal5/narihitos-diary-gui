// Mirrors src-tauri/src/user_settings/keybindings.rs; shared fixtures in registry.json prove parity.
import registry from "./registry.json";

/** Structural input; browser keyboard events implement it without making policy DOM-dependent. */
export interface ShortcutEvent {
  code: string;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  metaKey: boolean;
  isComposing: boolean;
  keyCode: number;
  repeat: boolean;
  preventDefault(): void;
}
export type Scope = "app" | "snippetTree";
const MODS = ["Control", "Option", "Shift", "Command"];
const NAMED = [
  "Backspace",
  "ForwardDelete",
  "Enter",
  "Escape",
  "Tab",
  "Space",
  "ArrowUp",
  "ArrowDown",
  "ArrowLeft",
  "ArrowRight",
  "Home",
  "End",
  "PageUp",
  "PageDown",
];

function canonicalKey(k: string): string | null {
  const named = NAMED.find((n) => n.toLowerCase() === k.toLowerCase());
  if (named) return named;
  if (/^[a-z0-9]$/i.test(k)) return k.toUpperCase();
  const f = /^f(\d{1,2})$/i.exec(k);
  return f && +f[1] >= 1 && +f[1] <= 12 ? `F${+f[1]}` : null;
}
/** Modifiers in canonical order, then the key; `null` when malformed. */
export function canonicalize(s: string): string | null {
  const mods = new Set<string>();
  let key: string | null = null;
  for (const part of s.split("+")) {
    const mod = MODS.find((m) => m.toLowerCase() === part.toLowerCase());
    if (mod) {
      if (mods.has(mod)) return null;
      mods.add(mod);
    } else {
      const k = canonicalKey(part);
      if (!k || key) return null;
      key = k;
    }
  }
  return key ? [...MODS.filter((m) => mods.has(m)), key].join("+") : null;
}
export function validate(
  scope: Scope,
  s: string,
): "invalid_shortcut" | "reserved" | null {
  const c = s.length <= 64 ? canonicalize(s) : null;
  if (!c) return "invalid_shortcut";
  const parts = c.split("+");
  const key = parts.pop() as string;
  const has = (m: string) => parts.includes(m);
  const printable = key.length === 1 || key === "Space";
  const deleting = key === "Backspace" || key === "ForwardDelete";
  const reserved =
    registry.reserved.all.includes(c) ||
    (scope === "app" && registry.reserved.app.includes(c)) ||
    key.startsWith("Arrow") ||
    (has("Option") && !has("Command") && !has("Control") && printable) ||
    (parts.length === 1 &&
      has("Control") &&
      "AEKFBNP".includes(key) &&
      key.length === 1) ||
    (!(has("Command") || has("Control") || has("Option")) &&
      !(
        scope === "snippetTree" &&
        parts.length === 1 &&
        has("Shift") &&
        deleting
      ));
  return reserved ? "reserved" : null;
}
/** Physical key (`code`), since `key` changes under Option and Shift. */
export function keyFromCode(code: string): string | null {
  const m = /^(?:Key([A-Z])|Digit(\d)|(F\d{1,2}))$/.exec(code);
  if (m) return m[1] ?? m[2] ?? m[3];
  if (code === "Delete") return "ForwardDelete";
  return NAMED.includes(code) ? code : null;
}
/** Canonical shortcut for a keydown, or `null` for modifier-only or unmapped keys. */
export function fromEvent(e: ShortcutEvent): string | null {
  const key = keyFromCode(e.code);
  if (!key) return null;
  const mods = [
    e.ctrlKey && "Control",
    e.altKey && "Option",
    e.shiftKey && "Shift",
    e.metaKey && "Command",
  ].filter(Boolean);
  return canonicalize([...mods, key].join("+"));
}
/** IME composition must never be intercepted; WebKit may end it with keyCode 229. */
export const composing = (e: ShortcutEvent) =>
  e.isComposing || e.keyCode === 229;
/** "Shift plus Command plus P" for VoiceOver. */
export const spoken = (s: string) => s.replaceAll("+", " plus ");

import { useEffect, useRef, useState } from "react";
import { composing, spoken } from "../application/actions/shortcut";

export interface PaletteEntry {
  id: string;
  label: string;
  shortcuts: string[];
  reason: string | null;
}
export function CommandPalette({
  entries,
  onRun,
  onClose,
}: {
  entries: PaletteEntry[];
  onRun: (id: string) => void;
  onClose: () => void;
}) {
  const [query, setQuery] = useState("");
  const [index, setIndex] = useState(0);
  // Captured during the first render, before autoFocus moves focus into the dialog.
  const [invoker] = useState(
    () => document.activeElement as HTMLElement | null,
  );
  // StrictMode unmounts and remounts at once; only restore focus on a real unmount.
  const mounted = useRef(false);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      queueMicrotask(() => {
        if (!mounted.current) invoker?.focus?.();
      });
    };
  }, [invoker]);
  const shown = entries.filter((e) =>
    e.label.toLowerCase().includes(query.trim().toLowerCase()),
  );
  const at = Math.min(index, shown.length - 1);
  return (
    <div role="dialog" aria-modal="true" aria-label="Command palette">
      <input
        autoFocus
        role="combobox"
        aria-expanded="true"
        aria-controls="palette-list"
        aria-label="Filter actions"
        value={query}
        onChange={(e) => {
          setQuery(e.target.value);
          setIndex(0);
        }}
        onKeyDown={(e) => {
          if (composing(e.nativeEvent)) return;
          if (e.key === "Escape") onClose();
          else if (e.key === "ArrowDown")
            setIndex(Math.min(at + 1, shown.length - 1));
          else if (e.key === "ArrowUp") setIndex(Math.max(at - 1, 0));
          else if (e.key === "Enter" && shown[at]) onRun(shown[at].id);
          else return;
          e.preventDefault();
        }}
      />
      <ul id="palette-list" role="listbox" aria-label="Actions">
        {shown.map((entry, i) => (
          <li
            key={entry.id}
            role="option"
            aria-selected={i === at}
            aria-disabled={entry.reason !== null}
            onClick={() => onRun(entry.id)}
          >
            {entry.label}
            {entry.shortcuts.map((s) => (
              <kbd key={s} aria-label={spoken(s)}>
                {s}
              </kbd>
            ))}
            {entry.reason && <small> — {entry.reason}</small>}
          </li>
        ))}
        {shown.length === 0 && <li>No matching action</li>}
      </ul>
    </div>
  );
}

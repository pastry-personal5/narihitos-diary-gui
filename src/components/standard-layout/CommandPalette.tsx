import { useEffect, useRef, useState } from "react";
import { flushSync } from "react-dom";
import { composing, spoken } from "../../application/actions/shortcut";
import { rankCommands } from "../../application/actions/rank";

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
  const [selection, setSelection] = useState<string | null>(
    () => rankCommands(entries, "")[0]?.id ?? null,
  );
  const [invoker] = useState(
    () => document.activeElement as HTMLElement | null,
  );
  const root = useRef<HTMLDivElement>(null);
  const running = useRef(false);
  const shown = rankCommands(entries, query);
  const selected = shown.find((entry) => entry.id === selection) ?? shown[0];
  if (selection !== (selected?.id ?? null)) setSelection(selected?.id ?? null);
  const at = shown.findIndex((entry) => entry.id === selected?.id);
  const close = (restore: boolean) => {
    onClose();
    if (restore) {
      const target = invoker?.isConnected
        ? invoker
        : document.getElementById("palette-toggle");
      target?.focus();
    }
  };
  const run = (id: string) => {
    if (running.current) return;
    running.current = true;
    flushSync(onClose);
    flushSync(() => onRun(id));
    // A command may establish focus in Settings or a confirmation dialog.
    if (document.activeElement === document.body) {
      (invoker?.isConnected
        ? invoker
        : document.getElementById("palette-toggle")
      )?.focus();
    }
  };
  const selectedId = selected?.id;
  useEffect(() => {
    if (selectedId)
      document
        .getElementById(`palette-option-${selectedId}`)
        ?.scrollIntoView?.({ block: "nearest" });
  }, [selectedId]);
  useEffect(() => {
    const outside = (event: Event) => {
      const target = event.target as Element | null;
      if (
        root.current?.contains(target) ||
        target?.closest?.("#palette-toggle")
      )
        return;
      onClose();
    };
    const modal = () => {
      if (document.querySelector('[aria-modal="true"], [data-capture]'))
        onClose();
    };
    const observer = new MutationObserver(modal);
    observer.observe(document.body, {
      childList: true,
      subtree: true,
      attributes: true,
      attributeFilter: ["aria-modal", "data-capture"],
    });
    modal();
    document.addEventListener("pointerdown", outside, true);
    document.addEventListener("focusin", outside, true);
    return () => {
      observer.disconnect();
      document.removeEventListener("pointerdown", outside, true);
      document.removeEventListener("focusin", outside, true);
    };
  }, [onClose]);
  return (
    <div
      ref={root}
      id="command-palette"
      className="command-palette"
      role="dialog"
      aria-label="Command palette"
    >
      <input
        autoFocus
        role="combobox"
        aria-expanded="true"
        aria-autocomplete="list"
        aria-controls="palette-list"
        aria-activedescendant={
          selected ? `palette-option-${selected.id}` : undefined
        }
        aria-label="Filter actions"
        placeholder="Search commands…"
        value={query}
        onChange={(event) => {
          setQuery(event.target.value);
          setSelection(
            rankCommands(entries, event.target.value)[0]?.id ?? null,
          );
        }}
        onKeyDown={(event) => {
          if (composing(event.nativeEvent)) return;
          if (event.key === "Escape") close(true);
          else if (event.key === "ArrowDown")
            setSelection(shown[Math.min(at + 1, shown.length - 1)]?.id ?? null);
          else if (event.key === "ArrowUp")
            setSelection(shown[Math.max(at - 1, 0)]?.id ?? null);
          else if (event.key === "Enter") {
            if (!event.repeat && selected) run(selected.id);
          } else return;
          event.preventDefault();
        }}
      />
      <ul id="palette-list" role="listbox" aria-label="Actions">
        {shown.map((entry) => (
          <li
            key={entry.id}
            id={`palette-option-${entry.id}`}
            role="option"
            aria-selected={entry.id === selected?.id}
            aria-disabled={entry.reason !== null}
            onPointerDown={(event) => event.preventDefault()}
            onClick={() => run(entry.id)}
          >
            <span>{entry.label}</span>
            <span className="palette-shortcuts">
              {entry.shortcuts.map((shortcut) => (
                <kbd key={shortcut} aria-label={spoken(shortcut)}>
                  {shortcut}
                </kbd>
              ))}
            </span>
            {entry.reason && <small>{entry.reason}</small>}
          </li>
        ))}
      </ul>
      {shown.length === 0 && <p>No matching command</p>}
      <span className="visually-hidden" role="status">
        {shown.length} commands
      </span>
    </div>
  );
}

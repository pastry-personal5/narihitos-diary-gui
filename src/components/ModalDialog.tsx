import { useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { composing } from "../application/actions/shortcut";

/** A destructive prompt starts on Cancel, traps focus, and restores a live invoker.
 * Background siblings are inert only while this dialog is mounted. */
export function ModalDialog({
  label,
  description,
  onDismiss,
  children,
  focusKey,
  className = "",
}: {
  label: string;
  description?: string;
  onDismiss: () => void;
  children: ReactNode;
  focusKey?: string;
  className?: string;
}) {
  const host = useRef<HTMLDivElement>(null),
    live = useRef(false);
  const [invoker] = useState(
    () => document.activeElement as HTMLElement | null,
  );
  useLayoutEffect(() => {
    const dialog = host.current;
    if (!dialog) return;
    live.current = true;
    const background = [
      ...(dialog.parentElement?.parentElement?.children ?? []),
    ].filter(
      (node): node is HTMLElement =>
        node instanceof HTMLElement && node !== dialog.parentElement,
    );
    const previous = background.map((node) => node.inert);
    for (const node of background) node.inert = true;
    (
      dialog.querySelector<HTMLElement>("[data-initial-focus]") ?? dialog
    ).focus();
    return () => {
      background.forEach((node, i) => {
        node.inert = previous[i];
      });
      live.current = false;
      queueMicrotask(() => {
        if (
          live.current ||
          document.querySelector('[role="dialog"][aria-modal="true"]')
        )
          return;
        const target =
          invoker?.isConnected && !invoker.matches(":disabled")
            ? invoker
            : document.querySelector<HTMLElement>(
                '[aria-label="Explorer tabs"] [aria-pressed="true"]',
              );
        target?.focus();
      });
    };
  }, [invoker, focusKey]);
  return (
    <div className="modal-scrim">
      <div
        ref={host}
        role="dialog"
        aria-modal="true"
        aria-label={label}
        aria-describedby={description}
        tabIndex={-1}
        className={`modal-dialog ${className}`}
        onKeyDown={(event) => {
          if (composing(event.nativeEvent)) return;
          if (event.key === "Escape") {
            event.preventDefault();
            event.stopPropagation();
            onDismiss();
          } else if (event.key === "Tab") {
            const buttons = [
              ...event.currentTarget.querySelectorAll<HTMLElement>(
                'button:not(:disabled), input:not(:disabled), [tabindex="0"]',
              ),
            ].filter((element) => !element.hidden);
            const index = buttons.indexOf(
                document.activeElement as HTMLElement,
              ),
              next = event.shiftKey
                ? index <= 0
                  ? buttons.at(-1)
                  : buttons[index - 1]
                : buttons[(index + 1) % buttons.length];
            event.preventDefault();
            (next ?? event.currentTarget).focus();
          }
        }}
      >
        {children}
      </div>
    </div>
  );
}

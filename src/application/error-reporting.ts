import type { ClientEvent } from "./bridge";

/** Reports uncaught errors as fixed events: no message, stack or URL ever leaves the WebView. */
export function installErrorReporting(
  target: EventTarget,
  report: (event: ClientEvent) => Promise<void>,
): void {
  target.addEventListener("error", () => {
    void report({
      code: "ui.render_error",
      level: "error",
      detail: { component: "layout" },
    });
  });
  target.addEventListener("unhandledrejection", () => {
    void report({
      code: "ui.bridge_error",
      level: "warn",
      detail: { component: "bridge" },
    });
  });
}

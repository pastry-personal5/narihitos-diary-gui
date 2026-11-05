import { describe, expect, it, vi } from "vitest";
import { installErrorReporting } from "../src/application/error-reporting";

describe("WebView error reporting", () => {
  it("reports uncaught errors and rejections with fixed redacted events", () => {
    const target = new EventTarget();
    const report = vi.fn().mockResolvedValue(undefined);
    installErrorReporting(target, report);
    target.dispatchEvent(new Event("error"));
    target.dispatchEvent(new Event("unhandledrejection"));
    expect(report.mock.calls).toEqual([
      [
        {
          code: "ui.render_error",
          level: "error",
          detail: { component: "layout" },
        },
      ],
      [
        {
          code: "ui.bridge_error",
          level: "warn",
          detail: { component: "bridge" },
        },
      ],
    ]);
  });
});

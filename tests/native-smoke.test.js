import { readFileSync } from "node:fs";
import { runInNewContext } from "node:vm";
import { JSDOM } from "jsdom";
import { afterEach, expect, test, vi } from "vitest";

const source = readFileSync("src-tauri/src/native_smoke.rs", "utf8");
const script = source
  .slice(source.indexOf("pub fn main_loaded"))
  .match(/r#"([\s\S]*?)"#/u)[1];

afterEach(() => vi.useRealTimers());

function fixture({ disabled = false, editable = "true" } = {}) {
  vi.useFakeTimers();
  const dom = new JSDOM(
    `
    <button aria-label="Create Empty" ${disabled ? "disabled" : ""}></button>
    <div class="editor-surface"><div class="cm-content" contenteditable="${editable}"></div></div>
  `,
    { url: "tauri://localhost" },
  );
  const invoke = vi.fn(async (command) => {
    if (command === "get_settings") throw Error("reached settings checks");
  });
  // WKWebView exposes the packaged custom-scheme origin; jsdom treats it as opaque.
  const window = {
    document: dom.window.document,
    location: { origin: "tauri://localhost" },
    __TAURI_INTERNALS__: { invoke },
  };
  const result = runInNewContext(script, {
    window,
    document: window.document,
    setTimeout: globalThis.setTimeout,
    clearTimeout: globalThis.clearTimeout,
    console: { error: vi.fn() },
    // These readiness regressions stop at settings; emulate only the preceding CSP check.
    fetch: () => {
      const event = new dom.window.Event("securitypolicyviolation");
      event.violatedDirective = "connect-src";
      event.blockedURI = "http://127.0.0.1:1/api/v1/status";
      window.document.dispatchEvent(event);
      return Promise.resolve();
    },
  });
  return { window, invoke, result };
}

test.each([
  { disabled: true, editable: "true" },
  { disabled: false, editable: "false" },
])("waits for automatic connection and editor readiness: %o", async (state) => {
  const { window, invoke, result } = fixture(state);
  await vi.advanceTimersByTimeAsync(100);
  expect(invoke.mock.calls).toEqual([["get_app_metadata"]]);
  expect(window.document.querySelector("button").textContent).toBe("");
  window.document.querySelector("button").disabled = false;
  window.document
    .querySelector(".cm-content")
    .setAttribute("contenteditable", "true");
  await vi.advanceTimersByTimeAsync(50);
  await result;
  expect(invoke).toHaveBeenCalledWith("get_settings");
  expect(window.__smokeError).toBe("reached settings checks");
});

test("reports readiness timeout before running connected fixture commands", async () => {
  const { window, invoke, result } = fixture({ disabled: true });
  await vi.advanceTimersByTimeAsync(5000);
  await result;
  expect(window.__smokeError).toBe(
    "automatic UI connection/editor readiness timeout",
  );
  expect(invoke.mock.calls).toEqual([["get_app_metadata"]]);
});

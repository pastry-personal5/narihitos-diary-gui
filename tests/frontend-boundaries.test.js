import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { frontendViolations } from "../scripts/frontend-boundaries.mjs";
function sources(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((item) => {
    const path = join(directory, item.name);
    return item.isDirectory()
      ? sources(path)
      : /\.[jt]sx?$/u.test(path)
        ? [path]
        : [];
  });
}
describe("frontend dependency boundaries", () => {
  it("keeps application policy inward and feature hooks local", () => {
    const failures = sources("src").flatMap((file) =>
      frontendViolations(file, readFileSync(file, "utf8")).map(
        (reason) => `${file}: ${reason}`,
      ),
    );
    expect(failures).toEqual([]);
  });
  it("rejects implementation imports through alternate paths and all import forms", () => {
    for (const source of [
      'import { ResourceBuffer } from "../../editor/resource-buffer";',
      'export { ResourceBuffer } from "../navigation/../../editor/resource-buffer";',
      'const adapter = import("../../adapters/tauri-bridge");',
      'type View = import("@codemirror/view").EditorView;',
      'const React = require("react");',
      "let host: HTMLElement;",
      'document.querySelector("main");',
    ])
      expect(
        frontendViolations("src/application/navigation/probe.ts", source)
          .length,
      ).toBeGreaterThan(0);
    expect(
      frontendViolations(
        "src/components/tabbed-editor/probe.ts",
        'import { useReview } from "../paragraph-review/useReview";',
      ).length,
    ).toBeGreaterThan(0);
  });
  it("permits application contracts and explicit outer composition", () => {
    expect(
      frontendViolations(
        "src/editor/probe.ts",
        'import type { ResourceBufferPort } from "../application/editor/buffers";',
      ),
    ).toEqual([]);
    expect(
      frontendViolations(
        "src/components/useAppControllers.ts",
        'import { CodeMirrorBuffers } from "../editor/resource-buffers"; import { useAIChat } from "./ai-chat/useAIChat";',
      ),
    ).toEqual([]);
    expect(
      frontendViolations(
        "src/application/navigation/probe.ts",
        'import type { ResourceBuffers } from "../editor/buffers";',
      ),
    ).toEqual([]);
  });
});

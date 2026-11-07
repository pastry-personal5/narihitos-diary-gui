// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import type { Bridge } from "../src/application/bridge";
import { ExportController } from "../src/application/export/controller";
import { ExportDialog } from "../src/components/ExportDialog";

afterEach(cleanup);
it("identifies the date, starts focus on Cancel and declines with Escape", async () => {
  const cancelJob = vi.fn(async () => ({}));
  const controller = new ExportController(
    {
      loadAuthentic: async () => ({
        exists: true,
        revision: { sequence: 1, sha256: "a".repeat(64) },
      }),
      startExport: async () => ({
        outcome: "needs_confirmation",
        preview: {
          operation_id: "op",
          expected_checkpoint: 1,
          token: "token",
          reason: "changed",
          write: "replace",
          expires_in_ms: 60000,
        },
      }),
      cancelJob,
      getOperation: async () => ({ export: { state: "cancelled" } }),
    } as unknown as Bridge,
    { flush: async () => true, newId: () => "op", notify: () => {} },
  );
  render(<ExportDialog controller={controller} />);
  await act(async () => controller.start("2026-11-05"));
  const dialog = screen.getByRole("dialog", { name: "Confirm export" });
  expect(dialog.textContent).toContain("2026-11-05");
  expect(document.activeElement).toBe(
    screen.getByRole("button", { name: "Cancel" }),
  );
  fireEvent.keyDown(dialog, { key: "Tab" });
  expect(document.activeElement).toBe(
    screen.getByRole("button", { name: "Replace" }),
  );
  fireEvent.keyDown(dialog, { key: "Escape" });
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  expect(cancelJob).toHaveBeenCalledWith("op");
});

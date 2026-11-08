// @vitest-environment jsdom
import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, it } from "vitest";
import { AIActivityStatus } from "../src/components/ai-activities/AIActivityStatus";
import { AIActivitiesStore } from "../src/application/ai-activities/store";
import { generationView } from "./generation-fixtures";

afterEach(cleanup);
it("shows brief live activity and never claims stale work is still running", () => {
  const store = new AIActivitiesStore();
  const v = generationView();
  const baseline = (jobs = [{ ...v.job, progress: v.progress }]) =>
    act(() =>
      store.event({
        kind: "job_snapshot",
        instance_id: "i",
        session_id: "s",
        first: true,
        complete: true,
        jobs,
      }),
    );
  baseline([]);
  const ui = render(<AIActivityStatus store={store} />);
  const status = () =>
    screen.getByLabelText("AI Activities status").textContent;
  expect(status()).toBe("AI idle");
  ui.rerender(
    <AIActivityStatus store={store} preparing="Sending AI Chat message" />,
  );
  expect(status()).toBe("Sending AI Chat message");
  ui.rerender(<AIActivityStatus store={store} />);
  baseline();
  expect(status()).toBe("Running generation for 2026-11-06");
  act(() =>
    store.observe({
      ...v.job,
      progress: { ...v.progress, silence_ms: 30_000 },
    }),
  );
  expect(status()).toBe("Waiting for generation for 2026-11-06");
  act(() =>
    store.observe({
      ...v.job,
      progress: { ...v.progress, cancellation_requested: true },
    }),
  );
  expect(status()).toBe("Stopping generation for 2026-11-06");
  act(() => store.event({ kind: "disconnected" }));
  expect(status()).toContain("unavailable");
  baseline();
  act(() =>
    store.observe({
      ...v.job,
      sequence: 4,
      state: "succeeded",
      progress: v.progress,
    }),
  );
  expect(status()).toBe("AI idle · 1 unread results");
  expect(
    screen.getByLabelText("AI Activities status").getAttribute("aria-live"),
  ).toBe("off");
});

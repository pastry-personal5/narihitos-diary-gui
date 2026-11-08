// @vitest-environment jsdom
import {
  cleanup,
  render,
  screen,
  fireEvent,
  waitFor,
} from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { ActivityDetails } from "../src/components/ActivityDetails";
import { ActivityStore } from "../src/application/activity/store";
import { generationView } from "./generation-fixtures";
it("exposes frozen details, preserves unread on refused navigation", async () => {
  const store = new ActivityStore();
  const v = generationView();
  store.event({
    kind: "job_snapshot",
    instance_id: "i",
    session_id: "s",
    first: true,
    complete: true,
    jobs: [{ ...v.job, state: "succeeded", progress: v.progress }],
  });
  const view = vi.fn().mockResolvedValue(false);
  render(
    <ActivityDetails
      store={store}
      view={view}
      cancel={vi.fn()}
      discard={vi.fn()}
    />,
  );
  expect(screen.getByText(/Writing Language: ko-KR/)).not.toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "View results" }));
  await waitFor(() => expect(view).toHaveBeenCalledTimes(1));
  expect(store.state.jobs.get("j")?.unread).toBe(true);
  fireEvent.click(
    screen.getByRole("button", { name: "Dismiss result indicator" }),
  );
  expect(store.state.jobs.size).toBe(1);
});

afterEach(cleanup);
it("shows the silence hint at thirty seconds only for a fresh active observation", () => {
  const store = new ActivityStore(),
    v = generationView();
  const baseline = (silence_ms: number) =>
    store.event({
      kind: "job_snapshot",
      instance_id: "i",
      session_id: "s",
      first: true,
      complete: true,
      jobs: [{ ...v.job, progress: { ...v.progress, silence_ms } }],
    });
  baseline(29_999);
  const ui = render(
    <ActivityDetails
      store={store}
      view={vi.fn()}
      cancel={vi.fn()}
      discard={vi.fn()}
    />,
  );
  expect(ui.container.textContent).not.toContain("Still waiting");
  baseline(30_000);
  ui.rerender(
    <ActivityDetails
      store={store}
      view={vi.fn()}
      cancel={vi.fn()}
      discard={vi.fn()}
    />,
  );
  expect(ui.container.textContent).toContain("Still waiting");
});

it("ignores an old-session View acknowledgement and never moves editor focus on completion", async () => {
  const store = new ActivityStore(),
    v = generationView();
  const active = { ...v.job, progress: v.progress };
  const baseline = (session: string, terminal = false) =>
    store.event({
      kind: "job_snapshot",
      instance_id: "i",
      session_id: session,
      first: true,
      complete: true,
      jobs: [
        {
          ...active,
          state: terminal ? "succeeded" : "running",
          progress: { ...v.progress, session_id: session },
        },
      ],
    });
  baseline("s");
  let resolve!: (opened: boolean) => void;
  render(
    <>
      <textarea aria-label="Editor" defaultValue="keep caret" />
      <ActivityDetails
        store={store}
        view={() =>
          new Promise((r) => {
            resolve = r;
          })
        }
        cancel={vi.fn()}
        discard={vi.fn()}
      />
    </>,
  );
  const editor = screen.getByRole("textbox", {
    name: "Editor",
  }) as HTMLTextAreaElement;
  editor.focus();
  editor.setSelectionRange(2, 5);
  baseline("s", true);
  await waitFor(() => expect(store.state.jobs.get("j")?.unread).toBe(true));
  expect(document.activeElement).toBe(editor);
  expect([editor.selectionStart, editor.selectionEnd]).toEqual([2, 5]);
  fireEvent.click(screen.getByRole("button", { name: "View results" }));
  baseline("replacement", true);
  resolve(true);
  await Promise.resolve();
  expect(store.state.jobs.get("j")?.unread).toBe(true);
});

it("keeps named textual progress available with reduced motion", () => {
  const original = window.matchMedia;
  window.matchMedia = vi.fn().mockImplementation(() => ({
    matches: true,
    media: "(prefers-reduced-motion: reduce)",
    addEventListener() {},
    removeEventListener() {},
  }));
  try {
    const store = new ActivityStore(),
      v = generationView();
    store.event({
      kind: "job_snapshot",
      instance_id: "i",
      session_id: "s",
      first: true,
      complete: true,
      jobs: [{ ...v.job, progress: v.progress }],
    });
    render(
      <ActivityDetails
        store={store}
        view={vi.fn()}
        cancel={vi.fn()}
        discard={vi.fn()}
      />,
    );
    const details = screen.getByRole("region", { name: "AI activity details" });
    expect(details.textContent).toContain("Writing Language: ko-KR");
    expect(details.textContent).toContain("Receiving the response");
    expect(
      screen.getByRole("button", { name: "Cancel AI run" }),
    ).not.toBeNull();
    expect(["", "none"]).toContain(getComputedStyle(details).animationName);
  } finally {
    window.matchMedia = original;
  }
});

it("keeps explicit cancel, discard and reconnect actions with error feedback", async () => {
  const store = new ActivityStore(),
    v = generationView();
  const job = { ...v.job, progress: v.progress };
  store.event({
    kind: "job_snapshot",
    instance_id: "i",
    session_id: "s",
    first: true,
    complete: true,
    jobs: [job],
  });
  const cancel = vi
    .fn()
    .mockRejectedValue(new Error("private provider message"));
  const discard = vi.fn().mockResolvedValue(undefined),
    retry = vi.fn();
  const ui = render(
    <ActivityDetails
      store={store}
      view={vi.fn()}
      cancel={cancel}
      discard={discard}
      retry={retry}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "Cancel AI run" }));
  await waitFor(() =>
    expect(screen.getByRole("alert").textContent).toContain(
      "could not be completed",
    ),
  );
  expect(cancel).toHaveBeenCalledWith(job);
  expect(ui.container.textContent).not.toContain("private provider message");
  store.event({ kind: "disconnected" });
  ui.rerender(
    <ActivityDetails
      store={store}
      view={vi.fn()}
      cancel={cancel}
      discard={discard}
      retry={retry}
    />,
  );
  expect(
    screen.getByRole<HTMLButtonElement>("button", { name: "Cancel AI run" })
      .disabled,
  ).toBe(true);
  fireEvent.click(screen.getByRole("button", { name: "Reconnect" }));
  expect(retry).toHaveBeenCalledOnce();
  store.event({
    kind: "job_snapshot",
    instance_id: "i",
    session_id: "s",
    first: true,
    complete: true,
    jobs: [{ ...job, sequence: 4, state: "cancelled" }],
  });
  ui.rerender(
    <ActivityDetails
      store={store}
      view={vi.fn()}
      cancel={cancel}
      discard={discard}
      retry={retry}
    />,
  );
  fireEvent.click(
    screen.getByRole("button", { name: "Discard generation results" }),
  );
  await waitFor(() => expect(discard).toHaveBeenCalledOnce());
  expect(screen.getByText(/This activity was stopped/)).toBeTruthy();
});

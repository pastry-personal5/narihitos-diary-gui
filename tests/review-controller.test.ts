import { describe, expect, it } from "vitest";
import { AuthenticError } from "../src/application/bridge";
import { ReviewController } from "../src/application/review/controller";
import { digest, rev, reviewFake, view } from "./review-fake";

let n = 0;
const ids = () => `00000000-0000-4000-8000-${String(++n).padStart(12, "0")}`;
function setup() {
  const f = reviewFake();
  const c = new ReviewController(f.bridge, f.editor, ids);
  return { ...f, c };
}

describe("paragraph review controller", () => {
  it("flushes Authentic before starting and shows the server body", async () => {
    const { c, calls, editor } = setup();
    expect(await c.start("2026-11-07", "job-1")).toBe(true);
    expect(editor.flush).toHaveBeenCalledWith("2026-11-07");
    expect(calls.startReview).toHaveBeenCalledTimes(1);
    expect(editor.show).toHaveBeenCalledWith(
      "2026-11-07",
      view(),
      expect.any(Function),
    );
    expect(c.state.announcement).toBe("Review started. 2 changes to review.");
  });

  it("does not start with a pending save problem", async () => {
    const { c, calls, editor } = setup();
    editor.flush.mockResolvedValue(false);
    expect(await c.start("2026-11-07", "job-1")).toBe(false);
    expect(calls.startReview).not.toHaveBeenCalled();
    expect(c.state.error).toBe("authentic_save");
  });

  it("sends the current revision and set digest, and shows accepted state only after the reply", async () => {
    const { c, calls, editor } = setup();
    await c.start("2026-11-07", "job-1");
    editor.show.mockClear();
    let finish!: () => void;
    calls.decideReview.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finish = () =>
            resolve({
              receipt: null,
              view: view({
                revision: rev(2),
                set_digest: digest("b"),
                hunks: [],
              }),
            });
        }),
    );
    const work = c.decide("h1", "accept");
    expect(c.state.busy).toBe(true);
    expect(editor.show).not.toHaveBeenCalled();
    // A second decision while one is in flight is ignored.
    expect(await c.decide("h2", "accept")).toBe(false);
    finish();
    await work;
    expect(calls.decideReview.mock.calls[0][0]).toMatchObject({
      hunk_id: "h1",
      action: "accept",
      expected_revision: rev(1),
      expected_set_digest: digest("a"),
    });
    expect(c.state.view?.revision).toEqual(rev(2));
    expect(c.state.announcement).toContain("All changes decided");
  });

  it("keeps the operation ID when an uncertain accept is retried", async () => {
    const { c, calls } = setup();
    await c.start("2026-11-07", "job-1");
    calls.decideReview.mockRejectedValueOnce(
      new AuthenticError("outcome_uncertain", true),
    );
    expect(await c.decide("h1", "accept")).toBe(false);
    expect(c.state.uncertain).toBe(true);
    // Ending is refused until reconciled.
    expect(await c.end()).toBe(false);
    expect(calls.endReview).not.toHaveBeenCalled();
    await c.decide("h1", "accept");
    const [first, second] = calls.decideReview.mock.calls.map(
      (call) => (call as unknown as [{ operation_id: string }])[0].operation_id,
    );
    expect(second).toBe(first);
    expect(c.state.uncertain).toBe(false);
  });

  it("invalidates without rebasing when the entry changed", async () => {
    const { c, calls, editor } = setup();
    await c.start("2026-11-07", "job-1");
    calls.decideReview.mockRejectedValueOnce(
      new AuthenticError("review_invalidated", false),
    );
    await c.decide("h1", "accept");
    expect(c.state.view).toBeNull();
    expect(c.state.invalidated).toBe(true);
    expect(c.state.announcement).toBe("Entry changed — regenerate to review.");
    expect(calls.endReview).toHaveBeenCalled();
    expect(editor.reload).toHaveBeenCalledWith("2026-11-07");
  });

  it("ignores its own saves but reacts to an unrelated revision change", async () => {
    const { c, calls, editor } = setup();
    await c.start("2026-11-07", "job-1");
    await c.onAuthenticChanged(); // same revision: our own write
    expect(c.state.view).not.toBeNull();
    calls.loadAuthentic.mockResolvedValueOnce({
      date: "2026-11-07",
      exists: true,
      revision: rev(5),
      body: "other",
    });
    await c.onAuthenticChanged();
    expect(c.state.invalidated).toBe(true);
    expect(editor.show).toHaveBeenLastCalledWith(
      "2026-11-07",
      null,
      expect.any(Function),
    );
  });

  it("rechecks after an in-flight decision instead of misreading its echo", async () => {
    const { c, calls } = setup();
    await c.start("2026-11-07", "job-1");
    let finish!: () => void;
    calls.decideReview.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finish = () =>
            resolve({
              receipt: null,
              view: view({ revision: rev(2), set_digest: digest("b") }),
            });
        }),
    );
    const work = c.decide("h1", "accept");
    await c.onAuthenticChanged(); // echo arrives before the reply
    calls.loadAuthentic.mockResolvedValue({
      date: "2026-11-07",
      exists: true,
      revision: rev(2),
      body: "x",
    });
    finish();
    await work;
    await Promise.resolve();
    expect(c.state.invalidated).toBe(false);
    expect(c.state.view?.revision).toEqual(rev(2));
  });

  it("ends the review and resumes editing", async () => {
    const { c, calls, editor } = setup();
    await c.start("2026-11-07", "job-1");
    expect(await c.end()).toBe(true);
    expect(calls.endReview).toHaveBeenCalledWith(view().review_id);
    expect(editor.show).toHaveBeenLastCalledWith(
      "2026-11-07",
      null,
      expect.any(Function),
    );
    expect(c.state.view).toBeNull();
    expect(c.state.announcement).toBe("Review ended. Editing resumed.");
  });
});

it.each(["decided", "set_mismatch"])(
  "removes stale decision controls after %s",
  async (error) => {
    const { c, calls, editor } = setup();
    await c.start("2026-11-07", "job-1");
    calls.decideReview.mockRejectedValueOnce(new AuthenticError(error, false));
    expect(await c.decide("h1", "reject")).toBe(false);
    expect(c.state.view).toBeNull();
    expect(editor.show).toHaveBeenLastCalledWith(
      "2026-11-07",
      null,
      expect.any(Function),
    );
    expect(editor.reload).toHaveBeenCalledWith("2026-11-07");
  },
);

it("rechecks a watcher read that predates a completed accept", async () => {
  const { c, calls } = setup();
  await c.start("2026-11-07", "job-1");
  let finish!: (file: Awaited<ReturnType<typeof calls.loadAuthentic>>) => void;
  calls.loadAuthentic.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  const read = c.onAuthenticChanged();
  await c.decide("h1", "accept");
  calls.loadAuthentic.mockResolvedValue({
    date: "2026-11-07",
    exists: true,
    revision: rev(2),
    body: "saved",
  });
  finish({ date: "2026-11-07", exists: true, revision: rev(1), body: "old" });
  await read;
  await Promise.resolve();
  expect(c.state.invalidated).toBe(false);
  expect(c.state.view?.revision).toEqual(rev(2));
});

it("preserves an acknowledged decision when the editor refuses the updated view", async () => {
  const { c, calls, editor } = setup();
  await c.start("2026-11-07", "job-1");
  editor.show.mockResolvedValueOnce(false);
  expect(await c.decide("h1", "accept")).toBe(true);
  expect(calls.decideReview).toHaveBeenCalledTimes(1);
  expect(calls.endReview).toHaveBeenCalled();
  expect(editor.show).toHaveBeenLastCalledWith(
    "2026-11-07",
    null,
    expect.any(Function),
  );
  expect(editor.reload).toHaveBeenCalledWith("2026-11-07");
  expect(c.state.view).toBeNull();
  expect(c.state.uncertain).toBe(false);
  expect(c.state.error).toBeNull();
  expect(await c.decide("h1", "accept")).toBe(false);
  expect(calls.decideReview).toHaveBeenCalledTimes(1);
});

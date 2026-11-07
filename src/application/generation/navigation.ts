import { AuthenticError } from "../bridge";
import type { NavigationModel } from "../navigation/model";
import type { Source } from "../navigation/contracts";
import type { Preparation, SelectedSnippet } from "./controller";
function fail(code: string): never {
  throw new AuthenticError(code, false);
}

/** Waits for this date's new import without switching editors or changing the user's date. */
async function imported(
  model: NavigationModel,
  date: string,
  signal: AbortSignal,
): Promise<Source[]> {
  const context = await new Promise<ReturnType<typeof model.imports.view>>(
    (resolve, reject) => {
      let stop = () => {};
      const abort = () => {
        stop();
        reject(new AuthenticError("cancelled", false));
      };
      const check = () => {
        if (signal.aborted) {
          abort();
          return;
        }
        const v = model.imports.view(date);
        if (v.phase === "ready" && !v.stale) {
          stop();
          signal.removeEventListener("abort", abort);
          resolve(v);
        } else if (["failed", "cancelled", "idle"].includes(v.phase)) {
          stop();
          signal.removeEventListener("abort", abort);
          reject(new AuthenticError("reimport_required", false));
        }
      };
      stop = model.imports.subscribe(check);
      signal.addEventListener("abort", abort, { once: true });
      check();
    },
  );
  let page = context;
  while (
    page.page?.next_cursor !== null &&
    page.page?.next_cursor !== undefined
  ) {
    if (signal.aborted) fail("cancelled");
    await model.imports.loadMore(date);
    const next = model.imports.view(date);
    if (
      next.phase !== "ready" ||
      next.error ||
      next.items.length <= page.items.length
    )
      fail("reimport_required");
    page = next;
  }
  return page.items.flatMap((item) =>
    "source" in item && item.source ? [item.source] : [],
  );
}
export function generationPreparation(
  model: NavigationModel,
  chatRunning: () => boolean,
  endReview?: (date: string) => Promise<boolean>,
): Preparation {
  return {
    chatRunning,
    selected: (date) =>
      model.imports.eligible(date).map((s) => ({
        import_id: s.import_id,
        source_id: s.id,
        digest: s.digest,
      })),
    dirty: (selected) => {
      const ids = new Set(selected.map((s) => s.source_id));
      return [...model.resources.resources.values()]
        .filter(
          (r) =>
            r.kind === "source" &&
            r.dirty &&
            r.source &&
            ids.has(r.source.source.id),
        )
        .map((r) => ({ id: r.source!.source.id, label: r.label }));
    },
    flushAuthentic: async (date) => {
      // Regeneration ends the old review first (the server refuses a live one for the date).
      if (endReview && !(await endReview(date))) return false;
      const resource = model.resources.resources.get(`authentic:${date}`);
      if (!resource?.session || resource.loading || resource.error)
        return false;
      return (await resource.session.settle()) === "proceed";
    },
    saveAndRefresh: async (date, selected, dirty, signal) => {
      const expected = new Map(selected.map((s) => [s.source_id, s.digest]));
      for (const item of dirty) {
        if (signal.aborted) fail("cancelled");
        const resource = [...model.resources.resources.values()].find(
          (r) => r.source?.source.id === item.id,
        );
        if (
          !resource ||
          !(await model.resources.saveSource(resource.key)) ||
          resource.dirty ||
          !resource.baseDigest
        )
          fail("source_save");
        expected.set(item.id, resource.baseDigest);
      }
      if (signal.aborted) fail("cancelled");
      await model.imports.refresh(date);
      const fresh = await imported(model, date, signal);
      const choices: SelectedSnippet[] = [];
      for (const chosen of selected) {
        const source = fresh.find((s) => s.id === chosen.source_id);
        if (
          !source?.eligible ||
          source.digest !== expected.get(chosen.source_id) ||
          model.imports.sourceDisabled(source)
        )
          fail("reimport_required");
        // Save was an explicit choice for this selection; authorize exactly the saved bytes.
        model.imports.setIncluded(date, source.id, true);
        choices.push({
          source_id: source.id,
          import_id: source.import_id,
          digest: source.digest,
        });
      }
      return choices;
    },
  };
}

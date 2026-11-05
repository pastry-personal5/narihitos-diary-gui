import { expect, it, vi } from "vitest";
import { AuthenticCatalog } from "../src/application/navigation/catalog";
import { SessionBudget } from "../src/application/navigation/budget";
import type { AuthenticPage, Bridge } from "../src/application/bridge";
function pending<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((r) => {
    resolve = r;
  });
  return { promise, resolve };
}
const row = (date: string) => ({ date, revision: null, problem: null });
it("coalesces watcher refresh while loading and publishes newly saved empty dates", async () => {
  const first = pending<AuthenticPage>();
  const list = vi
    .fn()
    .mockReturnValueOnce(first.promise)
    .mockResolvedValueOnce({ items: [row("2024-02-29")], next_cursor: null });
  const catalog = new AuthenticCatalog(
    { listAuthentic: list } as unknown as Bridge,
    new SessionBudget(),
  );
  catalog.connect(true);
  catalog.refresh();
  catalog.refresh();
  expect(list).toHaveBeenCalledTimes(1);
  first.resolve({ items: [], next_cursor: null });
  await vi.waitFor(() =>
    expect(catalog.items.map((x) => x.date)).toEqual(["2024-02-29"]),
  );
  expect(list).toHaveBeenCalledTimes(2);
  catalog.dispose();
});
it("retains a valid page and retries continuation after a failure", async () => {
  const list = vi
    .fn()
    .mockResolvedValueOnce({
      items: [row("2024-01-01")],
      next_cursor: "2024-01-01",
    })
    .mockRejectedValueOnce(new Error())
    .mockResolvedValueOnce({ items: [row("2024-02-29")], next_cursor: null });
  const catalog = new AuthenticCatalog(
    { listAuthentic: list } as unknown as Bridge,
    new SessionBudget(),
  );
  catalog.connect(true);
  await vi.waitFor(() => expect(catalog.cursor).toBe("2024-01-01"));
  await catalog.more();
  expect(catalog.error).not.toBeNull();
  expect(catalog.items).toHaveLength(1);
  await catalog.more();
  expect(catalog.items.map((x) => x.date)).toEqual([
    "2024-01-01",
    "2024-02-29",
  ]);
  expect(list.mock.calls.slice(1)).toEqual([["2024-01-01"], ["2024-01-01"]]);
  catalog.dispose();
});

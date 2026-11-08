import { SessionBudget } from "../src/application/navigation/budget";
import { GlobalMessagesStore } from "../src/application/global-messages/store";
import { expect, it, vi } from "vitest";
import type { Bridge } from "../src/application/bridge";
import { NavigationModel } from "../src/application/navigation/model";
import { navigationFake } from "./navigation-fake";
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((r) => {
    resolve = r;
  });
  return { promise, resolve };
}
it("ignores a late Today reply after a newer calendar browse", async () => {
  const nav = navigationFake();
  const calendar = await nav.getCalendar();
  const bridge = {
    ...nav,
    listAuthentic: async () => ({ items: [], next_cursor: null }),
  } as unknown as Bridge;
  const model = new NavigationModel(
    bridge,
    new GlobalMessagesStore(),
    new SessionBudget(),
  );
  const change = vi
    .spyOn(model.resources, "changeDate")
    .mockResolvedValue(true);
  vi.spyOn(model.imports, "select").mockResolvedValue(true);
  await model.connect(true, "instance", 1, true);
  model.resources.selectedDate = calendar.today;
  change.mockClear();
  const late = deferred<typeof calendar>();
  vi.spyOn(bridge, "getCalendar")
    .mockReturnValueOnce(late.promise)
    .mockResolvedValueOnce(calendar);
  const today = model.today();
  await model.browse(calendar.anchor, "month");
  late.resolve(calendar);
  await today;
  expect(change).not.toHaveBeenCalled();
  expect(model.calendar?.anchor).toBe(calendar.anchor);
  model.dispose();
});

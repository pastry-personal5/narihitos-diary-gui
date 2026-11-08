import { useSyncExternalStore } from "react";
interface Observable<T> {
  subscribe(listener: () => void): () => void;
  getSnapshot(): T;
}
const off = () => () => {};
/** Subscription plumbing shared by views, without borrowing another feature's hook. */
export function useControllerSnapshot<T>(
  controller: Observable<T> | undefined,
  empty: T,
): T {
  return useSyncExternalStore(
    controller?.subscribe ?? off,
    controller?.getSnapshot ?? (() => empty),
  );
}

import { useSyncExternalStore } from "react";
import type { ReviewController } from "../application/review/controller";
const NONE = {
  view: null,
  busy: false,
  invalidated: false,
  uncertain: false,
  error: null,
  announcement: "",
};
const off = () => () => {};
/** A missing controller (tests, fakes without review) reads as an idle review. */
export function useReview(controller?: ReviewController) {
  return useSyncExternalStore(
    controller?.subscribe ?? off,
    controller?.getSnapshot ?? (() => NONE),
  );
}

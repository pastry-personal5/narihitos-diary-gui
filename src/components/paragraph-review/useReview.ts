import {
  EMPTY_REVIEW_STATE,
  type ReviewController,
} from "../../application/review/controller";
import { useControllerSnapshot } from "../useControllerSnapshot";
/** A missing controller (tests, fakes without review) reads as an idle review. */
export function useReview(controller?: ReviewController) {
  return useControllerSnapshot(controller, EMPTY_REVIEW_STATE);
}

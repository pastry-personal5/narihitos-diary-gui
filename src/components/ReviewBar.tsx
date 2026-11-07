import {
  REVIEW_MESSAGE,
  type ReviewController,
} from "../application/review/controller";
import { useReview } from "./useReview";

/** Review status and the controls that are not tied to one change. Announcements come from one
 * polite status region so a long review never floods the screen reader. */
export function ReviewBar({
  controller,
  coordinated = false,
}: {
  controller: ReviewController;
  coordinated?: boolean;
}) {
  const s = useReview(controller);
  const pending = s.view?.hunks.length ?? 0;
  return (
    <>
      {(s.view || s.invalidated || s.error) && (
        <div className="review-bar" role="group" aria-label="Review">
          {s.view && (
            <>
              <strong>
                Reviewing candidate {s.view.candidate_id.split("-").at(-1)}
              </strong>
              <span>
                {pending
                  ? `${pending} ${pending === 1 ? "change" : "changes"} to review · editing paused`
                  : "All changes decided"}
              </span>
              {s.uncertain && (
                <span>
                  Retry the same change from its Accept or Reject button.
                </span>
              )}
              <button disabled={s.busy} onClick={() => void controller.end()}>
                End review
              </button>
            </>
          )}
          {s.invalidated && (
            <>
              <span role="alert">{REVIEW_MESSAGE.review_invalidated}</span>
              <button onClick={() => controller.dismissInvalidated()}>
                Dismiss
              </button>
            </>
          )}
          {s.error && !s.invalidated && (
            <span role="alert">
              {REVIEW_MESSAGE[s.error] ?? REVIEW_MESSAGE.failed}
            </span>
          )}
        </div>
      )}
      <p
        role={coordinated ? undefined : "status"}
        aria-live={coordinated ? "off" : undefined}
        aria-atomic="true"
        className="visually-hidden"
      >
        {s.announcement}
      </p>
    </>
  );
}

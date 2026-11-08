import { useRef, useState } from "react";
import type { CalendarAction } from "../application/navigation/contracts";
import dateIcon from "../assets/tab-date.png";
import snippetIcon from "../assets/tab-snippet.png";
import draftIcon from "../assets/tab-draft.png";
import entryIcon from "../assets/tab-entry.png";
import previousMonthIcon from "../assets/navigation/previous-month.png";
import nextMonthIcon from "../assets/navigation/next-month.png";
import { snippetLabels } from "../application/navigation/snippet-label";
import { SymbolLabel } from "./Symbol";
import { SnippetTable } from "./SnippetTable";
import type { NavigationModel } from "../application/navigation/model";

const views = ["Date tab", "Snippet tab", "Draft tab", "Entry tab"] as const;
const viewIcons = {
  "Date tab": dateIcon,
  "Snippet tab": snippetIcon,
  "Draft tab": draftIcon,
  "Entry tab": entryIcon,
};
import { useGeneration } from "./useGeneration";
import { candidateKey } from "./candidateTabs";
import type { GenerationController } from "../application/generation/controller";
export function Explorer({
  model,
  generator,
}: {
  model: NavigationModel;
  generator: GenerationController;
}) {
  const generation = useGeneration(generator);
  const [view, setView] = useState<(typeof views)[number]>("Date tab");
  const date = model.resources.selectedDate;
  const calendar = model.calendar;
  const [focusDate, setFocusDate] = useState<string | null>(null);
  const grid = useRef<HTMLDivElement>(null);
  const activeDate = calendar?.days.some((day) => day.date === focusDate)
    ? focusDate
    : calendar?.anchor;
  async function moveFocus(from: string, action: CalendarAction) {
    const result = await model.browse(from, action);
    if (!result) return;
    setFocusDate(result.anchor);
    requestAnimationFrame(() =>
      grid.current
        ?.querySelector<HTMLButtonElement>(`[data-date="${result.anchor}"]`)
        ?.focus(),
    );
  }
  const context = date ? model.imports.view(date) : null;
  const labels = snippetLabels(context?.page?.items ?? []);
  const groups = context?.page?.groups ?? [];
  return (
    <aside aria-label="Explorer" className="explorer">
      <h2>Explorer</h2>
      <div aria-label="Explorer views" className="explorer-tabs">
        {views.map((name) => (
          <button
            key={name}
            aria-pressed={view === name}
            title={name}
            onClick={() => setView(name)}
          >
            <span
              aria-hidden="true"
              className="symbol"
              style={{
                width: 20,
                height: 20,
                maskImage: `url(${viewIcons[name]})`,
                WebkitMaskImage: `url(${viewIcons[name]})`,
              }}
            />
            <span className="visually-hidden">{name}</span>
          </button>
        ))}
      </div>
      {view === "Date tab" && (
        <>
          <label>
            Date{" "}
            <input
              type="date"
              value={date ?? ""}
              disabled={!calendar || model.resources.transitioning}
              onChange={(e) => {
                if (e.target.value) void model.chooseDate(e.target.value);
              }}
            />
          </label>
          <button disabled={!calendar} onClick={() => void model.today()}>
            Today
          </button>
          {model.calendarLoading && <p role="status">Loading calendar…</p>}
          {model.calendarError && (
            <p role="alert">
              {model.calendarError}{" "}
              <button onClick={() => void model.browse()}>
                Retry calendar
              </button>
            </p>
          )}
          {calendar && (
            <>
              <div className="calendar-heading">
                <button
                  type="button"
                  className="calendar-month-button"
                  aria-label="Previous month"
                  title="Previous month"
                  disabled={!calendar.previous_month}
                  onClick={() =>
                    void model.browse(calendar.anchor, "previous_month")
                  }
                >
                  <span
                    aria-hidden="true"
                    className="bitmap-icon"
                    style={{
                      maskImage: `url(${previousMonthIcon})`,
                      WebkitMaskImage: `url(${previousMonthIcon})`,
                    }}
                  />
                </button>
                <strong>
                  {calendar.year}-{String(calendar.month).padStart(2, "0")}
                </strong>
                <button
                  type="button"
                  className="calendar-month-button"
                  aria-label="Next month"
                  title="Next month"
                  disabled={!calendar.next_month}
                  onClick={() =>
                    void model.browse(calendar.anchor, "next_month")
                  }
                >
                  <span
                    aria-hidden="true"
                    className="bitmap-icon"
                    style={{
                      maskImage: `url(${nextMonthIcon})`,
                      WebkitMaskImage: `url(${nextMonthIcon})`,
                    }}
                  />
                </button>
              </div>
              <div
                ref={grid}
                role="grid"
                aria-label={`Calendar ${calendar.year}-${String(calendar.month).padStart(2, "0")}`}
                className="calendar-grid"
              >
                {calendar.days.map((day) => (
                  <button
                    role="gridcell"
                    key={day.date}
                    aria-selected={date === day.date}
                    aria-label={day.date}
                    data-date={day.date}
                    tabIndex={day.date === activeDate ? 0 : -1}
                    onFocus={() => setFocusDate(day.date)}
                    onKeyDown={(e) => {
                      const action: Record<string, CalendarAction> = {
                        ArrowLeft: "previous_day",
                        ArrowRight: "next_day",
                        ArrowUp: "previous_week",
                        ArrowDown: "next_week",
                      };
                      if (action[e.key] && !e.nativeEvent.isComposing) {
                        e.preventDefault();
                        void moveFocus(day.date, action[e.key]);
                      }
                    }}
                    onClick={() => void model.chooseDate(day.date)}
                  >
                    {day.day}
                  </button>
                ))}
              </div>
            </>
          )}
        </>
      )}
      {view === "Snippet tab" && (
        <div data-snippet-tree>
          <button
            disabled={
              !date ||
              context?.phase === "running" ||
              context?.phase === "loading" ||
              context?.phase === "admitting"
            }
            onClick={() => date && void model.imports.refresh(date)}
          >
            <SymbolLabel name="arrow.clockwise" label="Refresh snippets" />
          </button>
          {context && (
            <>
              <p role="status">
                {context.phase === "running" || context.phase === "admitting"
                  ? "Import in progress…"
                  : context.phase === "loading"
                    ? "Loading snippets…"
                    : context.stale
                      ? "Results are stale. Refresh to use them."
                      : context.phase === "cancelled"
                        ? "Import cancelled."
                        : context.page?.incomplete
                          ? "Scan incomplete; some sources may be missing."
                          : context.partial
                            ? "More results are available."
                            : ""}
              </p>
              {context.error && (
                <p role="alert">
                  Import could not be completed ({context.error}).{" "}
                  <button
                    onClick={() => date && void model.imports.retry(date)}
                  >
                    Retry import
                  </button>
                </p>
              )}
              {context.resolutionSaved && (
                <p>Timestamp choice saved. A new import applies it.</p>
              )}
              {context.page?.reimport_required && (
                <p>Re-import to see groups and open sources.</p>
              )}
              {context.page && context.items.length === 0 && (
                <p>No snippets found.</p>
              )}
              <SnippetTable
                model={model}
                context={context}
                date={date}
                labels={labels}
                groups={groups}
              />
              {context.page?.next_cursor != null && (
                <button
                  disabled={context.phase === "loading"}
                  onClick={() => date && void model.imports.loadMore(date)}
                >
                  Load more snippets
                </button>
              )}
              {(context.phase === "running" ||
                context.phase === "admitting") && (
                <button onClick={() => date && void model.imports.cancel(date)}>
                  Cancel import
                </button>
              )}
            </>
          )}
        </div>
      )}
      {view === "Draft tab" && (
        <>
          {date ? (
            <button onClick={() => void model.resources.openAuthentic(date)}>
              Open Authentic · {date}
            </button>
          ) : (
            <p>Select a date.</p>
          )}
          {(() => {
            const slots = [...generation.jobs.values()]
              .filter((job) => job.date === date)
              .flatMap((job) => job.candidates.map((slot) => ({ job, slot })));
            return slots.length ? (
              <ul aria-label="Candidates">
                {slots.map(({ job, slot }) => (
                  <li key={candidateKey(job.job.id, slot.index)}>
                    <button
                      onClick={() =>
                        void model.resources.openCandidate(
                          candidateKey(job.job.id, slot.index),
                        )
                      }
                    >
                      Candidate {slot.index} ·{" "}
                      {slot.status.replaceAll("_", " ")}
                    </button>
                  </li>
                ))}
              </ul>
            ) : (
              <p>No candidates are available.</p>
            );
          })()}
        </>
      )}
      {view === "Entry tab" && (
        <>
          {model.catalog.items.length === 0 && <p>No saved dates yet.</p>}
          <ul aria-label="Saved dates">
            {model.catalog.items.map((item) => (
              <li key={item.date}>
                <button onClick={() => void model.chooseDate(item.date)}>
                  {item.date}
                </button>
                {item.problem && <span> {item.problem}</span>}
              </li>
            ))}
          </ul>
          {model.catalog.error && (
            <p role="alert">
              {model.catalog.error}{" "}
              <button onClick={() => void model.catalog.refresh()}>
                Retry saved dates
              </button>
            </p>
          )}
          {model.catalog.cursor && (
            <button onClick={() => void model.catalog.more()}>
              Load more dates
            </button>
          )}
        </>
      )}
    </aside>
  );
}

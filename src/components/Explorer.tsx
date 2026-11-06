import { useRef, useState } from "react";
import type { CalendarAction } from "../application/navigation/contracts";
import { snippetLabels } from "../application/navigation/snippet-label";
import { SymbolLabel } from "./Symbol";
import type { NavigationModel } from "../application/navigation/model";
import type {
  Source,
  SourceItem,
  TimeAuthority,
} from "../application/navigation/contracts";

const views = ["Date tab", "Snippet tab", "Draft tab", "Entry tab"] as const;
export function Explorer({ model }: { model: NavigationModel }) {
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
  const [choice, setChoice] = useState<{
    source: Source;
    authority: TimeAuthority;
  } | null>(null);
  const groupOf = (item: SourceItem) =>
    item.kind === "included" || item.kind === "warning"
      ? item.source.navigation?.group_id
      : item.group_id;
  const labels = snippetLabels(context?.page?.items ?? []);
  const groups = context?.page?.groups ?? [];
  const localDateOf = (item: SourceItem) => {
    const source = "source" in item ? item.source : null;
    const local = source?.navigation?.included;
    return local
      ? `${local.date} (UTC${local.offset_seconds >= 0 ? "+" : "−"}${String(Math.floor(Math.abs(local.offset_seconds) / 3600)).padStart(2, "0")}:${String(Math.floor((Math.abs(local.offset_seconds) % 3600) / 60)).padStart(2, "0")})`
      : "Needs attention";
  };
  const byLocalDate = (items: SourceItem[]) => {
    const buckets = new Map<string, SourceItem[]>();
    for (const item of items) {
      const key = localDateOf(item);
      const bucket = buckets.get(key) ?? [];
      bucket.push(item);
      buckets.set(key, bucket);
    }
    return [...buckets].map(([label, entries]) => (
      <section key={label} aria-label={label}>
        <h4>{label}</h4>
        <ul>{entries.map(row)}</ul>
      </section>
    ));
  };
  function row(item: SourceItem, index: number) {
    if (item.kind === "path_collision")
      return (
        <li key={`collision-${index}`}>
          Path collision: {item.relative_keys.join(", ")}
        </li>
      );
    const source = "source" in item ? item.source : null;
    if (!source)
      return (
        <li key={`failure-${index}`}>
          {item.kind === "failure"
            ? `${item.relative_key || "Source root"}: ${item.code}`
            : "Unavailable"}
        </li>
      );
    const inclusion = model.imports.inclusion(source.id);
    const warning =
      item.kind !== "included" ||
      source.dating.state !== "agreed" ||
      source.duplicate_producer_id;
    const moved = model.imports.sourceDisabled(source);
    const trashReason = model.trashReason(source);
    return (
      <li key={source.id} onFocusCapture={() => model.focusSnippet(source)}>
        <div className="snippet-row">
          <input
            type="checkbox"
            aria-label={`Include ${labels.get(source.id) ?? source.relative_key}`}
            checked={!!inclusion?.checked && source.eligible && !moved}
            disabled={!source.eligible || !!context?.stale || moved}
            onChange={(e) =>
              date &&
              model.imports.setIncluded(date, source.id, e.target.checked)
            }
          />
          <button
            disabled={!source.capabilities.read || !!context?.stale || moved}
            title={source.relative_key}
            onClick={() => void model.openSource(source)}
          >
            {labels.get(source.id) ?? source.relative_key}
          </button>
          <button
            disabled={!!trashReason}
            title={trashReason ?? "Preview this snippet and its related audio"}
            aria-label={`Move ${labels.get(source.id) ?? source.relative_key} to Trash`}
            onClick={() => void model.trashSource(source)}
          >
            Move to Trash
          </button>
        </div>
        {moved && (
          <p>Moved to Trash. Refresh snippets before using this row again.</p>
        )}
        {inclusion?.invalidated && (
          <p>Selection invalidated: {inclusion.invalidated}.</p>
        )}
        {item.kind === "failure" && <p>{item.code}</p>}
        {warning && (
          <details>
            <summary>Metadata warning</summary>
            <p>
              Timestamp status: {source.dating.state.replaceAll("_", " ")}
              {source.duplicate_producer_id ? "; duplicate producer ID" : ""}.
            </p>
            <p>
              Header:{" "}
              {source.navigation?.header?.datetime ??
                source.header_invalid ??
                "unavailable"}
            </p>
            <p>
              Filename:{" "}
              {source.navigation?.filename?.datetime ??
                source.filename_invalid ??
                "unavailable"}
            </p>
            {source.navigation?.authority && (
              <p>Chosen authority: {source.navigation.authority}.</p>
            )}
            {(["header", "filename"] as const).map((authority) => (
              <button
                key={authority}
                disabled={
                  !source.navigation?.[authority] ||
                  !!context?.stale ||
                  moved ||
                  !!context?.resolving
                }
                onClick={() => setChoice({ source, authority })}
              >
                Use {authority} time
              </button>
            ))}
            {!moved && choice?.source.id === source.id && (
              <p>
                Use {choice.authority} time for a new import?{" "}
                <button
                  onClick={() => {
                    if (date)
                      void model.imports.resolve(
                        date,
                        source,
                        choice.authority,
                      );
                    setChoice(null);
                  }}
                >
                  Confirm authority
                </button>{" "}
                <button onClick={() => setChoice(null)}>Cancel</button>
              </p>
            )}
          </details>
        )}
      </li>
    );
  }
  return (
    <aside aria-label="Explorer" className="explorer">
      <h2>Explorer</h2>
      <div aria-label="Explorer views" className="explorer-tabs">
        {views.map((name) => (
          <button
            key={name}
            aria-pressed={view === name}
            onClick={() => setView(name)}
          >
            {name}
          </button>
        ))}
      </div>
      <h3>{view}</h3>
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
                  disabled={!calendar.previous_month}
                  onClick={() =>
                    void model.browse(calendar.anchor, "previous_month")
                  }
                >
                  Previous month
                </button>
                <strong>
                  {calendar.year}-{String(calendar.month).padStart(2, "0")}
                </strong>
                <button
                  disabled={!calendar.next_month}
                  onClick={() =>
                    void model.browse(calendar.anchor, "next_month")
                  }
                >
                  Next month
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
              {groups.map((group) => (
                <details key={group.id} open>
                  <summary>
                    {group.alias} · {group.type}
                  </summary>
                  {byLocalDate(
                    context.items.filter((item) => groupOf(item) === group.id),
                  )}
                </details>
              ))}
              {context.items.some((item) => !groupOf(item)) &&
                byLocalDate(context.items.filter((item) => !groupOf(item)))}
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
          <p>No candidates are available.</p>
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

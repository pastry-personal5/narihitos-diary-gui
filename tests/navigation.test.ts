import { afterEach, describe, expect, it, vi } from "vitest";
import { AuthenticError } from "../src/application/bridge";
import { ImportController } from "../src/application/navigation/imports";
import { SessionBudget } from "../src/application/navigation/budget";
import type {
  ImportPage,
  NavigationBridge,
  Source,
  SourceItem,
} from "../src/application/navigation/contracts";
import {
  calendarDto,
  civilDate,
  importPageDto,
  sourceBodyDto,
} from "../src/adapters/navigation-validation";

const date = "2024-02-29";
function source(
  id = "source",
  digest = "a".repeat(64),
  importId = "import",
): Source {
  const local = { date, datetime: `${date}T00:00:00`, offset_seconds: 0 };
  return {
    id,
    relative_key: `2024/02/29/${id}.md`,
    digest,
    import_id: importId,
    dating: {
      state: "agreed",
      header: { unix_ms: 0, precision: "milli" },
      filename: { unix_ms: 0, precision: "milli" },
    },
    duplicate_producer_id: false,
    header_invalid: null,
    filename_invalid: null,
    eligible: true,
    capabilities: {
      read: true,
      save: false,
      trash: false,
      trash_reason: "trash_unverified",
      reason: "writer_unverified",
    },
    navigation: {
      group_id: "group",
      header: local,
      filename: local,
      included: local,
      authority: null,
    },
  };
}
const included = (s: Source): SourceItem => ({
  kind: "included",
  source: s,
  instant: { unix_ms: 0, precision: "milli" },
  misplaced: false,
});
function page(
  items: SourceItem[] = [included(source())],
  next: number | null = null,
  importId = "import",
): ImportPage {
  return {
    import_id: importId,
    date,
    zone: "UTC",
    timezone_database: "test",
    config_generation: 1,
    completed_at: 1,
    items,
    next_cursor: next,
    incomplete: false,
    groups: [{ id: "group", alias: "Diary", type: "ayumi" }],
    reimport_required: false,
  };
}
function fake(): NavigationBridge {
  return {
    getCalendar: vi.fn(),
    importDate: vi.fn(async () => ({
      import_id: "import",
      job: { id: "import", sequence: 1, state: "succeeded" },
    })),
    listSources: vi.fn(async () => page()),
    resolveSourceTime: vi.fn(async () => ({ resolved: true })),
    readSource: vi.fn(),
    getJob: vi.fn(async () => ({
      id: "import",
      sequence: 2,
      state: "succeeded",
    })),
    cancelJob: vi.fn(async () => ({
      id: "import",
      sequence: 2,
      state: "cancelled",
    })),
  };
}
const stores: ImportController[] = [];
function store(bridge = fake(), budget = new SessionBudget()) {
  let id = 0;
  const s = new ImportController(bridge, budget, () => `request-${++id}`, 50);
  stores.push(s);
  s.connect("server:session:1", 1);
  return { s, bridge };
}
afterEach(() => {
  for (const s of stores) s.dispose();
  stores.length = 0;
  vi.useRealTimers();
});
function deferred<T>() {
  let resolve!: (v: T) => void;
  let reject!: (e: unknown) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}

describe("navigation response boundary", () => {
  it("validates civil leap dates and calendar grids without browser Date", () => {
    expect(civilDate(date)).toBe(date);
    for (const d of ["2023-02-29", "0000-01-01", "2024-13-01", "2024-02-00"])
      expect(() => civilDate(d)).toThrow();
    const calendar = {
      anchor: date,
      today: date,
      year: 2024,
      month: 2,
      days: Array.from({ length: 29 }, (_, i) => ({
        date: `2024-02-${String(i + 1).padStart(2, "0")}`,
        day: i + 1,
        weekday: (i + 4) % 7,
      })),
      previous_month: "2024-01-29",
      next_month: "2024-03-29",
    };
    expect(calendarDto(calendar).days).toHaveLength(29);
    expect(() =>
      calendarDto({ ...calendar, days: calendar.days.slice(0, 28) }),
    ).toThrow();
    expect(() => calendarDto({ ...calendar, month: 3 })).toThrow();
    expect(() =>
      calendarDto({ ...calendar, previous_month: "2024-01-28" }),
    ).toThrow();
    expect(() =>
      calendarDto({
        ...calendar,
        days: calendar.days.map((day) => ({
          ...day,
          weekday: (day.weekday + 1) % 7,
        })),
      }),
    ).toThrow();
  });
  it("strips private additive data, validates provenance and forbids premature writers", () => {
    const raw = page();
    Object.assign(raw.items[0], { native_path: "PRIVATE" });
    expect(JSON.stringify(importPageDto(raw))).not.toContain("PRIVATE");
    const wrong = page([included({ ...source(), import_id: "wrong" })]);
    expect(() => importPageDto(wrong)).toThrow();
    const writer = {
      ...source(),
      capabilities: {
        read: true,
        save: true,
        trash: false,
        trash_reason: "trash_unverified",
        reason: "writer_unverified",
      },
    };
    expect(() =>
      sourceBodyDto({
        source: writer,
        snapshot_id: "import",
        body: "ok",
        capabilities: writer.capabilities,
      }),
    ).toThrow();
    expect(() =>
      sourceBodyDto({
        source: source(),
        snapshot_id: "other",
        body: "ok",
        capabilities: source().capabilities,
      }),
    ).toThrow();
    expect(() =>
      sourceBodyDto({
        source: source(),
        snapshot_id: "import",
        body: "あ".repeat(400000),
        capabilities: source().capabilities,
      }),
    ).toThrow();
  });
  it("distinguishes included resolved warnings from generation-ineligible warning rows", () => {
    const resolved = source();
    resolved.dating = {
      state: "header_only",
      header: { unix_ms: 0, precision: "milli" },
    };
    resolved.filename_invalid = "format";
    resolved.navigation!.authority = "header";
    expect(importPageDto(page([included(resolved)])).items[0].kind).toBe(
      "included",
    );
    expect(() =>
      importPageDto(
        page([{ kind: "warning", source: resolved, misplaced: false }]),
      ),
    ).toThrow();
  });
});

describe("session-owned imports", () => {
  it("waits for terminal success, recovers lost events with one lookup, and stops polling", async () => {
    vi.useFakeTimers();
    const { s, bridge } = store();
    vi.mocked(bridge.importDate).mockResolvedValue({
      import_id: "import",
      job: { id: "import", sequence: 1, state: "running" },
    });
    await s.select(date);
    expect(bridge.listSources).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(50);
    expect(bridge.getJob).toHaveBeenCalledTimes(1);
    expect(s.view(date).phase).toBe("ready");
    await vi.advanceTimersByTimeAsync(1000);
    expect(bridge.getJob).toHaveBeenCalledTimes(1);
  });
  it("reports failed/cancelled terminal jobs without listing nonexistent snapshots", async () => {
    const { s, bridge } = store();
    vi.mocked(bridge.importDate).mockResolvedValue({
      import_id: "import",
      job: {
        id: "import",
        sequence: 1,
        state: "failed",
        error_code: "invalid_diary_date",
      },
    });
    await s.select(date);
    expect(s.view(date).error).toBe("invalid_diary_date");
    expect(bridge.listSources).not.toHaveBeenCalled();
    vi.mocked(bridge.importDate).mockResolvedValue({
      import_id: "import",
      job: { id: "import", sequence: 1, state: "cancelled" },
    });
    await s.refresh(date);
    expect(s.view(date).phase).toBe("cancelled");
  });
  it("coalesces refresh and retries uncertain admission with its original request ID", async () => {
    const { s, bridge } = store();
    const pending =
      deferred<Awaited<ReturnType<NavigationBridge["importDate"]>>>();
    vi.mocked(bridge.importDate).mockReturnValueOnce(pending.promise);
    const first = s.select(date);
    await s.refresh(date);
    expect(bridge.importDate).toHaveBeenCalledTimes(1);
    pending.reject(new AuthenticError("server_unavailable", true));
    await first;
    await s.retry(date);
    expect(vi.mocked(bridge.importDate).mock.calls[1]).toEqual(
      vi.mocked(bridge.importDate).mock.calls[0],
    );
    expect(s.view(date).phase).toBe("ready");
  });
  it("ignores replies from a disconnected epoch and stops polling", async () => {
    vi.useFakeTimers();
    const { s, bridge } = store();
    const pending = deferred<ImportPage>();
    vi.mocked(bridge.listSources).mockReturnValue(pending.promise);
    const first = s.select(date);
    await Promise.resolve();
    s.connect(null, 1);
    pending.resolve(page());
    await first;
    expect(s.view(date).page).toBeNull();
    await vi.advanceTimersByTimeAsync(1000);
    expect(bridge.getJob).not.toHaveBeenCalled();
  });
  it("checks eligible snippets by default only on the selected day, not D-1 or D+1", async () => {
    const { s, bridge } = store();
    const other = (id: string, d: string) => {
      const x = source(id);
      const local = { date: d, datetime: `${d}T00:00:00`, offset_seconds: 0 };
      x.navigation = { ...x.navigation!, included: local };
      return included(x);
    };
    vi.mocked(bridge.listSources).mockResolvedValue(
      page([
        other("prev", "2024-02-28"),
        other("day", date),
        other("next", "2024-03-01"),
      ]),
    );
    await s.select(date);
    expect(s.inclusion("day")?.checked).toBe(true);
    expect(s.inclusion("prev")?.checked).toBe(false);
    expect(s.inclusion("next")?.checked).toBe(false);
  });
  it("publishes pages progressively, retries the same cursor and reconciles absence only at full coverage", async () => {
    const { s, bridge } = store();
    vi.mocked(bridge.listSources).mockResolvedValue(
      page([included(source("one")), included(source("two"))]),
    );
    await s.select(date);
    s.setIncluded(date, "one", false);
    vi.mocked(bridge.listSources)
      .mockResolvedValueOnce(page([included(source("one"))], 1))
      .mockRejectedValueOnce(new AuthenticError("unavailable", true))
      .mockResolvedValueOnce(page([included(source("three"))]));
    await s.refresh(date);
    expect(s.view(date).items).toHaveLength(1);
    expect(s.inclusion("two")?.checked).toBe(true);
    expect(s.inclusion("one")?.checked).toBe(false);
    await s.loadMore(date);
    expect(s.view(date).items).toHaveLength(1);
    expect(s.view(date).error).toBe("unavailable");
    await s.retry(date);
    expect(vi.mocked(bridge.listSources).mock.calls.slice(-2)).toEqual([
      ["import", 1],
      ["import", 1],
    ]);
    expect(s.inclusion("two")?.invalidated).toBe("missing");
    expect(s.inclusion("three")?.checked).toBe(true);
  });
  it("leaves old choices pending on incomplete scans and invalidates changed bytes", async () => {
    const { s, bridge } = store();
    await s.select(date);
    vi.mocked(bridge.listSources).mockResolvedValue({
      ...page([]),
      incomplete: true,
    });
    await s.refresh(date);
    expect(s.inclusion("source")?.checked).toBe(true);
    expect(s.eligible(date)).toEqual([]);
    vi.mocked(bridge.listSources).mockResolvedValue(
      page([included(source("source", "b".repeat(64)))]),
    );
    await s.refresh(date);
    expect(s.inclusion("source")).toMatchObject({
      checked: false,
      invalidated: "changed",
    });
    s.setIncluded(date, "source", true);
    expect(s.eligible(date)).toHaveLength(1);
  });
  it("rejects mismatched provenance, duplicate sources and nonadvancing cursors without replacing valid state", async () => {
    const { s, bridge } = store();
    await s.select(date);
    for (const invalid of [
      { ...page(), date: "2024-03-01" },
      page([included(source()), included(source())]),
      page([included(source())], 0),
      { ...page(), config_generation: 2 },
    ]) {
      vi.mocked(bridge.listSources).mockResolvedValue(invalid);
      await s.refresh(date);
      expect(s.view(date).error).toBe("invalid_response");
      expect(s.view(date).items).toHaveLength(1);
      expect(s.view(date).stale).toBe(true);
    }
  });
  it("records a saved authority separately when its required new import fails", async () => {
    const { s, bridge } = store();
    await s.select(date);
    vi.mocked(bridge.importDate).mockRejectedValue(
      new AuthenticError("unavailable", false),
    );
    await s.resolve(date, source(), "header");
    expect(bridge.resolveSourceTime).toHaveBeenCalledWith(
      "import",
      "source",
      "a".repeat(64),
      "header",
    );
    expect(s.view(date)).toMatchObject({
      resolutionSaved: true,
      error: "unavailable",
      stale: true,
    });
  });
  it("checks a newly resolved source while preserving a prior explicit exclusion", async () => {
    const { s, bridge } = store();
    const unresolved = { ...source(), eligible: false };
    vi.mocked(bridge.listSources).mockResolvedValueOnce(
      page([{ kind: "warning", source: unresolved, misplaced: false }]),
    );
    await s.select(date);
    expect(s.inclusion("source")?.checked).toBe(false);
    vi.mocked(bridge.listSources).mockResolvedValue(page());
    await s.refresh(date);
    expect(s.inclusion("source")?.checked).toBe(true);
    s.setIncluded(date, "source", false);
    vi.mocked(bridge.listSources).mockResolvedValueOnce(
      page([{ kind: "warning", source: unresolved, misplaced: false }]),
    );
    await s.refresh(date);
    vi.mocked(bridge.listSources).mockResolvedValue(page());
    await s.refresh(date);
    expect(s.inclusion("source")?.checked).toBe(false);
  });
  it("cancels an import requested while admission is still pending", async () => {
    const { s, bridge } = store();
    const pending =
      deferred<Awaited<ReturnType<NavigationBridge["importDate"]>>>();
    vi.mocked(bridge.importDate).mockReturnValueOnce(pending.promise);
    const selected = s.select(date);
    await Promise.resolve();
    await s.cancel(date);
    expect(bridge.cancelJob).not.toHaveBeenCalled();
    pending.resolve({
      import_id: "import",
      job: { id: "import", sequence: 1, state: "running" },
    });
    await selected;
    expect(bridge.cancelJob).toHaveBeenCalledWith("import");
    expect(s.view(date).phase).toBe("cancelled");
    expect(bridge.listSources).not.toHaveBeenCalled();
  });
  it("cancels a superseded admission without changing the newer date", async () => {
    const { s, bridge } = store();
    const pending =
      deferred<Awaited<ReturnType<NavigationBridge["importDate"]>>>();
    vi.mocked(bridge.importDate)
      .mockReturnValueOnce(pending.promise)
      .mockResolvedValueOnce({
        import_id: "next",
        job: { id: "next", sequence: 1, state: "cancelled" },
      });
    const old = s.select(date);
    await Promise.resolve();
    await s.select("2024-03-01");
    pending.resolve({
      import_id: "import",
      job: { id: "import", sequence: 1, state: "running" },
    });
    await old;
    expect(bridge.cancelJob).toHaveBeenCalledWith("import");
    expect(s.view("2024-03-01").phase).toBe("cancelled");
  });
  it("polls a new connection while an old lookup is still unresolved", async () => {
    vi.useFakeTimers();
    const { s, bridge } = store();
    const old = deferred<Awaited<ReturnType<NavigationBridge["getJob"]>>>();
    vi.mocked(bridge.importDate).mockResolvedValue({
      import_id: "import",
      job: { id: "import", sequence: 1, state: "running" },
    });
    vi.mocked(bridge.getJob)
      .mockReturnValueOnce(old.promise)
      .mockResolvedValueOnce({ id: "import", sequence: 2, state: "succeeded" });
    await s.select(date);
    await vi.advanceTimersByTimeAsync(50);
    expect(bridge.getJob).toHaveBeenCalledTimes(1);
    s.connect(null, 1);
    s.connect("server:session:2", 1);
    await s.select(date);
    await vi.advanceTimersByTimeAsync(50);
    expect(bridge.getJob).toHaveBeenCalledTimes(2);
    expect(s.view(date).phase).toBe("ready");
    old.resolve({ id: "import", sequence: 1, state: "failed" });
    await Promise.resolve();
    expect(s.view(date).phase).toBe("ready");
  });
  it("retries a terminal failed import with a new admission", async () => {
    const { s, bridge } = store();
    vi.mocked(bridge.importDate).mockResolvedValueOnce({
      import_id: "import",
      job: { id: "import", sequence: 1, state: "failed", error_code: "limit" },
    });
    await s.select(date);
    expect(s.view(date).phase).toBe("failed");
    await s.retry(date);
    expect(bridge.importDate).toHaveBeenCalledTimes(2);
    expect(s.view(date).phase).toBe("ready");
    expect(vi.mocked(bridge.importDate).mock.calls[0][1]).not.toBe(
      vi.mocked(bridge.importDate).mock.calls[1][1],
    );
  });
  it("refuses work when protected retained payloads fill the shared budget", async () => {
    const budget = new SessionBudget(10 * 1024 * 1024);
    budget.reserve("protected editor", 1);
    const { s, bridge } = store(fake(), budget);
    await s.select(date);
    expect(s.view(date).error).toBe("session_limit");
    expect(bridge.listSources).not.toHaveBeenCalled();
    expect(budget.used).toBe(1);
  });
});

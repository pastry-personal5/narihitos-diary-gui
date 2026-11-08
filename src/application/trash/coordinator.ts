import { AuthenticError, type Bridge } from "../bridge";
import { SessionBudget, payloadBytes } from "../navigation/budget";
import type { DeleteBufferBaseline } from "../navigation/resources";
import type {
  TrashConfirmRequest,
  TrashPreview,
  TrashPreviewRequest,
  TrashResult,
} from "./contracts";

export interface TrashEvents {
  prepare(
    sourceId: string,
    previous?: DeleteBufferBaseline,
  ): Promise<DeleteBufferBaseline | null | false>;
  reserve(sourceId: string, operationId: string): void;
  release(sourceId: string, operationId: string): void;
  reconcileReservations(unfinished: ReadonlySet<string>): void;
  moved(sourceId: string, baseline?: DeleteBufferBaseline | null): void;
  notice(text: string): void;
}
interface Target {
  sourceId: string;
  label: string;
  request: TrashPreviewRequest;
}
export type TrashDialog =
  | (Target & { phase: "preparing" })
  | (Target & {
      phase: "preview";
      preview: TrashPreview;
      baseline: DeleteBufferBaseline | null;
      expires: number;
      error: string | null;
    })
  | (Target & { phase: "error"; error: string })
  | { phase: "result"; operationId: string };
export interface TrashHandle {
  id: string;
  sourceId: string;
  label: string;
  phase: "submitting" | "checking" | "known";
  result?: TrashResult;
  error: string | null;
  request?: TrashConfirmRequest;
  baseline?: DeleteBufferBaseline | null;
  admittedCheckpoint?: number;
  submitting: boolean;
  polling: number | null;
  timer?: ReturnType<typeof setTimeout>;
}
const failure = (e: unknown) =>
  e instanceof AuthenticError ? e.code : "unavailable";
const rejected = new Set([
  "preview_expired",
  "preview_stale",
  "member_changed",
  "ownership_ambiguous",
  "trash_unsupported",
  "source_stale",
  "unsupported_format",
  "invalid_input",
  "limit",
  "resource_reserved",
  "checkpoint_mismatch",
  "operation_mismatch",
]);
/** Owns admitted operations independently of dialogs, rows and dates. Only the
 * server's durable member results invalidate buffers or selections. */
export class TrashCoordinator {
  readonly records = new Map<string, TrashHandle>();
  dialog: TrashDialog | null = null;
  preparing = false;
  online = false;
  writable = false;
  discovering = false;
  discoveryError: string | null = null;
  page: TrashResult[] = [];
  nextCursor: string | null = null;
  private epoch = 0;
  private dialogEpoch = 0;
  private disposed = false;
  private readonly listeners = new Set<() => void>();
  private readonly appliedMoves = new Set<string>();
  private readonly discoveryAllocations = new Set<string>();
  constructor(
    private readonly bridge: Bridge,
    private readonly budget: SessionBudget,
    private readonly events: TrashEvents,
    private readonly newId = () => crypto.randomUUID(),
    private readonly now = () => performance.now(),
    private readonly pollMs = 750,
  ) {}
  subscribe = (fn: () => void) => {
    this.listeners.add(fn);
    return () => {
      this.listeners.delete(fn);
    };
  };
  private emit() {
    if (!this.disposed) for (const fn of this.listeners) fn();
  }
  private current(epoch: number) {
    return !this.disposed && this.online && epoch === this.epoch;
  }
  get expired() {
    return (
      this.dialog?.phase === "preview" && this.now() >= this.dialog.expires
    );
  }
  get activity(): TrashHandle[] {
    const all = new Map<string, TrashHandle>(
      this.page.map((result) => [
        result.operation_id,
        {
          id: result.operation_id,
          sourceId: result.source_id,
          label: result.members[0].relative_label,
          phase: "known",
          result,
          error: null,
          submitting: false,
          polling: null,
        },
      ]),
    );
    for (const [id, record] of this.records) all.set(id, record);
    return [...all.values()];
  }
  async connect(online: boolean, writable = online) {
    this.online = online;
    this.writable = online && writable;
    const epoch = ++this.epoch;
    this.dismiss();
    for (const entry of this.records.values()) {
      clearTimeout(entry.timer);
      entry.polling = null;
      entry.submitting = false;
    }
    this.discovering = false;
    this.emit();
    if (!online) return;
    for (const entry of this.records.values()) {
      if (entry.request) void this.submit(entry, epoch);
      else void this.poll(entry, epoch);
    }
    await this.discover(epoch);
  }
  dismiss() {
    this.dialogEpoch++;
    this.dialog = null;
    this.preparing = false;
    this.budget.release("trash-preview");
    this.emit();
  }
  async start(
    sourceId: string,
    label: string,
    request: TrashPreviewRequest = {
      kind: "initial",
      operation_id: null,
      expected_checkpoint: 0,
    },
  ) {
    if (!this.writable || this.disposed || this.preparing) return;
    const epoch = this.epoch,
      serial = ++this.dialogEpoch;
    this.preparing = true;
    this.dialog = null;
    this.emit();
    const needsMarkdown =
      request.kind === "initial" ||
      this.activity.find((e) => e.id === request.operation_id)?.result
        ?.members[0].state !== "moved";
    const baseline = needsMarkdown ? await this.events.prepare(sourceId) : null;
    if (!this.current(epoch) || serial !== this.dialogEpoch) return;
    if (baseline === false) {
      this.preparing = false;
      this.emit();
      return;
    }
    const target = { sourceId, label, request };
    this.dialog = { ...target, phase: "preparing" };
    this.emit();
    try {
      const preview = await this.bridge.previewSourceDelete(sourceId, request);
      if (!this.current(epoch) || serial !== this.dialogEpoch) return;
      if (preview.expected_checkpoint !== request.expected_checkpoint)
        throw new AuthenticError("invalid_response", false);
      if (!this.budget.reserve("trash-preview", payloadBytes(preview)))
        throw new AuthenticError("session_limit", false);
      this.dialog = {
        ...target,
        phase: "preview",
        preview,
        baseline,
        expires: this.now() + preview.expires_in_ms,
        error: null,
      };
    } catch (e) {
      if (this.current(epoch) && serial === this.dialogEpoch)
        this.dialog = { ...target, phase: "error", error: failure(e) };
    } finally {
      if (serial === this.dialogEpoch) {
        this.preparing = false;
        this.emit();
      }
    }
  }
  refreshPreview() {
    const d = this.dialog;
    if (d && d.phase !== "result")
      return this.start(d.sourceId, d.label, d.request);
  }
  async confirm() {
    const d = this.dialog;
    if (d?.phase !== "preview" || this.preparing || !this.writable) return;
    if (this.expired) {
      d.error = "preview_expired";
      this.emit();
      return;
    }
    const epoch = this.epoch,
      serial = ++this.dialogEpoch;
    this.preparing = true;
    this.dialog = null;
    this.emit();
    const baseline = d.preview.ordinals.includes(0)
      ? await this.events.prepare(d.sourceId, d.baseline ?? undefined)
      : null;
    if (!this.current(epoch) || serial !== this.dialogEpoch) return;
    this.preparing = false;
    if (baseline === false) {
      this.dialog = d;
      this.emit();
      return;
    }
    if (this.now() >= d.expires) {
      this.dialog = { ...d, error: "preview_expired" };
      this.emit();
      return;
    }
    const id =
      d.request.kind === "continuation" ? d.request.operation_id : this.newId();
    const request: TrashConfirmRequest = {
      kind: d.request.kind,
      operation_id: id,
      step_id: this.newId(),
      expected_checkpoint: d.preview.expected_checkpoint,
      plan_hash: d.preview.plan_hash,
      ordinals: [...d.preview.ordinals],
      token: d.preview.token,
    };
    const entry: TrashHandle = {
      id,
      sourceId: d.sourceId,
      label: d.label,
      phase: "submitting",
      request,
      baseline,
      error: null,
      submitting: false,
      polling: null,
      result: this.records.get(id)?.result,
    };
    if (
      (!this.records.has(id) && this.records.size >= 128) ||
      !this.budget.reserve(`trash:${id}`, payloadBytes(request) + 4096)
    ) {
      this.dialog = { ...d, error: "session_limit" };
      this.emit();
      return;
    }
    this.records.set(id, entry);
    this.dialog = null;
    this.budget.release("trash-preview");
    this.events.reserve(entry.sourceId, id);
    this.events.notice(
      `${entry.label}: Move to Trash requested. Its result remains available in Trash Recovery.`,
    );
    this.emit();
    void this.poll(entry, epoch);
    await this.submit(entry, epoch);
  }
  private async submit(entry: TrashHandle, epoch: number) {
    if (!this.current(epoch) || !entry.request || entry.submitting) return;
    entry.submitting = true;
    const request = entry.request;
    this.emit();
    try {
      const ack = await this.bridge.confirmSourceDelete(
        entry.sourceId,
        request,
      );
      if (!this.current(epoch)) return;
      if (
        ack.operation_id !== entry.id ||
        ack.step_id !== request.step_id ||
        ack.admitted_checkpoint !== request.expected_checkpoint + 1
      )
        throw new AuthenticError("invalid_response", false);
      entry.request = undefined;
      entry.admittedCheckpoint = ack.admitted_checkpoint;
      entry.phase = "checking";
      entry.error = null;
    } catch (e) {
      if (!this.current(epoch)) return;
      const code = failure(e);
      entry.error = code;
      if (
        rejected.has(code) &&
        (!entry.result ||
          entry.result.checkpoint <= request.expected_checkpoint)
      ) {
        entry.request = undefined;
        clearTimeout(entry.timer);
        if (!entry.result) {
          this.records.delete(entry.id);
          this.budget.release(`trash:${entry.id}`);
          this.events.release(entry.sourceId, entry.id);
        } else entry.phase = "known";
        this.dialog = {
          phase: "error",
          sourceId: entry.sourceId,
          label: entry.label,
          request:
            request.kind === "initial"
              ? { kind: "initial", operation_id: null, expected_checkpoint: 0 }
              : {
                  kind: "continuation",
                  operation_id: entry.id,
                  expected_checkpoint: request.expected_checkpoint,
                },
          error: code,
        };
        this.events.notice(
          `${entry.label}: Move to Trash needs a new preview and explicit confirmation. Local text has been retained.`,
        );
      } else entry.phase = "checking";
    } finally {
      if (this.current(epoch)) {
        entry.submitting = false;
        this.emit();
        if (this.records.get(entry.id) === entry) void this.poll(entry, epoch);
      }
    }
  }
  private accept(result: TrashResult, entry?: TrashHandle) {
    if (
      entry &&
      (result.operation_id !== entry.id ||
        result.source_id !== entry.sourceId ||
        (entry.request && result.plan_hash !== entry.request.plan_hash))
    )
      throw new AuthenticError("invalid_response", false);
    const previous = entry?.result;
    if (
      entry?.admittedCheckpoint &&
      result.checkpoint < entry.admittedCheckpoint
    )
      return;
    if (
      previous &&
      (result.checkpoint < previous.checkpoint ||
        (previous.state === "succeeded" && result.state !== "succeeded") ||
        (result.checkpoint === previous.checkpoint &&
          previous.members.some(
            (m, i) =>
              m.state === "moved" && result.members[i].state !== "moved",
          )))
    )
      return;
    if (result.state !== "succeeded")
      this.events.reserve(result.source_id, result.operation_id);
    if (
      result.members[0].state === "moved" &&
      !this.appliedMoves.has(result.operation_id)
    ) {
      if (
        !this.budget.reserve("trash-effects", (this.appliedMoves.size + 1) * 80)
      )
        throw new AuthenticError("session_limit", false);
      this.appliedMoves.add(result.operation_id);
      this.events.moved(result.source_id, entry?.baseline);
    }
    if (result.state === "succeeded")
      this.events.release(result.source_id, result.operation_id);
    this.page = this.page.flatMap((item) =>
      item.operation_id !== result.operation_id
        ? [item]
        : result.state === "succeeded"
          ? []
          : [result],
    );
    if (entry) {
      entry.result = result;
      entry.error = null;
      if (
        entry.request &&
        result.checkpoint > entry.request.expected_checkpoint
      )
        entry.request = undefined;
      if (!entry.request)
        entry.phase = result.state === "running" ? "checking" : "known";
      if (previous?.state !== result.state)
        this.events.notice(
          `${entry.label}: ${result.state === "succeeded" ? "Move to Trash completed" : result.state === "needs_confirmation" ? "Move to Trash paused; inspect the member results before retrying" : result.state === "uncertain" ? "Trash outcome is uncertain; no automatic retry will run" : "Move to Trash is running"}.`,
        );
    }
  }
  private async poll(entry: TrashHandle, epoch: number) {
    if (
      !this.current(epoch) ||
      this.records.get(entry.id) !== entry ||
      entry.polling === epoch
    )
      return;
    entry.polling = epoch;
    clearTimeout(entry.timer);
    try {
      const operation = await this.bridge.getOperation(entry.id);
      if (!this.current(epoch) || this.records.get(entry.id) !== entry) return;
      if (operation.operation_id !== entry.id || !operation.trash)
        throw new AuthenticError("invalid_response", false);
      this.accept(operation.trash, entry);
    } catch (e) {
      if (this.current(epoch) && this.records.get(entry.id) === entry)
        entry.error = failure(e);
    } finally {
      if (this.current(epoch) && this.records.get(entry.id) === entry) {
        entry.polling = null;
        if (
          entry.request ||
          entry.submitting ||
          !entry.result ||
          (entry.admittedCheckpoint !== undefined &&
            entry.result.checkpoint < entry.admittedCheckpoint) ||
          entry.result.state === "running"
        )
          entry.timer = setTimeout(
            () => void this.poll(entry, epoch),
            this.pollMs,
          );
        this.emit();
      }
    }
  }
  inspect(id: string) {
    this.retainResult(id);
    this.dialog = { phase: "result", operationId: id };
    this.emit();
    const entry = this.records.get(id);
    if (entry && this.online) void this.poll(entry, this.epoch);
  }
  private retainResult(id: string) {
    if (this.records.has(id)) return;
    const result = this.page.find((r) => r.operation_id === id);
    if (!result) return;
    if (this.records.size >= 128 || !this.budget.reserve(`trash:${id}`, 4096)) {
      this.discoveryError = "session_limit";
      return;
    }
    this.records.set(id, {
      id,
      sourceId: result.source_id,
      label: result.members[0].relative_label,
      result,
      phase: "known",
      error: null,
      submitting: false,
      polling: null,
    });
  }
  retry(id: string) {
    this.retainResult(id);
    const entry = this.activity.find((e) => e.id === id),
      r = entry?.result;
    if (
      !entry ||
      !r ||
      r.state !== "needs_confirmation" ||
      !r.members.some((m) => m.retry_eligible) ||
      entry.request ||
      entry.submitting
    )
      return;
    return this.start(entry.sourceId, entry.label, {
      kind: "continuation",
      operation_id: id,
      expected_checkpoint: r.checkpoint,
    });
  }
  check(id: string) {
    const entry = this.records.get(id);
    if (entry && this.online) {
      if (entry.request && !entry.submitting)
        void this.submit(entry, this.epoch);
      void this.poll(entry, this.epoch);
    } else void this.refreshDiscovery();
  }
  dismissCompleted(id: string) {
    const entry = this.records.get(id);
    if (
      entry?.result?.state !== "succeeded" ||
      entry.request ||
      entry.submitting
    )
      return;
    clearTimeout(entry.timer);
    this.records.delete(id);
    this.budget.release(`trash:${id}`);
    this.emit();
  }
  private async discover(epoch: number) {
    // Older test bridges omit optional product features; the production bridge always
    // provides the three explicit Trash commands.
    if (!this.bridge.listSourceDeletions || !this.current(epoch)) return;
    this.discovering = true;
    this.discoveryError = null;
    this.emit();
    const started = this.now();
    let cursor: string | undefined;
    let count = 0;
    const unfinished = new Set<string>();
    const allocation = `trash-discovery:${epoch}`;
    this.discoveryAllocations.add(allocation);
    try {
      do {
        const page = await this.bridge.listSourceDeletions(cursor, 50);
        if (!this.current(epoch)) return;
        for (const result of page.operations)
          unfinished.add(result.operation_id);
        if (!this.budget.reserve(allocation, unfinished.size * 80))
          throw new AuthenticError("session_limit", false);
        for (const result of page.operations)
          this.accept(result, this.records.get(result.operation_id));
        if (!cursor) {
          if (!this.budget.reserve("trash-page", payloadBytes(page)))
            throw new AuthenticError("session_limit", false);
          this.page = page.operations;
          this.nextCursor = page.next_cursor;
          this.emit();
        }
        if (page.next_cursor === cursor)
          throw new AuthenticError("invalid_response", false);
        cursor = page.next_cursor ?? undefined;
        count++;
        if (cursor && (count >= 1000 || this.now() - started >= 120_000))
          throw new AuthenticError("limit", false);
      } while (cursor);
      if (count > 1) {
        const first = await this.bridge.listSourceDeletions(undefined, 50);
        if (!this.current(epoch)) return;
        for (const result of first.operations) {
          unfinished.add(result.operation_id);
          this.accept(result, this.records.get(result.operation_id));
        }
        if (!this.budget.reserve("trash-page", payloadBytes(first)))
          throw new AuthenticError("session_limit", false);
        this.page = first.operations;
        this.nextCursor = first.next_cursor;
      }
      for (const entry of this.records.values())
        if (
          entry.request ||
          entry.submitting ||
          entry.result?.state !== "succeeded"
        )
          unfinished.add(entry.id);
      this.events.reconcileReservations(unfinished);
    } catch (e) {
      if (this.current(epoch)) this.discoveryError = failure(e);
    } finally {
      this.budget.release(allocation);
      this.discoveryAllocations.delete(allocation);
      if (this.current(epoch)) {
        this.discovering = false;
        this.emit();
      }
    }
  }
  refreshDiscovery() {
    if (this.discovering) return;
    for (const entry of this.records.values())
      if (entry.result?.state !== "succeeded")
        void this.poll(entry, this.epoch);
    return this.discover(this.epoch);
  }
  async nextPage() {
    if (!this.online || this.discovering || !this.nextCursor) return;
    const epoch = this.epoch,
      cursor = this.nextCursor;
    this.discovering = true;
    this.emit();
    try {
      const page = await this.bridge.listSourceDeletions(cursor, 50);
      if (!this.current(epoch)) return;
      if (!this.budget.reserve("trash-page", payloadBytes(page)))
        throw new AuthenticError("session_limit", false);
      for (const result of page.operations)
        this.accept(result, this.records.get(result.operation_id));
      this.page = page.operations;
      this.nextCursor = page.next_cursor;
      this.discoveryError = null;
    } catch (e) {
      if (this.current(epoch)) this.discoveryError = failure(e);
    } finally {
      if (this.current(epoch)) {
        this.discovering = false;
        this.emit();
      }
    }
  }
  dispose() {
    this.disposed = true;
    this.epoch++;
    this.dialogEpoch++;
    for (const entry of this.records.values()) {
      clearTimeout(entry.timer);
      this.budget.release(`trash:${entry.id}`);
    }
    this.budget.release("trash-preview");
    this.budget.release("trash-page");
    this.budget.release("trash-effects");
    for (const allocation of this.discoveryAllocations)
      this.budget.release(allocation);
    this.discoveryAllocations.clear();
    this.listeners.clear();
  }
}

/** Export DTOs. The token lives only in a preview; remote contents never cross the bridge. */
export type ExportState =
  | "running"
  | "needs_confirmation"
  | "uncertain"
  | "succeeded"
  | "failed"
  | "cancelled";
export type ExportTarget = "icloud" | "notion";
export type ExportReason = "changed" | "deleted" | "unowned" | "uncertain";
export interface ExportResult {
  kind: "export";
  operation_id: string;
  target: ExportTarget;
  date: string;
  revision: { sequence: number; sha256: string };
  state: ExportState;
  checkpoint: number;
  reason: string | null;
}
export interface ExportPreview {
  operation_id: string;
  expected_checkpoint: number;
  token: string;
  reason: ExportReason;
  write: "create" | "replace";
  expires_in_ms: number;
}
export type StartExportOutcome =
  | { outcome: "needs_confirmation"; preview: ExportPreview }
  | { outcome: "result"; result: ExportResult };
export interface StartExportRequest {
  operation_id: string;
  date: string;
  target: ExportTarget;
  expected_revision: { sequence: number; sha256: string };
}
export interface ConfirmExportRequest {
  step_id: string;
  expected_checkpoint: number;
  token: string;
}
export interface ExportBridge {
  startExport?(request: StartExportRequest): Promise<StartExportOutcome>;
  confirmExport?(
    operationId: string,
    request: ConfirmExportRequest,
  ): Promise<{ admitted_checkpoint: number }>;
  abandonExport?(operationId: string): Promise<StartExportOutcome>;
  confirmAbandonExport?(
    operationId: string,
    request: ConfirmExportRequest,
  ): Promise<ExportResult>;
  listExports?(): Promise<ExportResult[]>;
}

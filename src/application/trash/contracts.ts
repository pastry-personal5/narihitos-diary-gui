/** Safe source-relative deletion DTOs. Tokens live only until confirmation; native
 * paths and receipts never cross the bridge. */
export type TrashState =
  "running" | "needs_confirmation" | "uncertain" | "succeeded";
export type TrashMemberState =
  "pending" | "moved" | "missing" | "failed" | "uncertain";
export type TrashMemberReason =
  | "unavailable"
  | "changed"
  | "unsafe_path"
  | "ownership_ambiguous"
  | "root_changed"
  | "uncertain";
export interface TrashMember {
  ordinal: 0 | 1;
  relative_label: string;
  state: TrashMemberState;
  reason: TrashMemberReason | null;
  retry_eligible: boolean;
}
export type TrashPreviewRequest =
  | { kind: "initial"; operation_id: null; expected_checkpoint: 0 }
  | { kind: "continuation"; operation_id: string; expected_checkpoint: number };
export interface TrashConfirmRequest {
  kind: "initial" | "continuation";
  operation_id: string;
  step_id: string;
  expected_checkpoint: number;
  plan_hash: string;
  ordinals: number[];
  token: string;
}
export interface TrashPreview {
  token: string;
  plan_hash: string;
  expected_checkpoint: number;
  expires_in_ms: number;
  ordinals: number[];
  members: TrashMember[];
}
export interface TrashAcknowledged {
  operation_id: string;
  step_id: string;
  admitted_checkpoint: number;
}
export interface TrashResult {
  kind: "source_trash";
  operation_id: string;
  source_id: string;
  plan_hash: string;
  checkpoint: number;
  state: TrashState;
  members: TrashMember[];
}
export interface TrashPage {
  operations: TrashResult[];
  next_cursor: string | null;
}
export interface TrashBridge {
  previewSourceDelete(
    sourceId: string,
    request: TrashPreviewRequest,
  ): Promise<TrashPreview>;
  confirmSourceDelete(
    sourceId: string,
    request: TrashConfirmRequest,
  ): Promise<TrashAcknowledged>;
  listSourceDeletions(cursor?: string, limit?: number): Promise<TrashPage>;
}

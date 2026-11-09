import type { GenerationBridge } from "./generation/contracts";
import type { ReviewBridge } from "./review/contracts";
import type { NavigationBridge } from "./navigation/contracts";
import type { TrashBridge, TrashResult } from "./trash/contracts";
import type { ExportBridge, ExportResult } from "./export/contracts";
import type { StreamEvent } from "./events";
/** Only redacted DTOs belong here. Discovery paths and credentials stay in Rust. */
export interface AppMetadata {
  version: string;
}
export interface Status {
  instance_id: string;
  workspace_key: string;
  server_version: string;
  api_version: 1;
  api_minor: number;
  storage_ready: boolean;
  capabilities: string[];
}
/** Events the WebView may report; the server-side codes are not reportable. */
export interface ClientEvent {
  code:
    | "ui.bridge_error"
    | "ui.render_error"
    | "ui.generation_preparing"
    | "ui.generation_dirty"
    | "ui.generation_saving"
    | "ui.generation_flushing"
    | "ui.generation_submitting"
    | "ui.generation_admitted"
    | "ui.generation_failed";
  level: "debug" | "warn" | "error";
  traceId?: string;
  detail?: {
    component: "editor" | "bridge" | "layout" | "settings";
    count?: number;
    error?:
      | "unavailable"
      | "internal"
      | "busy"
      | "limit"
      | "invalid"
      | "empty_selection"
      | "reimport_required"
      | "profile_not_ready"
      | "context"
      | "source_save"
      | "authentic_save"
      | "discard_required"
      | "cancelled";
  };
}
export interface Revision {
  sequence: number;
  sha256: string;
}
export interface AuthenticFile {
  date: string;
  exists: boolean;
  revision: Revision | null;
  body: string;
}
export interface SaveAck {
  operation_id: string;
  revision: Revision;
  buffer_generation: number;
}
export interface AuthenticPage {
  items: { date: string; revision: Revision | null; problem: string | null }[];
  next_cursor: string | null;
}
export interface RefreshReport {
  files: number;
  problems: number;
}
export interface OperationState {
  operation_id: string;
  kind?: string;
  checkpoint?: number;
  state: string;
  terminal: boolean;
  succeeded: boolean;
  result: string | null;
  trash?: TrashResult;
  export?: ExportResult;
}
export interface SaveRequest {
  date: string;
  operationId: string;
  expectedRevision: Revision | null;
  body: string;
  bufferGeneration: number;
}
/** Allowlisted failure of an Authentic command; never carries server text or paths. */
export class AuthenticError extends Error {
  constructor(
    readonly code: string,
    readonly retryable: boolean,
  ) {
    super(code);
  }
}
export interface Availability {
  alias: string;
  available: boolean;
  reason: string;
}
export interface ReloadIssue {
  path: string;
  code: string;
}
export interface ReloadResult {
  outcome: "applied" | "restart_required" | "rejected" | "busy";
  config_generation: number | null;
  issues: ReloadIssue[];
}
/** Redacted server config status: availability only, never paths, tokens or YAML. */
export interface ServerSettings {
  config_generation: number;
  last_reload: ReloadResult | null;
  storage_ready: boolean;
  sources: Availability[];
  profiles: Availability[];
}
export type ProfileState =
  | "unchecked"
  | "missing"
  | "unsupported_version"
  | "unavailable"
  | "available_auth_unknown"
  | "unauthenticated"
  | "ready"
  | "busy"
  | "failed";
export type ProfileReason =
  "assets_missing" | "assets_invalid" | "adapter_unimplemented";
export interface ProfileCapability {
  available: boolean;
  reason: ProfileReason | null;
}
/** Closed reasons behind a profile status; the list lives in profile-reasons.json. */
export type ProfileStatusReason = string;
/** Read-only agent profile status. Nothing here can start a run. */
export interface ProfileInfo {
  alias: string;
  type: string;
  status: ProfileState;
  /** `null` when ready, unchecked or not attributed. */
  reason: ProfileStatusReason | null;
  capabilities: { chat: ProfileCapability; generate: ProfileCapability };
}
/** The active editor snapshot sent with one chat message. */
export interface ChatDocumentSnapshot {
  kind: "authentic" | "source";
  date: string;
  unsaved: boolean;
  body: string;
}
export type ChatTurnState = "running" | "done" | "failed" | "cancelled";
export interface ChatTurn {
  job_id: string;
  state: ChatTurnState;
  /** Closed agent error code; never model text. */
  error_code: string | null;
  stage: string | null;
  revision: number;
  /** Provisional text while running; replaced by the validated reply in the transcript. */
  provisional: string | null;
}
export interface ChatMessage {
  role: "user" | "assistant";
  text: string;
}
export interface ChatSnapshot {
  chat_id: string;
  profile: string;
  type: string;
  locale: string;
  transcript: ChatMessage[];
  turn: ChatTurn | null;
}
export interface StatusLineSettings {
  generate_ready: boolean;
  review_ready: boolean;
  chat_ready: boolean;
}
export const DEFAULT_STATUS_LINE: StatusLineSettings = {
  generate_ready: true,
  review_ready: true,
  chat_ready: true,
};
export interface UserSettings {
  revision: string;
  read_only: boolean;
  recoverable: boolean;
  warnings: { section: string; code: string }[];
  writing_locale: string | null;
  generation_count: number;
  status_line?: StatusLineSettings;
  /** Profile aliases omitted from this map default to enabled. */
  agent_progress?: Record<string, boolean>;
  keybindings: unknown;
  /** Defaults merged with valid, conflict-free overrides, one list per action. */
  effective_keybindings: Record<string, string[]>;
  keybinding_issues: { id: string; code: string }[];
}
export interface UserSettingsUpdate {
  /** `writing_init` takes the system's preferred tags and only fills an empty choice. */
  section:
    | "writing"
    | "writing_init"
    | "keybindings"
    | "generation"
    | "status_line"
    | "agent_progress";
  value: unknown;
  expectedRevision: string;
  recover: boolean;
}
/** Allowlisted failure of a settings command; never carries paths or file contents. */
export class SettingsError extends Error {
  constructor(readonly code: string) {
    super(code);
  }
}
/** Result of the native SF Symbol lookup; `png_base64` is a template image to tint with CSS. */
export type SymbolResult =
  { status: "ok"; png_base64: string } | { status: "unsupported" };
export interface Bridge
  extends
    NavigationBridge,
    TrashBridge,
    GenerationBridge,
    ReviewBridge,
    ExportBridge {
  logClientEvent?(event: ClientEvent): Promise<void>;
  listJobs?(cursor?: string): Promise<JobPage>;
  metadata(): Promise<AppMetadata>;
  /** Absent in fakes; symbols then fall back to their text labels. */
  getSymbol?(name: string, size: number, scale: number): Promise<SymbolResult>;
  serverSettings(): Promise<ServerSettings>;
  reloadSettings(): Promise<ReloadResult>;
  revealConfigurationFolder?(): Promise<void>;
  /** Absent in fakes; Settings then omits the profile status list. */
  listProfiles?(): Promise<ProfileInfo[]>;
  checkProfile?(alias: string): Promise<ProfileInfo>;
  /** Absent in fakes; AI Chat then stays disabled. */
  openChat?(
    profile: string,
    locale: string,
    purpose?: "chat" | "paragraph_review",
  ): Promise<{ chat_id: string }>;
  sendChat?(
    chatId: string,
    requestId: string,
    message: string,
    document: ChatDocumentSnapshot | null,
  ): Promise<{ job_id: string }>;
  getChat?(chatId: string): Promise<ChatSnapshot>;
  closeChat?(chatId: string): Promise<void>;
  getUserSettings(): Promise<UserSettings>;
  updateUserSettings(update: UserSettingsUpdate): Promise<UserSettings>;
  connect(): Promise<Status>;
  status(): Promise<Status>;
  listAuthentic(cursor?: string): Promise<AuthenticPage>;
  loadAuthentic(date: string): Promise<AuthenticFile>;
  saveAuthentic(request: SaveRequest): Promise<SaveAck>;
  createEmptyAuthentic(date: string, operationId: string): Promise<SaveAck>;
  refreshAuthentic(): Promise<RefreshReport>;
  getOperation(operationId: string): Promise<OperationState>;
  /** The handler resolves true when the window may close (pending saves are settled). */
  onCloseRequested?(
    handler: () => Promise<boolean>,
    failed?: () => void,
  ): Promise<() => void>;
  watch(observer: (event: StreamEvent) => void): Promise<() => Promise<void>>;
}
export type Connection =
  | { kind: "disconnected"; message: string }
  | { kind: "connecting" }
  | { kind: "connected"; status: Status };

/** Body-less discovery page; event_cursor anchors replay after the first page. */
export interface JobPage {
  jobs: import("./events").SnapshotJob[];
  next_cursor: string | null;
  event_cursor: number;
}

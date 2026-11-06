import type { NavigationBridge } from "./navigation/contracts";
import type { TrashBridge, TrashResult } from "./trash/contracts";
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
  code: "ui.bridge_error" | "ui.render_error";
  level: "warn" | "error";
  traceId?: string;
  detail?: {
    component: "editor" | "bridge" | "layout" | "settings";
    count?: number;
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
export interface UserSettings {
  revision: string;
  read_only: boolean;
  recoverable: boolean;
  warnings: { section: string; code: string }[];
  writing_locale: string | null;
  keybindings: unknown;
  /** Defaults merged with valid, conflict-free overrides, one list per action. */
  effective_keybindings: Record<string, string[]>;
  keybinding_issues: { id: string; code: string }[];
}
export interface UserSettingsUpdate {
  /** `writing_init` takes the system's preferred tags and only fills an empty choice. */
  section: "writing" | "writing_init" | "keybindings";
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
export interface Bridge extends NavigationBridge, TrashBridge {
  metadata(): Promise<AppMetadata>;
  /** Absent in fakes; symbols then fall back to their text labels. */
  getSymbol?(name: string, size: number, scale: number): Promise<SymbolResult>;
  serverSettings(): Promise<ServerSettings>;
  reloadSettings(): Promise<ReloadResult>;
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
  onCloseRequested?(handler: () => Promise<boolean>): Promise<() => void>;
  watch(observer: (event: StreamEvent) => void): Promise<() => Promise<void>>;
}
export type Connection =
  | { kind: "disconnected"; message: string }
  | { kind: "connecting" }
  | { kind: "connected"; status: Status };

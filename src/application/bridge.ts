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
export interface Bridge {
  metadata(): Promise<AppMetadata>;
  connect(): Promise<Status>;
  status(): Promise<Status>;
  watch(observer: (event: StreamEvent) => void): Promise<() => Promise<void>>;
}
export type Connection =
  | { kind: "disconnected"; message: string }
  | { kind: "connecting" }
  | { kind: "connected"; status: Status };

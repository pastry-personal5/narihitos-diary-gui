import type { IllustratedEntry } from "../illustrated/contracts";
import type { AuthenticSession } from "../authentic/session";
import type { SourceBody } from "./contracts";
import type { ResourceBufferPort } from "../editor/buffers";

export interface EditorResource {
  instance: number;
  editGeneration: number;
  key: string;
  label: string;
  kind: "authentic" | "source" | "candidate" | "illustrated";
  illustrated?: IllustratedEntry;
  date?: string;
  buffer: ResourceBufferPort;
  session?: AuthenticSession;
  source?: Omit<SourceBody, "body" | "front_matter">;
  loading: boolean;
  error: string | null;
  dirty: boolean;
  stale: boolean;
  epoch: number;
  pending?: Promise<void>;
  /** Whole-file digest the next source save is based on: the one read or last saved. */
  baseDigest?: string;
  /** Kept while the sent text is unchanged so a retry reuses the operation ID. */
  saveOp?: { id: string; text: string };
  saving?: Promise<boolean>;
  saveError?: string | null;
  detached?: boolean;
}
export interface DeleteBufferBaseline {
  instance: number;
  generation: number;
  discarded: boolean;
}
export type SourceChoice = "save" | "discard" | "cancel";
export interface ResourceEvents {
  notice(text: string): void;
  sourceGuard(resource: EditorResource): Promise<SourceChoice>;
  savedDatesChanged(): void;
}

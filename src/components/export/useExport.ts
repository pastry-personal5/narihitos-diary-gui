import { useSyncExternalStore } from "react";
import type { ExportController } from "../../application/export/controller";
const NONE = {
  phase: "idle",
  contentKind: "authentic",
  date: null,
  target: "icloud",
  preview: null,
  uncertain: null,
  announcement: "",
} as const;
const off = () => () => {};
/** A missing controller (tests, fakes without export) reads as idle. */
export function useExport(controller?: ExportController) {
  return useSyncExternalStore(
    controller?.subscribe ?? off,
    controller?.getSnapshot ?? (() => NONE),
  );
}

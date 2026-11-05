import { useEffect, useRef } from "react";
import { mountScratchEditor } from "../editor/scratch-editor";
export function ScratchEditor() {
  const host = useRef<HTMLDivElement>(null);
  const buffer = useRef("");
  useEffect(() => {
    if (!host.current) return;
    return mountScratchEditor(host.current, buffer.current, (text) => {
      buffer.current = text;
    });
  }, []);
  return (
    <section aria-label="Tabbed Editor" className="editor-pane">
      <div className="pane-heading">
        <h2>Tabbed Editor</h2>
        <span>Scratch</span>
      </div>
      <p id="scratch-warning" className="notice">
        Synthetic scratch buffer · Never saved. Closing this window discards its
        text.
      </p>
      <div ref={host} className="editor-host" />
    </section>
  );
}

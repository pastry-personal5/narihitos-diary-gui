import type { ReactNode } from "react";

export function MainToolbarStrip({ children }: { children: ReactNode }) {
  return (
    <header aria-label="Main Toolbar Strip" className="main-toolbar-strip">
      <div className="toolbar-title">
        <h1>Narihito’s Diary</h1>
      </div>
      <div className="toolbar-actions">{children}</div>
    </header>
  );
}

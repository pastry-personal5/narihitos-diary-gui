import { useState, type ReactNode } from "react";
import { PaneSeparator } from "./PaneSeparator";

/** Owns pane geometry only; feature state survives resize and Settings visibility. */
export function MainContent({
  hidden,
  iconBar,
  explorer,
  tabbedEditor,
  aiChat,
}: {
  hidden: boolean;
  iconBar: ReactNode;
  explorer: ReactNode;
  tabbedEditor: ReactNode;
  aiChat: ReactNode;
}) {
  const [explorerWidth, setExplorerWidth] = useState(220);
  const [chatWidth, setChatWidth] = useState(480);
  return (
    <div
      className="main-content"
      aria-label="Main Content"
      hidden={hidden}
      style={{
        gridTemplateColumns: `48px ${explorerWidth}px 6px minmax(220px, 1fr) 6px ${chatWidth}px`,
      }}
    >
      {iconBar}
      {explorer}
      <PaneSeparator
        label="Resize Explorer"
        orientation="vertical"
        value={explorerWidth}
        min={160}
        max={480}
        change={setExplorerWidth}
      />
      {tabbedEditor}
      <PaneSeparator
        label="Resize AI Chat"
        orientation="vertical"
        value={chatWidth}
        min={160}
        max={480}
        change={setChatWidth}
        reverse
      />
      {aiChat}
    </div>
  );
}

export interface GlobalMessage {
  id: number;
  time: string;
  severity: "info" | "warning" | "error";
  text: string;
}
/** Accepts app-authored display strings, never an exception's message or backend diagnostics. */
export class GlobalMessagesStore {
  items: GlobalMessage[] = [];
  removed = 0;
  private nextId = 1;
  private readonly listeners = new Set<() => void>();
  constructor(private readonly now = () => new Date().toLocaleTimeString()) {}
  subscribe = (l: () => void) => {
    this.listeners.add(l);
    return () => {
      this.listeners.delete(l);
    };
  };
  add(
    text: string,
    severity: GlobalMessage["severity"] = "info",
    deduplicate = false,
  ) {
    if (
      deduplicate &&
      this.items.at(-1)?.text === text &&
      this.items.at(-1)?.severity === severity
    )
      return;
    this.items = [
      ...this.items,
      { id: this.nextId++, time: this.now(), severity, text },
    ];
    if (this.items.length > 500) {
      this.removed += this.items.length - 500;
      this.items = this.items.slice(-500);
    }
    for (const l of this.listeners) l();
  }
}

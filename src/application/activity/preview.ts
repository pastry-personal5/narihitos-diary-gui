/** A bounded owner supplies text after budget checks. Only mounted output subscribes. */
export class PreviewBuffer {
  private latest = "";
  private published = "";
  private last = -Infinity;
  private timer: ReturnType<typeof setTimeout> | undefined;
  private listeners = new Set<() => void>();
  constructor(private now = () => performance.now()) {}
  snapshot = () => this.published;
  subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
      if (!this.listeners.size && this.timer) {
        clearTimeout(this.timer);
        this.timer = undefined;
        this.published = this.latest;
      }
    };
  };
  update(text: string) {
    this.latest = text;
    if (!this.listeners.size) {
      this.published = text;
      return;
    }
    const delay = Math.max(0, 100 - (this.now() - this.last));
    if (!delay) this.flush();
    else if (!this.timer) this.timer = setTimeout(() => this.flush(), delay);
  }
  private flush() {
    this.timer = undefined;
    if (this.published === this.latest) return;
    this.published = this.latest;
    this.last = this.now();
    this.listeners.forEach((listener) => listener());
  }
  clear() {
    if (this.timer) clearTimeout(this.timer);
    this.timer = undefined;
    this.latest = this.published = "";
    this.last = -Infinity;
    this.listeners.forEach((listener) => listener());
  }
}

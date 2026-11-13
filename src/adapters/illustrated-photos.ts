import type { Bridge } from "../application/bridge";
import type { SessionBudget } from "../application/navigation/budget";

export interface PhotoLease {
  url: string;
  width: number;
  height: number;
  release(): void;
}
/** One transport at a time; retained PNGs share the session budget. Decoded RGBA has a separate 256 MiB ceiling. */
export class IllustratedPhotos {
  private tail: Promise<unknown> = Promise.resolve();
  private decoded = 0;
  private serial = 0;
  constructor(
    private bridge: Bridge,
    private budget: SessionBudget,
  ) {}
  acquire(
    date: string,
    version: string,
    n: number,
    signal: AbortSignal,
  ): Promise<PhotoLease> {
    const work = this.tail
      .catch(() => {})
      .then(async () => {
        if (signal.aborted || !this.bridge.readIllustratedPhoto)
          throw new Error("cancelled");
        const key = `illustrated-photo:${++this.serial}`;
        let pixels = 0,
          url: string | null = null,
          live = true;
        const release = () => {
          if (!live) return;
          live = false;
          if (url) URL.revokeObjectURL(url);
          this.decoded -= pixels;
          this.budget.release(key);
          signal.removeEventListener("abort", release);
        };
        if (!this.budget.reserve(key, 32 * 1024 * 1024))
          throw new Error("session_limit");
        try {
          const bytes = await this.bridge.readIllustratedPhoto(
            date,
            version,
            n,
          );
          if (signal.aborted) throw new Error("cancelled");
          if (bytes.length < 33 || bytes.length > 32 * 1024 * 1024)
            throw new Error("invalid_response");
          const header = new DataView(
            bytes.buffer,
            bytes.byteOffset,
            bytes.byteLength,
          );
          const width = header.getUint32(16),
            height = header.getUint32(20);
          pixels = width * height * (bytes[24] === 16 ? 8 : 4);
          if (
            ![137, 80, 78, 71, 13, 10, 26, 10].every(
              (v, i) => bytes[i] === v,
            ) ||
            header.getUint32(12) !== 0x49484452 ||
            width * height > 32_000_000 ||
            !width ||
            !height ||
            width > 8192 ||
            height > 8192 ||
            this.decoded + pixels > 256 * 1024 * 1024
          ) {
            pixels = 0;
            throw new Error("image_limit");
          }
          this.decoded += pixels;
          if (!this.budget.reserve(key, bytes.byteLength))
            throw new Error("session_limit");
          url = URL.createObjectURL(
            new Blob([new Uint8Array(bytes)], { type: "image/png" }),
          );
          signal.addEventListener("abort", release, { once: true });
          return { url, width, height, release };
        } catch (error) {
          release();
          throw error;
        }
      });
    this.tail = work;
    return work;
  }
}

import { createContext, useContext, useEffect, useState } from "react";
import type { SymbolResult } from "../application/bridge";

/** Native lookup plus the one-notice-per-name report; provided once by App. */
export interface SymbolHost {
  load(name: string, size: number, scale: number): Promise<SymbolResult>;
  unsupported(name: string): void;
}
const Host = createContext<SymbolHost | null>(null);
export const SymbolProvider = Host.Provider;

// In-memory only. The in-flight promise is cached so simultaneous mounts share one native call;
// a rejected call is evicted so a transient failure can retry on the next mount.
// `value` is filled once the call resolves so a remount can paint the symbol on first render.
interface Entry {
  promise: Promise<SymbolResult>;
  value?: SymbolResult;
}
const caches = new WeakMap<SymbolHost, Map<string, Entry>>();
function cacheOf(host: SymbolHost) {
  const cache = caches.get(host) ?? new Map<string, Entry>();
  caches.set(host, cache);
  return cache;
}
function lookup(host: SymbolHost, name: string, size: number, scale: number) {
  const cache = cacheOf(host);
  const key = `${name}|${size}|${scale}`;
  let hit = cache.get(key);
  if (!hit) {
    const entry: Entry = { promise: host.load(name, size, scale) };
    hit = entry;
    cache.set(key, entry);
    entry.promise.then(
      (value) => (entry.value = value),
      () => cache.delete(key),
    );
  }
  return hit.promise;
}
function peek(host: SymbolHost, name: string, size: number, scale: number) {
  const value = cacheOf(host).get(`${name}|${size}|${scale}`)?.value;
  return value?.status === "ok" ? value.png_base64 : null;
}

/**
 * Decoration only: the text label is always in the DOM (hidden while the symbol shows), so the
 * accessible name never depends on the native lookup. `fallback` is the visible text without it.
 */
export function SymbolLabel({
  name,
  label,
  fallback = label,
  size = 16,
}: {
  name: string;
  label: string;
  fallback?: string;
  size?: number;
}) {
  const host = useContext(Host);
  const scale = window.devicePixelRatio >= 2 ? 2 : 1;
  const key = `${name}|${size}|${scale}`;
  // Keyed so a changed name/size/scale never shows the previous symbol.
  const [loaded, setLoaded] = useState<{ key: string; png: string } | null>(
    null,
  );
  const png =
    (host && peek(host, name, size, scale)) ??
    (loaded?.key === key ? loaded.png : null);
  useEffect(() => {
    if (!host) return;
    let live = true;
    lookup(host, name, size, scale)
      .then((result) => {
        if (!live) return;
        if (result.status === "ok") setLoaded({ key, png: result.png_base64 });
        else host.unsupported(name);
      })
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [host, name, size, scale, key]);
  if (!png) return <>{fallback}</>;
  const mask = `url(data:image/png;base64,${png})`;
  return (
    <>
      <span
        aria-hidden="true"
        className="symbol"
        style={{
          width: size,
          height: size,
          maskImage: mask,
          WebkitMaskImage: mask,
        }}
      />
      <span className="visually-hidden">{label}</span>
    </>
  );
}

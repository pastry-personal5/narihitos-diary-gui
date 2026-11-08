import { useRef, useState } from "react";
export function PaneSeparator({
  label,
  orientation,
  value,
  min,
  max,
  change,
  reverse = false,
}: {
  label: string;
  orientation: "horizontal" | "vertical";
  value: number;
  min: number;
  max: number;
  change(value: number): void;
  reverse?: boolean;
}) {
  const [dragging, setDragging] = useState(false);
  const drag = useRef<{ pointer: number; start: number; value: number } | null>(
    null,
  );
  const clamp = (n: number) => Math.max(min, Math.min(max, n));
  const coordinate = (e: React.PointerEvent) =>
    orientation === "vertical" ? e.clientX : e.clientY;
  const direction = reverse ? -1 : 1;
  return (
    <div
      role="separator"
      aria-label={label}
      aria-orientation={orientation}
      aria-valuemin={min}
      aria-valuemax={max}
      aria-valuenow={Math.round(value)}
      tabIndex={0}
      data-dragging={dragging || undefined}
      className={`pane-separator ${orientation}`}
      onKeyDown={(e) => {
        const delta =
          orientation === "vertical"
            ? e.key === "ArrowLeft"
              ? -10
              : e.key === "ArrowRight"
                ? 10
                : 0
            : e.key === "ArrowUp"
              ? -10
              : e.key === "ArrowDown"
                ? 10
                : 0;
        if (delta && !e.nativeEvent.isComposing) {
          e.preventDefault();
          change(clamp(value + delta * direction));
        }
      }}
      onPointerDown={(e) => {
        if (e.button !== 0) return;
        setDragging(true);
        drag.current = { pointer: e.pointerId, start: coordinate(e), value };
        e.currentTarget.setPointerCapture(e.pointerId);
      }}
      onPointerMove={(e) => {
        const state = drag.current;
        if (state?.pointer === e.pointerId)
          change(
            clamp(state.value + (coordinate(e) - state.start) * direction),
          );
      }}
      onPointerUp={(e) => {
        if (drag.current?.pointer === e.pointerId) {
          drag.current = null;
          setDragging(false);
          e.currentTarget.releasePointerCapture(e.pointerId);
        }
      }}
      onPointerCancel={() => {
        drag.current = null;
        setDragging(false);
      }}
      onLostPointerCapture={() => {
        drag.current = null;
        setDragging(false);
      }}
    />
  );
}

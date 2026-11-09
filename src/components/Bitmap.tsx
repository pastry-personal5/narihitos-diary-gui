/** App-owned bitmap mask, tinted by the surrounding control's text color. */
export function Bitmap({ src, size = 18 }: { src: string; size?: number }) {
  return (
    <span
      aria-hidden="true"
      className="bitmap-icon"
      style={{
        width: size,
        height: size,
        maskImage: `url(${src})`,
        WebkitMaskImage: `url(${src})`,
      }}
    />
  );
}

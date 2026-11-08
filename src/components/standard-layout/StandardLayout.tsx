import type { ReactNode } from "react";

/** Window structure only; controller and session ownership stays in App. */
export function StandardLayout({
  closing,
  children,
}: {
  closing: boolean;
  children: ReactNode;
}) {
  return (
    <main
      inert={closing}
      aria-label="Standard Layout"
      className="standard-layout"
    >
      {children}
    </main>
  );
}

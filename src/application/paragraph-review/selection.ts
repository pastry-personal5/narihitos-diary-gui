import type { ParagraphSnapshot } from "./contracts";

/** Prose targeting policy. Editor adapters supply text and caret; no UI dependencies. */
export function paragraphAt(
  body: string,
  caret: number,
): ParagraphSnapshot | null {
  if (!Number.isInteger(caret) || caret < 0 || caret > body.length) return null;
  let from = body.lastIndexOf("\n", caret - 1) + 1;
  if (caret === 0) from = 0;
  let to = body.indexOf("\n", caret);
  if (to < 0) to = body.length;
  if (!body.slice(from, to).trim()) return null;
  while (from > 0) {
    const previous = body.lastIndexOf("\n", from - 2) + 1;
    if (!body.slice(previous, from - 1).trim()) break;
    from = previous;
  }
  while (to < body.length) {
    const next = body.indexOf("\n", to + 1);
    const end = next < 0 ? body.length : next;
    if (!body.slice(to + 1, end).trim()) break;
    to = end;
  }
  return { body, from, to, text: body.slice(from, to) };
}

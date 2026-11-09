import codex from "../../../assets/prompts/codex/review-paragraph.md?raw";
import claude from "../../../assets/prompts/claude/review-paragraph.md?raw";
import qwen from "../../../assets/prompts/qwen/review-paragraph.md?raw";
import apple from "../../../assets/prompts/apple-foundation-models/review-paragraph.md?raw";

const prompts: Record<string, string> = {
  codex,
  claude,
  qwen,
  "apple-foundation-models": apple,
};
/** Fail closed for unknown adapters or a malformed asset; no hard-coded fallback. */
export function paragraphReviewPrompt(
  type: string,
  paragraph: string,
  context: import("../../application/paragraph-review/contracts").ParagraphReviewSnippet[],
  locale: string,
): string {
  const template = prompts[type];
  const header = `---\nprovider: ${type}\nmodel: default\nreasoning_effort: ${type === "apple-foundation-models" ? "not-applicable" : "default"}\n---\n`;
  if (!template?.startsWith(header)) throw new Error("prompt_unavailable");
  const body = template.slice(header.length).trim();
  if (
    body.split("{{data}}").length !== 2 ||
    !body.includes("{{locale}}") ||
    /\{\{(?!data\}\}|locale\}\})/u.test(body)
  )
    throw new Error("prompt_unavailable");
  return body.replace(/\{\{data\}\}|\{\{locale\}\}/gu, (token) =>
    token === "{{data}}" ? JSON.stringify({ paragraph, context }) : locale,
  );
}

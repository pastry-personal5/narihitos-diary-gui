import { readFileSync } from "node:fs";
import { expect, it } from "vitest";

const css = readFileSync("src/styles.css", "utf8");
const root = css.slice(
  css.indexOf(":root"),
  css.indexOf("}\n", css.indexOf(":root")) + 1,
);
const rest = css.replace(root, "");

it("defines every color once, in the :root token block", () => {
  expect(rest).not.toMatch(
    /#[0-9a-fA-F]{3,8}\b|\brgba?\(|\bhsla?\(|:\s*(white|black)\b/,
  );
  expect(css).not.toMatch(/prefers-color-scheme/);
});

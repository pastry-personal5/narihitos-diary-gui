import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { expect, it } from "vitest";

it("preserves collected license bytes and exact resolved npm versions", () => {
  const inventory = JSON.parse(
    readFileSync("assets/third-party/inventory.json", "utf8"),
  );
  const lock = JSON.parse(readFileSync("package-lock.json", "utf8"));
  const npm = inventory.filter((item) => item.ecosystem === "npm");
  expect(npm.map((item) => `${item.name}@${item.version}`).sort()).toEqual(
    Object.entries(lock.packages)
      .filter(([path]) => path)
      .map(
        ([path, item]) =>
          `${item.name || path.split("node_modules/").at(-1)}@${item.version}`,
      )
      .sort(),
  );
  for (const item of inventory) {
    expect(typeof item.license).toBe("string");
    expect(item.selected_licenses.length).toBeGreaterThan(0);
    expect(item.selected_licenses.some((value) => value.includes("LGPL"))).toBe(
      false,
    );
    if (item.ecosystem === "cargo" || item.installed)
      expect(item.notices.length).toBeGreaterThan(0);
    for (const notice of item.notices) {
      expect(notice.file).toMatch(/^licenses\/[a-f0-9]{64}\.txt$/u);
      const bytes = readFileSync(`assets/third-party/${notice.file}`);
      expect(createHash("sha256").update(bytes).digest("hex")).toBe(
        notice.sha256,
      );
      if (notice.source_url)
        expect(notice.source_url).toMatch(
          /^https:\/\/raw\.githubusercontent\.com\/[^/]+\/[^/]+\/[a-f0-9]{40}\//u,
        );
    }
  }
});

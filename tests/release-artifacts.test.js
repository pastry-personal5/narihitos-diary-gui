import { createHash } from "node:crypto";
import { afterEach, expect, test } from "vitest";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { verifyRelease } from "../scripts/release-artifacts.mjs";
const dirs = [];
afterEach(() =>
  dirs.splice(0).forEach((p) => rmSync(p, { recursive: true, force: true })),
);
function fixture() {
  const root = mkdtempSync(join(tmpdir(), "diary artifact "));
  dirs.push(root);
  const app = join(root, "Diary.app");
  const resources = join(app, "Contents/Resources");
  mkdirSync(join(resources, "third-party/licenses"), { recursive: true });
  mkdirSync(join(app, "Contents/MacOS"));
  const server = join(root, "server"),
    client = join(root, "client"),
    notices = join(root, "notices");
  writeFileSync(
    server,
    "celebrated, award-winning novelist careful writing companion for a private diary",
  );
  writeFileSync(client, "ordinary client");
  writeFileSync(join(app, "Contents/MacOS/diary-client"), "ordinary client");
  writeFileSync(notices, "notices ".repeat(30));
  writeFileSync(
    join(resources, "third-party-notices.md"),
    "notices ".repeat(30),
  );
  const sha256 = createHash("sha256").update("license").digest("hex");
  writeFileSync(
    join(resources, `third-party/licenses/${sha256}.txt`),
    "license",
  );
  for (const name of ["inventory.json", "supplemental-notices.json"]) {
    writeFileSync(
      join(resources, "third-party", name),
      JSON.stringify([
        { notices: [{ file: `licenses/${sha256}.txt`, sha256 }] },
      ]),
    );
  }
  return {
    server,
    client,
    app,
    notices,
    inspect: () => "arm64",
    plist: (_, key) =>
      key === "CFBundleExecutable" ? "diary-client" : "1.0.0",
  };
}
test("records actual digests and distinguishes helper fallback", () => {
  const result = verifyRelease(fixture());
  expect(result.artifacts).toHaveLength(7);
  expect(result.artifacts[0].sha256).toMatch(/^[a-f0-9]{64}$/);
  expect(result.helper).toContain("unavailable");
});
test.each([
  "scaffold-fake-request",
  "DIARY_FAKE_EXPORT_DIR",
  "DIARY_FAKE_NOTION_FILE",
])("rejects server contamination %s", (marker) => {
  const f = fixture();
  writeFileSync(f.server, marker);
  expect(() => verifyRelease(f)).toThrow("Fixture marker");
});
test("rejects missing resources, version drift, architecture drift and client mismatch", () => {
  const f = fixture();
  expect(() => verifyRelease({ ...f, inspect: () => "x86_64" })).toThrow(
    "arm64",
  );
  expect(() => verifyRelease({ ...f, version: "2.0.0" })).toThrow("version");
  writeFileSync(f.client, "different");
  expect(() => verifyRelease(f)).toThrow("clients differ");
  rmSync(f.server);
  expect(() => verifyRelease(f)).toThrow();
});
test("rejects omitted notices and prompt assets", () => {
  const f = fixture();
  writeFileSync(f.server, "ordinary");
  expect(() => verifyRelease(f)).toThrow("prompt");
  rmSync(f.notices);
  expect(() => verifyRelease(f)).toThrow();
});

test("rejects bundled client fixtures and absent license resources", () => {
  const f = fixture();
  const bundled = join(f.app, "Contents/MacOS/diary-client");
  writeFileSync(bundled, "SMOKE main_cancelled_job_read");
  expect(() => verifyRelease(f)).toThrow("Fixture marker");
  writeFileSync(bundled, "ordinary client");
  rmSync(join(f.app, "Contents/Resources/third-party/licenses"), {
    recursive: true,
  });
  expect(() => verifyRelease(f)).toThrow();
});

test("rejects corrupted packaged licenses even when the directory is populated", () => {
  const f = fixture();
  const sha256 = createHash("sha256").update("license").digest("hex");
  writeFileSync(
    join(f.app, "Contents/Resources/third-party/licenses", `${sha256}.txt`),
    "changed license",
  );
  expect(() => verifyRelease(f)).toThrow("License digest mismatch");
});

test("rejects a missing referenced license despite unrelated remaining files", () => {
  const f = fixture();
  const root = join(f.app, "Contents/Resources/third-party/licenses");
  const sha256 = createHash("sha256").update("license").digest("hex");
  rmSync(join(root, `${sha256}.txt`));
  writeFileSync(join(root, "unrelated.txt"), "unrelated");
  expect(() => verifyRelease(f)).toThrow();
});

test("rejects inventories stripped of license references", () => {
  const f = fixture();
  writeFileSync(join(f.app, "Contents/Resources/third-party/inventory.json"), JSON.stringify([{ notices: [] }]));
  expect(() => verifyRelease(f)).toThrow("No license references");
});

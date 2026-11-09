import { execFileSync } from "node:child_process";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import process from "node:process";
import { afterEach, expect, test } from "vitest";

const directories = [];
afterEach(() => {
  for (const directory of directories.splice(0))
    rmSync(directory, { recursive: true, force: true });
});

function artifacts() {
  const target = mkdtempSync(join(tmpdir(), "diary debug artifacts "));
  directories.push(target);
  for (const name of [
    "diary-server",
    "diary-client",
    "bundle/macos/Narihito's Diary.app/Contents/MacOS/diary-client",
  ]) {
    const path = join(target, "debug", name);
    mkdirSync(dirname(path), { recursive: true });
    writeFileSync(path, "ordinary artifact");
  }
  return target;
}

function run(args, env = process.env) {
  return execFileSync(
    process.execPath,
    [resolve("scripts/check-fixture-exclusion.mjs"), ...args],
    {
      env,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    },
  );
}

test("debug scan respects explicit and environment target directories containing spaces", () => {
  const target = artifacts();
  expect(run(["--target-dir", target])).toContain("exclude fixture markers");
  expect(run([], { ...process.env, CARGO_TARGET_DIR: target })).toContain(
    "exclude fixture markers",
  );
  writeFileSync(join(target, "debug/diary-server"), "DIARY_FAULT");
  expect(() => run(["--target-dir", target])).toThrow("Fixture marker");
});

test("explicit artifact mode requires every artifact and rejects mixed debug options", () => {
  expect(() => run(["--server", "absent"])).toThrow("Missing --client");
  expect(() =>
    run(["--server", "absent", "--target-dir", artifacts()]),
  ).toThrow("cannot be combined");
});

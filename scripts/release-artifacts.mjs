import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";

export const fixtureMarkers = {
  client: [
    "SMOKE main_cancelled_job_read",
    "smoke-unexpected-allow",
    "SMOKE illustrated_gui",
    "DIARY_ILLUSTRATED_GUI_SMOKE",
  ],
  server: [
    "DIARY_FAULT",
    "scaffold-fake-request",
    "scaffold-fake-job",
    "SMOKE server_folder_readable",
    "DIARY_FAKE_EXPORT_DIR",
    "DIARY_FAKE_NOTION_FILE",
  ],
};
export function scan(path, kind) {
  const bytes = readFileSync(path);
  for (const marker of fixtureMarkers[kind]) {
    if (bytes.includes(marker))
      throw new Error(`Fixture marker in ${path}: ${marker}`);
  }
  return { path, sha256: createHash("sha256").update(bytes).digest("hex") };
}
export function verifyRelease({
  server,
  client,
  app,
  notices,
  version = "1.0.0",
  inspect = (path) =>
    execFileSync("/usr/bin/lipo", ["-archs", path], {
      encoding: "utf8",
    }).trim(),
  plist = (path, key) =>
    execFileSync("/usr/libexec/PlistBuddy", ["-c", `Print :${key}`, path], {
      encoding: "utf8",
    }).trim(),
}) {
  const info = join(app, "Contents/Info.plist");
  if (plist(info, "CFBundleShortVersionString") !== version)
    throw new Error("Bundle version mismatch");
  const bundled = join(
    app,
    "Contents/MacOS",
    plist(info, "CFBundleExecutable"),
  );
  const records = [];
  for (const [path, kind] of [
    [server, "server"],
    [client, "client"],
    [bundled, "client"],
  ]) {
    if (inspect(path) !== "arm64") throw new Error(`Expected arm64: ${path}`);
    records.push(scan(path, kind));
  }
  if (records[1].sha256 !== records[2].sha256)
    throw new Error("Standalone and bundled clients differ");
  const resources = join(app, "Contents/Resources");
  function tree(path) {
    for (const entry of readdirSync(path, { withFileTypes: true })) {
      const child = join(path, entry.name);
      if (entry.isDirectory()) tree(child);
      else if (entry.isFile()) records.push(scan(child, "client"));
      else throw new Error(`Unexpected resource type: ${child}`);
    }
  }
  const notice = readFileSync(notices);
  if (
    notice.length < 100 ||
    !notice.equals(readFileSync(join(resources, "third-party-notices.md")))
  )
    throw new Error("Missing or differing notices");
  if (!readdirSync(join(resources, "third-party/licenses")).length)
    throw new Error("Missing licenses");
  const licenseRoot = join(resources, "third-party");
  for (const name of ["inventory.json", "supplemental-notices.json"]) {
    const inventory = JSON.parse(readFileSync(join(licenseRoot, name), "utf8"));
    if (!Array.isArray(inventory) || !inventory.length)
      throw new Error(`Empty notice inventory: ${name}`);
    let referenced = 0;
    for (const item of inventory) {
      if (!Array.isArray(item.notices))
        throw new Error(`Invalid notice inventory: ${name}`);
      for (const notice of item.notices) {
        referenced++;
        if (
          !/^licenses\/[a-f0-9]{64}\.txt$/u.test(notice.file) ||
          notice.file !== `licenses/${notice.sha256}.txt`
        )
          throw new Error("Invalid license reference");
        const digest = createHash("sha256")
          .update(readFileSync(join(licenseRoot, notice.file)))
          .digest("hex");
        if (digest !== notice.sha256)
          throw new Error(`License digest mismatch: ${notice.file}`);
      }
    }
    if (!referenced) throw new Error(`No license references: ${name}`);
  }
  tree(resources);
  const bytes = readFileSync(server);
  for (const phrase of [
    "celebrated, award-winning novelist",
    "careful writing companion for a private diary",
  ]) {
    if (!bytes.includes(phrase))
      throw new Error(`Missing server prompt: ${phrase}`);
  }
  return {
    version,
    artifacts: records,
    helper: bytes.includes("FoundationModels.framework")
      ? "embedded; runtime probe required"
      : "unavailable; not live integration",
  };
}

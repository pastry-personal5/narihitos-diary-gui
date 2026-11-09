import { parseArgs } from "node:util";
import { join } from "node:path";
import process from "node:process";
import { scan, verifyRelease } from "./release-artifacts.mjs";
const { values } = parseArgs({
  options: Object.fromEntries(
    ["server", "client", "app", "notices", "target-dir"].map((key) => [
      key,
      { type: "string" },
    ]),
  ),
});
if (
  ["server", "client", "app", "notices"].some(
    (key) => values[key] !== undefined,
  )
) {
  if (values["target-dir"] !== undefined)
    throw new Error(
      "--target-dir cannot be combined with explicit release artifacts",
    );
  for (const key of ["server", "client", "app", "notices"])
    if (!values[key]) throw new Error(`Missing --${key}`);
  console.log(JSON.stringify(verifyRelease(values), null, 2));
} else {
  const debug = join(
    values["target-dir"] ?? process.env.CARGO_TARGET_DIR ?? "src-tauri/target",
    "debug",
  );
  for (const [path, kind] of [
    [join(debug, "diary-client"), "client"],
    [join(debug, "diary-server"), "server"],
    [
      join(
        debug,
        "bundle/macos/Narihito's Diary.app/Contents/MacOS/diary-client",
      ),
      "client",
    ],
  ])
    scan(path, kind);
  console.log("Ordinary debug binaries exclude fixture markers");
}

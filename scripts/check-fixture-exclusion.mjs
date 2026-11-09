import { parseArgs } from "node:util";
import { scan, verifyRelease } from "./release-artifacts.mjs";
const { values } = parseArgs({
  options: Object.fromEntries(
    ["server", "client", "app", "notices"].map((key) => [
      key,
      { type: "string" },
    ]),
  ),
});
if (Object.keys(values).length) {
  for (const key of ["server", "client", "app", "notices"])
    if (!values[key]) throw new Error(`Missing --${key}`);
  console.log(JSON.stringify(verifyRelease(values), null, 2));
} else {
  for (const [path, kind] of [
    ["src-tauri/target/debug/diary-client", "client"],
    ["src-tauri/target/debug/diary-server", "server"],
    [
      "src-tauri/target/debug/bundle/macos/Narihito's Diary.app/Contents/MacOS/diary-client",
      "client",
    ],
  ])
    scan(path, kind);
  console.log("Ordinary debug binaries exclude fixture markers");
}

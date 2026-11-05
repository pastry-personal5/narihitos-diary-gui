import { readFileSync } from "node:fs";
const targets = [
  [
    "src-tauri/target/debug/diary-client",
    ["SMOKE main_cancelled_job_read", "smoke-unexpected-allow"],
  ],
  [
    "src-tauri/target/debug/diary-server",
    [
      "scaffold-fake-request",
      "scaffold-fake-job",
      "SMOKE server_folder_readable",
    ],
  ],
  [
    "src-tauri/target/debug/bundle/macos/Narihito's Diary.app/Contents/MacOS/diary-client",
    ["SMOKE main_cancelled_job_read", "smoke-unexpected-allow"],
  ],
];
for (const [path, markers] of targets) {
  const bytes = readFileSync(path);
  for (const marker of markers)
    if (bytes.includes(marker))
      throw new Error(`Fixture marker present in ordinary binary: ${path}`);
}
console.log(
  "Ordinary client bundle and standalone binaries exclude fixture markers",
);

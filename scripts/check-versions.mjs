import { URL } from "node:url";
import { readFileSync } from "node:fs";
import { assertVersions } from "./version-policy.mjs";
const app = JSON.parse(
  readFileSync(new URL("../package.json", import.meta.url)),
);
const cargo = readFileSync(
  new URL("../src-tauri/Cargo.toml", import.meta.url),
  "utf8",
);
const tauri = JSON.parse(
  readFileSync(new URL("../src-tauri/tauri.conf.json", import.meta.url)),
);
console.log(assertVersions(app, cargo, tauri));

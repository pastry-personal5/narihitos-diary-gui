export function assertVersions(app, cargo, tauri) {
  const version = cargo.match(
    /\[workspace.package\][\s\S]*?version = "([^"]+)"/u,
  )?.[1];
  if (version !== app.version || tauri.version !== app.version)
    throw new Error("Cargo/package/Tauri version drift");
  if (app.version !== "1.0.0") throw new Error("Unexpected scaffold version");
  return `Version v${app.version}`;
}

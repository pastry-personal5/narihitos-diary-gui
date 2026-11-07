import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const metadata = JSON.parse(
  execFileSync(
    "cargo",
    [
      "metadata",
      "--manifest-path",
      "src-tauri/Cargo.toml",
      "--no-deps",
      "--format-version",
      "1",
    ],
    { encoding: "utf8" },
  ),
);
const allowed = {
  "diary-domain": [],
  "diary-application": ["diary-domain"],
  "diary-logging": ["diary-application"],
  "diary-infrastructure": [
    "diary-application",
    "diary-domain",
    "diary-logging",
  ],
  "diary-api": ["diary-application", "diary-domain"],
  "diary-server": [
    "diary-application",
    "diary-infrastructure",
    "diary-api",
    "diary-logging",
  ],
  "diary-client": ["diary-application", "diary-logging"],
};

describe("scaffold assembly", () => {
  it("selects all seven packages in the unchanged Cargo gate", () => {
    expect(metadata.packages.map((p) => p.name).sort()).toEqual(
      Object.keys(allowed).sort(),
    );
    expect([...metadata.workspace_default_members].sort()).toEqual(
      [...metadata.workspace_members].sort(),
    );
    for (const name of ["diary-client", "diary-server"]) {
      const pkg = metadata.packages.find((p) => p.name === name);
      expect(
        pkg.targets.some(
          (target) => target.kind.includes("bin") && target.name === name,
        ),
      ).toBe(true);
    }
  });

  it("keeps every dependency kind pointing inward", () => {
    for (const pkg of metadata.packages) {
      for (const dep of pkg.dependencies) {
        if (dep.name.startsWith("diary-"))
          expect(allowed[pkg.name]).toContain(dep.name);
        else if (["diary-domain", "diary-application"].includes(pkg.name))
          expect(
            pkg.name === "diary-domain"
              ? ["unicode-normalization", "pulldown-cmark"]
              : [],
          ).toContain(dep.name);
      }
    }
  });

  it("inherits the release version throughout Cargo", () => {
    const app = JSON.parse(readFileSync("package.json", "utf8"));
    for (const pkg of metadata.packages) expect(pkg.version).toBe(app.version);
    expect(
      execFileSync("node", ["scripts/check-versions.mjs"], {
        encoding: "utf8",
      }).trim(),
    ).toBe("Version v1.0.0");
  });
});

describe("explicit command permission boundary", () => {
  const expected = [
    "connect_server",
    "get_status",
    "open_session",
    "heartbeat_session",
    "close_session",
    "get_settings",
    "reload_settings",
    "get_user_settings",
    "update_user_settings",
    "get_job",
    "list_jobs",
    "cancel_job",
    "subscribe_events",
    "get_app_metadata",
    "log_client_event",
    "get_symbol",
    "get_calendar",
    "list_agents",
    "check_agent",
    "open_chat",
    "send_chat",
    "get_chat",
    "close_chat",
    "start_generation",
    "get_generation",
    "read_candidate",
    "discard_generation",
    "start_review",
    "decide_review",
    "end_review",
    "read_source",
    "save_source",
    "preview_source_delete",
    "confirm_source_delete",
    "list_source_deletions",
    "import_date",
    "list_sources",
    "resolve_source_time",
    "list_authentic",
    "load_authentic",
    "save_authentic",
    "create_empty_authentic",
    "refresh_authentic",
    "get_operation",
    "watch_close",
    "finish_close",
  ].sort();
  const main = readFileSync("src-tauri/src/main.rs", "utf8");
  const build = readFileSync("src-tauri/build.rs", "utf8");
  const config = JSON.parse(readFileSync("src-tauri/tauri.conf.json", "utf8"));
  const capability = JSON.parse(
    readFileSync("src-tauri/capabilities/main.json", "utf8"),
  );
  it("matches manifest, invoke handler and individual permissions exactly", () => {
    const manifest = [...build.matchAll(/"([a-z_]+)"/gu)]
      .map((m) => m[1])
      .sort();
    const handler = main
      .match(/generate_handler!\[([^\]]+)\]/u)[1]
      .split(",")
      .map((v) => v.trim())
      .filter(Boolean)
      .sort();
    expect(manifest).toEqual(expected);
    expect(handler).toEqual(expected);
    expect(
      capability.permissions
        .map((p) => p.replace(/^allow-/u, "").replaceAll("-", "_"))
        .sort(),
    ).toEqual(expected);
    expect(
      [...main.matchAll(/#\[tauri::command\]\s*(?:async )?fn ([a-z_]+)/gu)]
        .map((m) => m[1])
        .sort(),
    ).toEqual(expected);
  });
  it("selects only the main local capability and grants no generic access", () => {
    expect(config.app.security.capabilities).toEqual(["main"]);
    expect(capability.local).toBe(true);
    expect(capability.windows).toEqual(["main"]);
    expect(capability.remote).toBeUndefined();
    expect(config.app.security.csp).toContain(
      "connect-src ipc: http://ipc.localhost",
    );
    expect(config.app.security.csp).not.toContain("127.0.0.1");
    expect(
      expected.some((name) => /shell|file|http|execute|proxy/u.test(name)),
    ).toBe(false);
  });
  it("checks formatting across all workspace members as part of the full gate", () => {
    expect(() =>
      execFileSync(
        "cargo",
        ["fmt", "--manifest-path", "src-tauri/Cargo.toml", "--all", "--check"],
        { stdio: "pipe" },
      ),
    ).not.toThrow();
  });
});

it("keeps fixture composition opt-in with no alternate public admission command", () => {
  for (const path of [
    "src-tauri/Cargo.toml",
    "src-tauri/crates/diary-api/Cargo.toml",
    "src-tauri/crates/diary-server/Cargo.toml",
  ]) {
    const manifest = readFileSync(path, "utf8");
    expect(manifest).toContain("scaffold-smoke =");
    expect(manifest).not.toMatch(/default\s*=\s*\[[^\]]*scaffold-smoke/u);
  }
  for (const path of [
    "src-tauri/src/main.rs",
    "src-tauri/crates/diary-api/src/lib.rs",
  ]) {
    expect(readFileSync(path, "utf8")).toMatch(
      /#\[cfg\(feature = "scaffold-smoke"\)\]\s*mod (native_smoke|smoke);/u,
    );
  }
});

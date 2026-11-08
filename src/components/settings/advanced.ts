import type {
  ProfileInfo,
  ServerSettings,
  UserSettings,
} from "../../application/bridge";
import { PROFILE_REASON } from "../../application/ai-agents/presentation";

export interface Blocker {
  id: string;
  affects: string;
  text: string;
  /** Settings pane that fixes it, when there is one. */
  fix?: "Writing Language";
}

/** Everything in the current configuration that stops Chat, Generate or saving. Pure, so it is testable. */
export function blockers(
  connected: boolean,
  server: ServerSettings | undefined,
  user: UserSettings | undefined,
  profiles: ProfileInfo[] | undefined,
): Blocker[] {
  if (!connected || !server)
    return [
      {
        id: "disconnected",
        affects: "Everything",
        text: "The server is not connected.",
      },
    ];
  const out: Blocker[] = [];
  const add = (
    id: string,
    affects: string,
    text: string,
    fix?: Blocker["fix"],
  ) => out.push({ id, affects, text, fix });
  if (!server.storage_ready)
    add("storage", "Everything", "Storage is unavailable.");
  if (!user) return out;
  if (user.read_only)
    add("read-only", "Settings", "Settings are read-only in this session.");
  if (!user.writing_locale)
    add(
      "locale",
      "Chat, Generate",
      "No Writing Language is chosen.",
      "Writing Language",
    );
  if (!server.sources.some((s) => s.available))
    add("sources", "Generate", "No source is available to draw snippets from.");
  if ((profiles ?? server.profiles).length === 0)
    add("no-profiles", "Chat, Generate", "No agent profile is configured.");
  else if (profiles) {
    for (const [what, pick] of [
      ["Chat", (p: ProfileInfo) => p.capabilities.chat],
      ["Generate", (p: ProfileInfo) => p.capabilities.generate],
    ] as const) {
      if (profiles.some((p) => pick(p).available)) continue;
      const reason = profiles.map((p) => pick(p).reason).find((r) => r);
      add(
        `no-${what}`,
        what,
        `No profile can ${what === "Chat" ? "chat" : "generate"}${reason ? `: ${PROFILE_REASON[reason] ?? reason}` : ""}.`,
      );
    }
    // Only definite failures count: unchecked, busy and sign-in-unknown profiles may still work.
    const dead = ["missing", "unsupported_version", "unavailable", "failed"];
    if (profiles.every((p) => dead.includes(p.status)))
      add(
        "none-ready",
        "Chat, Generate",
        "Every agent profile failed its check.",
      );
  }
  return out;
}

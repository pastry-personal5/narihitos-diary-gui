import {
  SettingsError,
  DEFAULT_STATUS_LINE,
  type Bridge,
  type ReloadResult,
  type ServerSettings,
  type UserSettings,
  type UserSettingsUpdate,
} from "../src/application/bridge";

type SettingsPart = Pick<
  Bridge,
  "serverSettings" | "reloadSettings" | "getUserSettings" | "updateUserSettings"
>;
export interface SettingsFake extends SettingsPart {
  user: UserSettings;
  server: ServerSettings;
  updates: UserSettingsUpdate[];
  reloadAnswer: ReloadResult;
}
import { DEFAULT_BINDINGS } from "../src/application/actions/actions";
const tag = /^[a-z]{2,3}(-[A-Za-z0-9]{2,8})*$/;
/** An in-memory stand-in for the settings commands; share one between Apps to model restart. */
export function settingsFake(
  user: Partial<UserSettings> = {},
  server: Partial<ServerSettings> = {},
): SettingsFake {
  let n = 0;
  const fake: SettingsFake = {
    user: {
      revision: "missing",
      read_only: false,
      recoverable: false,
      warnings: [],
      writing_locale: null,
      generation_count: 3,
      status_line: { ...DEFAULT_STATUS_LINE },
      agent_progress: {},
      keybindings: {},
      effective_keybindings: { ...DEFAULT_BINDINGS },
      keybinding_issues: [],
      ...user,
    },
    server: {
      config_generation: 1,
      last_reload: null,
      storage_ready: true,
      sources: [],
      profiles: [],
      ...server,
    },
    updates: [],
    reloadAnswer: { outcome: "applied", config_generation: 2, issues: [] },
    serverSettings: async () => structuredClone(fake.server),
    reloadSettings: async () => {
      fake.server = {
        ...fake.server,
        last_reload: fake.reloadAnswer,
        config_generation:
          fake.reloadAnswer.config_generation ?? fake.server.config_generation,
      };
      return fake.reloadAnswer;
    },
    getUserSettings: async () => structuredClone(fake.user),
    updateUserSettings: async (u) => {
      fake.updates.push(u);
      if (u.expectedRevision !== fake.user.revision)
        throw new SettingsError("conflict");
      if (u.recover) {
        fake.user = {
          ...fake.user,
          read_only: false,
          recoverable: false,
          warnings: [],
        };
      } else if (fake.user.read_only) {
        throw new SettingsError("read_only");
      } else if (u.section === "writing_init") {
        const found = (u.value as string[]).find((t) => tag.test(t));
        if (!found) throw new SettingsError("language_unresolved");
        if (fake.user.writing_locale === null) fake.user.writing_locale = found;
      } else if (u.section === "keybindings") {
        const v = u.value as {
          set?: Record<string, string[] | null>;
          reset_all?: boolean;
        };
        const stored = (v.reset_all ? {} : fake.user.keybindings) as Record<
          string,
          string[]
        >;
        for (const [id, keys] of Object.entries(v.set ?? {})) {
          if (keys === null) delete stored[id];
          else stored[id] = keys;
        }
        fake.user.keybindings = stored;
        fake.user.effective_keybindings = { ...DEFAULT_BINDINGS, ...stored };
      } else if (u.section === "agent_progress") {
        fake.user.agent_progress = structuredClone(u.value) as Record<
          string,
          boolean
        >;
      } else if (u.section === "status_line") {
        fake.user.status_line = structuredClone(
          u.value,
        ) as UserSettings["status_line"];
      } else if (u.section === "generation") {
        const count = (u.value as { count?: unknown }).count;
        if (
          typeof count !== "number" ||
          !Number.isInteger(count) ||
          count < 1 ||
          count > 5
        )
          throw new SettingsError("invalid_input");
        fake.user.generation_count = count;
      } else if (u.section === "writing") {
        if (typeof u.value !== "string" || !tag.test(u.value))
          throw new SettingsError("invalid_input");
        fake.user.writing_locale = u.value;
      }
      fake.user = { ...fake.user, revision: `r${++n}` };
      return structuredClone(fake.user);
    },
  };
  return fake;
}

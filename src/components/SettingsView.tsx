import { useState } from "react";
import {
  RELOAD_MESSAGE,
  SETTINGS_MESSAGE,
  type useSettings,
} from "./useSettings";

const SHORTCUTS = ["en-US", "ko-KR"] as const;
const WARNING: Record<string, string> = {
  file_invalid: "The settings file could not be read; defaults are in use.",
  newer_schema: "The settings file is from a newer version; it is read-only.",
  location_unsafe:
    "The settings folder has unsafe permissions; settings are read-only.",
  section_invalid: "A saved section is invalid and was ignored.",
};
type Model = ReturnType<typeof useSettings>;

export function SettingsView({
  model,
  connected,
  onClose,
}: {
  model: Model;
  connected: boolean;
  onClose: () => void;
}) {
  const { user, server, error } = model;
  const [section, setSection] = useState<"Status" | "Writing Language">(
    "Status",
  );
  const [draft, setDraft] = useState("");
  const [confirmRecover, setConfirmRecover] = useState(false);
  const locale = user?.writing_locale ?? null;
  const last = server?.last_reload;
  return (
    <section aria-label="Settings" className="settings">
      <nav aria-label="Settings sections">
        {(["Status", "Writing Language"] as const).map((name) => (
          <button
            key={name}
            aria-pressed={section === name}
            onClick={() => setSection(name)}
          >
            {name}
          </button>
        ))}
        <button onClick={onClose}>Close Settings</button>
      </nav>
      <div className="settings-body">
        {user?.read_only && (
          <p role="alert">Settings are read-only in this session.</p>
        )}
        {user?.warnings.map((w) => (
          <p role="alert" key={`${w.section}.${w.code}`}>
            {WARNING[w.code] ?? "A settings warning occurred."}
          </p>
        ))}
        {user?.recoverable && (
          <p>
            {confirmRecover ? (
              <>
                The original file is kept as a private backup first.{" "}
                <button
                  onClick={() => {
                    setConfirmRecover(false);
                    void model.recover();
                  }}
                >
                  Confirm recovery
                </button>{" "}
                <button onClick={() => setConfirmRecover(false)}>Cancel</button>
              </>
            ) : (
              <button onClick={() => setConfirmRecover(true)}>
                Recover settings…
              </button>
            )}
          </p>
        )}
        {error && <p role="alert">{SETTINGS_MESSAGE[error] ?? error}</p>}
        {section === "Status" ? (
          <div aria-label="Status">
            <h2>Status</h2>
            {!connected || !server ? (
              <p>
                {model.serverError
                  ? (SETTINGS_MESSAGE[model.serverError] ?? model.serverError)
                  : "Connect to the server to see its configuration status."}
              </p>
            ) : (
              <>
                <p>
                  Configuration generation {server.config_generation} · Storage{" "}
                  {server.storage_ready ? "ready" : "unavailable"}
                </p>
                <h3>Sources</h3>
                <ul aria-label="Sources">
                  {server.sources.length === 0 && <li>None configured</li>}
                  {server.sources.map((s) => (
                    <li key={s.alias}>
                      {s.alias}: {s.available ? "available" : s.reason}
                    </li>
                  ))}
                </ul>
                <h3>Agent profiles</h3>
                <ul aria-label="Agent profiles">
                  {server.profiles.length === 0 && <li>None configured</li>}
                  {server.profiles.map((p) => (
                    <li key={p.alias}>
                      {p.alias}: {p.available ? "available" : p.reason}
                    </li>
                  ))}
                </ul>
                {last && (
                  <div role="status" aria-label="Last reload">
                    <p>{RELOAD_MESSAGE[last.outcome]}</p>
                    <ul>
                      {last.issues.map((i) => (
                        <li key={`${i.path}.${i.code}`}>
                          {i.path}: {i.code}
                        </li>
                      ))}
                    </ul>
                  </div>
                )}
                <button disabled={model.reloading} onClick={model.reload}>
                  Reload configuration
                </button>
              </>
            )}
          </div>
        ) : (
          <div aria-label="Writing Language">
            <h2>Writing Language</h2>
            <p>
              {locale
                ? `Current: ${locale}. New AI runs use it.`
                : "No Writing Language is saved. Generation stays unavailable until you choose one."}
            </p>
            <label>
              Language tag
              <input
                value={draft}
                placeholder={locale ?? "ja-JP"}
                onChange={(e) => setDraft(e.target.value)}
              />
            </label>
            <button
              disabled={user?.read_only || draft.trim() === ""}
              onClick={() => {
                void model.setWriting(draft.trim());
                setDraft("");
              }}
            >
              Save language
            </button>
            {locale === null && !user?.read_only && (
              <p>
                {SHORTCUTS.map((tag) => (
                  <button key={tag} onClick={() => void model.setWriting(tag)}>
                    {tag}
                  </button>
                ))}
              </p>
            )}
          </div>
        )}
      </div>
    </section>
  );
}

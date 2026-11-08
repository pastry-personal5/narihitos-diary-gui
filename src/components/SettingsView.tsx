import type { Connection } from "../application/bridge";
import { KeyboardShortcuts } from "./KeyboardShortcuts";
import { useState } from "react";
import { SymbolLabel } from "./Symbol";
import { blockers } from "./advanced";
import { ProfileTable } from "./ProfileTable";
import type { useProfiles } from "./useProfiles";
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
  connection,
  onConnect,
  profiles,
  onClose,
}: {
  model: Model;
  connected: boolean;
  connection: Connection;
  onConnect: () => void;
  profiles: ReturnType<typeof useProfiles>;
  onClose: () => void;
}) {
  const { user, server, error } = model;
  const issues = blockers(connected, server, user, profiles.profiles);
  const [section, setSection] = useState<
    | "Status"
    | "Server"
    | "Writing Language"
    | "Keyboard Shortcuts"
    | "Generation"
  >("Status");
  const [draft, setDraft] = useState("");
  const [confirmRecover, setConfirmRecover] = useState(false);
  const locale = user?.writing_locale ?? null;
  const last = server?.last_reload;
  return (
    <section aria-label="Settings" className="settings">
      <nav aria-label="Settings sections">
        <button
          className="settings-back"
          title="Close Settings"
          aria-label="Go back"
          onClick={onClose}
        >
          <SymbolLabel
            name="chevron.backward"
            label="Go back"
            fallback="←"
            size={20}
          />
        </button>
        {(
          [
            "Status",
            "Server",
            "Writing Language",
            "Keyboard Shortcuts",
            "Generation",
          ] as const
        ).map((name) => (
          <button
            key={name}
            aria-pressed={section === name}
            onClick={() => setSection(name)}
          >
            {name}
          </button>
        ))}
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
          <p
            role={confirmRecover ? "dialog" : undefined}
            aria-label={confirmRecover ? "Recover settings" : undefined}
          >
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
              <div className="card">
                <p>
                  {model.serverError
                    ? (SETTINGS_MESSAGE[model.serverError] ?? model.serverError)
                    : "Connect to the server to see its configuration status."}
                </p>
              </div>
            ) : (
              <>
                <div className="card">
                  <h3>Server</h3>
                  <p>
                    Configuration generation {server.config_generation} ·
                    Storage {server.storage_ready ? "ready" : "unavailable"}
                  </p>
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
                  {model.serverError && (
                    <p role="alert">
                      {SETTINGS_MESSAGE[model.serverError] ?? model.serverError}
                    </p>
                  )}
                  <button disabled={model.reloading} onClick={model.reload}>
                    Reload configuration
                  </button>
                </div>
                <div className="card">
                  <h3>Sources</h3>
                  <ul aria-label="Sources">
                    {server.sources.length === 0 && <li>None configured</li>}
                    {server.sources.map((s) => (
                      <li key={s.alias}>
                        {s.alias}: {s.available ? "available" : s.reason}
                      </li>
                    ))}
                  </ul>
                </div>
                <div className="card">
                  <h3>Agent profiles</h3>
                  {profiles.supported ? (
                    <ProfileTable profiles={profiles} />
                  ) : (
                    <ul aria-label="Agent profiles">
                      {server.profiles.length === 0 && <li>None configured</li>}
                      {server.profiles.map((p) => (
                        <li key={p.alias}>
                          {p.alias}: {p.available ? "available" : p.reason}
                        </li>
                      ))}
                    </ul>
                  )}
                </div>
              </>
            )}
            <div className="card" aria-label="Advanced">
              <h3>Advanced</h3>
              {issues.length === 0 ? (
                <p>Nothing is blocking Chat or Generate.</p>
              ) : (
                <ul>
                  {issues.map((i) => (
                    <li key={i.id}>
                      <strong>{i.affects}:</strong> {i.text}{" "}
                      {i.fix && (
                        <button onClick={() => setSection(i.fix!)}>
                          Open {i.fix}
                        </button>
                      )}
                    </li>
                  ))}
                </ul>
              )}
            </div>
          </div>
        ) : section === "Server" ? (
          <div className="card" aria-label="Server connection">
            <h2>Server</h2>
            <p role="status" aria-label="Server connection status">
              {connection.kind === "connected"
                ? connection.status.storage_ready
                  ? "Connected"
                  : "Connected · Storage unavailable"
                : connection.kind === "connecting"
                  ? "Connecting…"
                  : connection.message}
            </p>
            <button
              onClick={onConnect}
              disabled={connection.kind === "connecting"}
            >
              Connect
            </button>
          </div>
        ) : section === "Generation" ? (
          <div className="card" aria-label="Generation settings">
            <h2>Generation</h2>
            <label htmlFor="generation-count">Candidates per generation</label>
            <select
              id="generation-count"
              value={user?.generation_count ?? 3}
              disabled={!user || user.read_only}
              onChange={(e) =>
                void model.setGenerationCount(Number(e.target.value))
              }
            >
              {[1, 2, 3, 4, 5].map((n) => (
                <option key={n} value={n}>
                  {n}
                </option>
              ))}
            </select>
            <p>
              Saved for new requests. Candidates run sequentially with the
              selected profile.
            </p>
          </div>
        ) : section === "Keyboard Shortcuts" ? (
          <KeyboardShortcuts user={user} save={model.setKeybindings} />
        ) : (
          <div className="card" aria-label="Writing Language">
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
              onClick={() =>
                void model
                  .setWriting(draft.trim())
                  .then((saved) => saved && setDraft(""))
              }
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

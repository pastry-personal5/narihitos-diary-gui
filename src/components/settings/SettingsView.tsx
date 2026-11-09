import type { AIChatController } from "../../application/ai-chat/controller";
import type { GenerationController } from "../../application/generation/controller";
import { ProfilePicker } from "../ai-agents/ProfilePicker";
import { DEFAULT_STATUS_LINE } from "../../application/bridge";
import type { Connection } from "../../application/bridge";
import { KeyboardShortcuts } from "./KeyboardShortcuts";
import { useState } from "react";
import { SymbolLabel } from "../Symbol";
import { blockers } from "./advanced";
import { ProfileTable } from "../ai-agents/ProfileTable";
import type { ProfileStatusModel } from "../../application/ai-agents/contracts";
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
  chat,
  generator,
  connected,
  connection,
  onConnect,
  profiles,
  paragraphProfiles = [],
  paragraphProfile = null,
  onParagraphProfile,
  onClose,
}: {
  model: Model;
  chat: AIChatController;
  generator: GenerationController;
  connected: boolean;
  connection: Connection;
  onConnect: () => void;
  profiles: ProfileStatusModel;
  paragraphProfiles?: import("../../application/bridge").ProfileInfo[];
  paragraphProfile?: string | null;
  onParagraphProfile?: (alias: string) => void;
  onClose: () => void;
}) {
  const { user, server, error } = model;
  const issues = blockers(connected, server, user, profiles.profiles);
  const [section, setSection] = useState<
    | "Status"
    | "Server"
    | "Sources"
    | "AI Agents"
    | "Writing Language"
    | "Status Line"
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
          autoFocus
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
            "Server",
            "Status",
            "Sources",
            "AI Agents",
            "Writing Language",
            "Generation",
            "Status Line",
            "Keyboard Shortcuts",
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
          <div>
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

            {server ? (
              <div className="card">
                <h3>Server</h3>
                <p>
                  Configuration generation {server.config_generation} · Storage{" "}
                  {server.storage_ready ? "ready" : "unavailable"}
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
            ) : (
              <div className="card">
                <p>
                  {model.serverError
                    ? (SETTINGS_MESSAGE[model.serverError] ?? model.serverError)
                    : "Connect to the server to see its configuration status."}
                </p>
              </div>
            )}
          </div>
        ) : section === "Sources" ? (
          <div>
            <h2>Sources</h2>
            {server ? (
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
            ) : (
              <div className="card">
                <p>Connect to the server to see its sources.</p>
              </div>
            )}
            <div className="card" aria-label="Server configuration folder">
              <h3>Server configuration</h3>
              <p>
                Edit sources in the server configuration file, then reload
                configuration in Server.
              </p>
              <button
                disabled={!connected || model.revealing}
                onClick={() => void model.revealConfigurationFolder()}
              >
                {model.revealing
                  ? "Opening Finder…"
                  : "Reveal configuration folder in Finder"}
              </button>
              {model.revealError && (
                <p role="alert">
                  Could not open the server configuration folder in Finder.
                  Check the server connection and try again.
                </p>
              )}
            </div>
          </div>
        ) : section === "AI Agents" ? (
          <div>
            <h2>AI Agents</h2>
            <ProfilePicker chat={chat} generator={generator} />
            <div className="card" aria-label="Paragraph Review profile">
              <h3>Paragraph Review profile</h3>
              <label htmlFor="paragraph-review-profile">
                Paragraph Review profile
              </label>
              <select
                id="paragraph-review-profile"
                value={paragraphProfile ?? ""}
                disabled={paragraphProfiles.length === 0}
                onChange={(event) => onParagraphProfile?.(event.target.value)}
              >
                {paragraphProfiles.map((p) => (
                  <option key={p.alias} value={p.alias}>
                    {p.alias} · {p.status.replaceAll("_", " ")}
                  </option>
                ))}
              </select>
            </div>
            {profiles.supported ? (
              <div className="card">
                <h3>Agent profiles</h3>
                <ProfileTable
                  profiles={profiles}
                  progress={user?.agent_progress}
                  progressDisabled={
                    !user ||
                    user.read_only ||
                    user.recoverable ||
                    model.savingAgentProgress
                  }
                  onProgress={(alias, enabled) =>
                    void model.setAgentProgress({
                      ...user?.agent_progress,
                      [alias]: enabled,
                    })
                  }
                />
              </div>
            ) : server ? (
              <div className="card">
                <h3>Agent profiles</h3>
                <ul aria-label="Agent profiles">
                  {server.profiles.length === 0 && <li>None configured</li>}
                  {server.profiles.map((p) => (
                    <li key={p.alias}>
                      {p.alias}: {p.available ? "available" : p.reason}
                    </li>
                  ))}
                </ul>
              </div>
            ) : (
              <div className="card">
                <h3>Agent profiles</h3>
                <p>Connect to the server to see its agent profiles.</p>
              </div>
            )}
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
        ) : section === "Status Line" ? (
          <div className="card" aria-label="Status Line settings">
            <h2>Status Line</h2>
            <p>Choose which ready indicators appear in the status line.</p>
            {(
              [
                ["generate_ready", "Generate ready"],
                ["review_ready", "Review ready"],
                ["chat_ready", "Chat ready"],
              ] as const
            ).map(([key, label]) => (
              <label key={key}>
                <input
                  type="checkbox"
                  checked={(user?.status_line ?? DEFAULT_STATUS_LINE)[key]}
                  disabled={!user || user.read_only}
                  onChange={(event) =>
                    void model.setStatusLine({
                      ...(user?.status_line ?? DEFAULT_STATUS_LINE),
                      [key]: event.target.checked,
                    })
                  }
                />
                {label}
              </label>
            ))}
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

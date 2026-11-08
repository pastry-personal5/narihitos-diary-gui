import type { AIChatController } from "../../application/ai-chat/controller";
import type { GenerationController } from "../../application/generation/controller";
import { useSyncExternalStore } from "react";

/** Chat and generation profile cards in Settings > AI Agents. Status comes from background checks. */
export function ProfilePicker({
  chat,
  generator,
}: {
  chat: AIChatController;
  generator: GenerationController;
}) {
  const c = useSyncExternalStore(chat.subscribe, () => chat.state);
  const g = useSyncExternalStore(generator.subscribe, () => generator.state);
  return (
    <>
      <div className="card" aria-label="Chat profile">
        <h3>Chat profile</h3>
        <label htmlFor="chat-profile">Chat profile</label>
        <select
          id="chat-profile"
          value={c.profile ?? ""}
          disabled={chat.running || c.profiles.length === 0}
          onChange={(e) => void chat.selectProfile(e.target.value)}
        >
          {c.profiles.map((p) => (
            <option key={p.alias} value={p.alias}>
              {p.alias} · {p.status.replaceAll("_", " ")}
            </option>
          ))}
        </select>
      </div>
      <div className="card" aria-label="Generation profile">
        <h3>Generation profile</h3>
        <label htmlFor="generation-profile">Generation profile</label>
        <select
          id="generation-profile"
          value={g.profile ?? ""}
          disabled={generator.running || g.profiles.length === 0}
          onChange={(e) => generator.selectProfile(e.target.value)}
        >
          {g.profiles.map((p) => (
            <option key={p.alias} value={p.alias}>
              {p.alias} · {p.status.replaceAll("_", " ")}
            </option>
          ))}
        </select>
      </div>
    </>
  );
}

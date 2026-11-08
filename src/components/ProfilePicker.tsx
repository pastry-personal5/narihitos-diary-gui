import type { ChatController } from "../application/chat";
import type { GenerationController } from "../application/generation/controller";
import { useChat } from "./useChat";
import { useGeneration } from "./useGeneration";

/** Chat and generation profile cards in Settings > AI Agents. Status comes from background checks. */
export function ProfilePicker({
  chat,
  generator,
}: {
  chat: ChatController;
  generator: GenerationController;
}) {
  const c = useChat(chat);
  const g = useGeneration(generator);
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

import type { Bridge } from "../../application/bridge";
import type {
  ParagraphReviewer,
  ParagraphReviewRequest,
} from "../../application/paragraph-review/contracts";
import { paragraphReviewPrompt } from "./prompt";

type Transport = Pick<
  Bridge,
  "openChat" | "sendChat" | "getChat" | "closeChat"
>;
/** Current AI implementation. Conversation IDs, chat DTOs and polling never cross this adapter. */
export class ChatParagraphReviewer implements ParagraphReviewer {
  constructor(
    private transport: Transport,
    private timeoutMs = 600_000,
    private pollMs = 750,
  ) {}
  get available() {
    return !!(
      this.transport.openChat &&
      this.transport.sendChat &&
      this.transport.getChat &&
      this.transport.closeChat
    );
  }
  async review(
    request: ParagraphReviewRequest,
    signal: AbortSignal,
  ): Promise<string> {
    if (!this.available) throw new Error("unavailable");
    const contextPromise = Promise.resolve(request.context);
    let id: string | null = null;
    let closed = false;
    const local = new AbortController();
    const abort = () => local.abort();
    signal.addEventListener("abort", abort, { once: true });
    if (signal.aborted) local.abort();
    const close = async () => {
      if (!id || closed) return;
      closed = true;
      await this.transport.closeChat!(id).catch(() => {});
    };
    const cancelled = new Promise<never>((_resolve, reject) => {
      const stop = () => {
        void close();
        reject(new Error("cancelled"));
      };
      local.signal.addEventListener("abort", stop, { once: true });
      if (local.signal.aborted) stop();
    });
    const run = async () => {
      if (local.signal.aborted) throw new Error("cancelled");
      id = (
        await this.transport.openChat!(
          request.profile.alias,
          request.locale,
          "paragraph_review",
        )
      ).chat_id;
      if (local.signal.aborted) {
        await close();
        throw new Error("cancelled");
      }
      const message = paragraphReviewPrompt(
        request.profile.type,
        request.text,
        await contextPromise,
        request.locale,
      );
      await this.transport.sendChat!(id, crypto.randomUUID(), message, null);
      while (!local.signal.aborted) {
        const result = await this.transport.getChat!(id);
        if (local.signal.aborted) throw new Error("cancelled");
        if (result.turn && result.turn.state !== "running") {
          if (result.turn.state !== "done") throw new Error("review_failed");
          return (
            result.transcript.filter((item) => item.role === "assistant").at(-1)
              ?.text ?? ""
          );
        }
        await new Promise<void>((resolve) => {
          const done = () => {
            clearTimeout(timer);
            local.signal.removeEventListener("abort", done);
            resolve();
          };
          const timer = setTimeout(done, this.pollMs);
          local.signal.addEventListener("abort", done, { once: true });
        });
      }
      throw new Error("cancelled");
    };
    const deadline = setTimeout(abort, this.timeoutMs);
    try {
      return await Promise.race([run(), cancelled]);
    } finally {
      clearTimeout(deadline);
      signal.removeEventListener("abort", abort);
      local.abort();
      await close();
    }
  }
}

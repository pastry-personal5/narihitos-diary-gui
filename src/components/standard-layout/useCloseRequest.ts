import type { GlobalMessagesStore } from "../../application/global-messages/store";
import { useEffect, useState } from "react";
import type { Bridge } from "../../application/bridge";
import type { GenerationController } from "../../application/generation/controller";
import type { NavigationModel } from "../../application/navigation/model";

/** Window-close handshake: flush work, ask the server, and report whether closing is pending. */
export function useCloseRequest(
  bridge: Bridge,
  model: NavigationModel,
  generator: GenerationController,
  messages: GlobalMessagesStore,
) {
  const [closePending, setClosePending] = useState(false);
  useEffect(() => {
    let off: (() => void) | undefined;
    let disposed = false;
    void bridge
      .onCloseRequested?.(
        async () => {
          if (
            generator.state.dirty ||
            (generator.state.submitting &&
              !generator.state.activeJob &&
              !generator.state.uncertain)
          )
            await generator.cancel();
          const proceed = await model.resources.closeApp();
          if (proceed) setClosePending(true);
          return proceed;
        },
        () => {
          setClosePending(false);
          messages.add(
            "The server could not confirm closing. The window stayed open; retry closing or reconnect.",
            "error",
            true,
          );
        },
      )
      .then((stop) => {
        if (disposed) stop();
        else off = stop;
      });
    return () => {
      disposed = true;
      off?.();
    };
  }, [bridge, model, generator, messages]);
  return closePending;
}

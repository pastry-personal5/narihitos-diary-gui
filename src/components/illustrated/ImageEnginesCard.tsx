import { useEffect, useSyncExternalStore } from "react";
import type { IllustratedController } from "../../application/illustrated/controller";

export function ImageEnginesCard({
  controller,
  connected,
}: {
  controller: IllustratedController;
  connected: boolean;
}) {
  const state = useSyncExternalStore(
    controller.subscribe,
    () => controller.state,
  );
  useEffect(() => {
    if (connected) void controller.checkEngines();
  }, [controller, connected]);
  return (
    <div className="card" aria-label="Image engines">
      <h3>Image engines</h3>
      <p>
        Illustrated diaries use the Generation profile and Writing Language.
        Engine selection and photo settings come from server configuration;
        restart the server after changing them. Readiness is checked
        automatically in the background; problems appear in Global Messages.
      </p>
      {!connected ? (
        <p>Connect to see image engines.</p>
      ) : (
        <>
          {state.engineError && (
            <p role="alert">
              Image engine status could not be checked.{" "}
              <button onClick={() => void controller.checkEngines()}>
                Retry engine status
              </button>
            </p>
          )}
          {!state.engines ? (
            <p>Loading image engine status…</p>
          ) : !state.engines.image_engines.length ? (
            <p>No image engines configured.</p>
          ) : (
            <table aria-label="Image engines" aria-busy={!!state.checking}>
              <thead>
                <tr>
                  <th>Engine</th>
                  <th>Status</th>
                  <th>Check</th>
                </tr>
              </thead>
              <tbody>
                {state.engines.image_engines.map((engine) => (
                  <tr key={engine.id}>
                    <th scope="row">
                      {engine.label} <small>({engine.id})</small>
                      {state.engines?.illustrated_engine === engine.id
                        ? " · Selected for Illustrated"
                        : ""}
                    </th>
                    <td>
                      {engine.status}
                      {engine.reason ? " · Prerequisites unavailable" : ""}
                    </td>
                    <td>
                      <button
                        disabled={!!state.checking || engine.status === "busy"}
                        aria-label={`Check ${engine.label}`}
                        onClick={() => void controller.checkEngine(engine.id)}
                      >
                        {state.checking === engine.id ? "Checking…" : "Check"}
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </>
      )}
    </div>
  );
}

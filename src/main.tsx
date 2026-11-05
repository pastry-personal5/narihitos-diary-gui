import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./components/App";
import { logClientEvent, tauriBridge } from "./adapters/tauri-bridge";
import { installErrorReporting } from "./application/error-reporting";
import "./styles.css";
installErrorReporting(window, logClientEvent);
const root = document.getElementById("root");
if (!root) throw new Error("Missing application root");
createRoot(root).render(
  <StrictMode>
    <App bridge={tauriBridge} />
  </StrictMode>,
);

import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./app/App.js";
import { ExportWorkerApp } from "./app/ExportWorkerApp.js";
import "./i18n/i18n.js";
import "./app/global.css";

const container = document.getElementById("root");
if (!container) throw new Error("Missing #root element");

// Desktop-only hidden window (see useAreaExport.ts): rendering it via a query param on the same
// entry point avoids a second Vite/Tauri build target for one small headless view.
const isExportWorker = new URLSearchParams(location.search).has("exportWorker");

createRoot(container).render(
  <StrictMode>{isExportWorker ? <ExportWorkerApp /> : <App />}</StrictMode>,
);

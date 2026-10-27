import React from "react";
import ReactDOM from "react-dom/client";
import { HashRouter } from "react-router-dom";
import App from "./App";
import { useI18n } from "./i18n";
import "./index.css";
import { api } from "./lib/api";

/** Renders once the language is known, so the app doesn't open in English first. */
function render() {
  api
    .getSettings()
    .then((s) => useI18n.getState().setLanguage(s.language))
    .catch(() => {})
    .finally(mount);
}

function mount() {
  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <HashRouter>
        <App />
      </HashRouter>
    </React.StrictMode>,
  );
}

// In a plain browser during development, answer the Rust commands with demo data (see lib/demo.ts).
if (import.meta.env.DEV && !("__TAURI_INTERNALS__" in window)) {
  import("./lib/demo").then((demo) => {
    demo.installDemo();
    render();
  });
} else {
  render();
}

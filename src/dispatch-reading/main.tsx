// Dispatch: the read-aloud pill's window (src-tauri/src/dispatch_reading_pill.rs).
import React from "react";
import ReactDOM from "react-dom/client";
import { listen } from "@tauri-apps/api/event";
import { ReadingPill } from "./ReadingPill";
import {
  applyTheme,
  getStoredTheme,
  syncThemeFromSettings,
} from "@/lib/utils/theme";
import type { Theme } from "@/bindings";
import "@/i18n";

// Its own webview, so it follows the theme on its own, like Handy's overlay.
applyTheme(getStoredTheme());
syncThemeFromSettings();
listen<Theme>("theme-changed", (event) => applyTheme(event.payload));

// It floats over other apps: no webview right-click menu in release builds.
if (import.meta.env.PROD) {
  window.addEventListener("contextmenu", (e) => e.preventDefault());
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ReadingPill />
  </React.StrictMode>,
);

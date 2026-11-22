// Dispatch: the splash screen's window (src-tauri/src/dispatch_splash.rs).
// Shown for about a second while Dispatch loads, when it's opened by hand.
import React from "react";
import ReactDOM from "react-dom/client";
import { Splash } from "./Splash";

if (import.meta.env.PROD) {
  window.addEventListener("contextmenu", (e) => e.preventDefault());
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <Splash />
  </React.StrictMode>,
);

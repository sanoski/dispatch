// Dispatch: tells Rust the main window has drawn its first frame, so the
// splash screen can make way for it (src-tauri/src/dispatch_splash.rs).
import { useEffect } from "react";
import { commands } from "@/bindings";

export const useMainWindowReady = () => {
  useEffect(() => {
    // Two frames: the first one is painted by the time the second starts.
    const frame = requestAnimationFrame(() =>
      requestAnimationFrame(() => {
        commands.mainWindowReady().catch(() => {});
      }),
    );
    return () => cancelAnimationFrame(frame);
  }, []);
};

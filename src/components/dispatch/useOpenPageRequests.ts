// Dispatch: lets Rust switch the main window to a sidebar page, for example
// the read-aloud pill's "Set up" button opening Read Aloud
// (dispatch_reading_pill.rs, OPEN_PAGE_EVENT).
import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { SECTIONS_CONFIG, type SidebarSection } from "../Sidebar";

const OPEN_PAGE_EVENT = "dispatch-open-page";

export const useOpenPageRequests = (
  open: (section: SidebarSection) => void,
) => {
  useEffect(() => {
    const unlisten = listen<string>(OPEN_PAGE_EVENT, (event) => {
      if (event.payload in SECTIONS_CONFIG) {
        open(event.payload as SidebarSection);
      }
    });
    return () => {
      unlisten.then((stop) => stop());
    };
  }, [open]);
};

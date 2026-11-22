// Dispatch achievements: the status from Rust, kept fresh as things unlock.
import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { commands, type AchievementsStatus } from "@/bindings";

/** Rust's event when something new unlocks (dispatch_achievements.rs). */
export const ACHIEVEMENTS_CHANGED = "dispatch-achievements-changed";

/** Voice data tiers, in minutes (matches TIER_MINUTES in Rust). */
export const TIERS = [
  { id: "tier_1", minutes: 30 },
  { id: "tier_2", minutes: 120 },
  { id: "tier_3", minutes: 300 },
  { id: "tier_4", minutes: 600 },
] as const;

export const useAchievements = () => {
  const [status, setStatus] = useState<AchievementsStatus | null>(null);
  const [failed, setFailed] = useState(false);

  const load = useCallback(async () => {
    const result = await commands.getAchievements();
    if (result.status === "ok") {
      setStatus(result.data);
      setFailed(false);
    } else {
      console.error("Couldn't load achievements:", result.error);
      setFailed(true);
    }
    return result.status === "ok" ? result.data : null;
  }, []);

  useEffect(() => {
    load();
    const unlisten = listen(ACHIEVEMENTS_CHANGED, () => load());
    return () => {
      unlisten.then((stop) => stop());
    };
  }, [load]);

  return { status, failed, reload: load };
};

/** "21.2 min", or "2 h 05 min" from an hour up. */
export const audioLength = (ms: number): { value: string; unit: string } => {
  const minutes = ms / 60_000;
  if (minutes < 60) return { value: minutes.toFixed(1), unit: "min" };
  const hours = Math.floor(minutes / 60);
  const rest = Math.floor(minutes - hours * 60);
  return { value: `${hours} h ${String(rest).padStart(2, "0")}`, unit: "min" };
};

/** "Nov 21" in the user's locale. */
export const shortDate = (unixSecs: number, locale: string): string =>
  new Date(unixSecs * 1000).toLocaleDateString(locale, {
    month: "short",
    day: "numeric",
  });

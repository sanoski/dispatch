// Dispatch achievements: the pop-up when something unlocks, with the signal
// lamps flashing like a real crossing and a soft chime. It only shows while
// the window is on screen; anything unlocked while it was hidden waits for
// the next time it's opened. Three or more at once share one pop-up.
import React, { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { X } from "lucide-react";
import { commands, type Achievement, type SoundTheme } from "@/bindings";
import { useSettings } from "@/hooks/useSettings";
import { SignalLamps } from "./SignalLamps";
import { ACHIEVEMENTS_CHANGED, TIERS } from "./useAchievements";
import { playAchievementSound } from "./chime";

const SHOW_FOR_MS = 9_000;

interface Popup {
  key: string;
  items: Achievement[];
  /** For a single hidden one: how many hidden are found, counting it. */
  hiddenFound: number;
  hiddenTotal: number;
}

export const UnlockPopup: React.FC<{ onView: () => void }> = ({ onView }) => {
  const { t } = useTranslation();
  const { getSetting } = useSettings();
  const [queue, setQueue] = useState<Popup[]>([]);
  const [paused, setPaused] = useState(false);
  const queued = useRef(new Set<string>());
  const soundRef = useRef<{
    on: boolean;
    volume: number;
    theme: SoundTheme | undefined;
  }>({ on: true, volume: 1, theme: undefined });
  soundRef.current = {
    on: getSetting("audio_feedback") ?? true,
    volume: getSetting("audio_feedback_volume") ?? 1,
    theme: getSetting("sound_theme"),
  };

  const check = useCallback(async () => {
    if (document.visibilityState !== "visible") return;
    const result = await commands.getAchievements();
    if (result.status !== "ok") return;
    const all = result.data.achievements;
    const fresh = all
      .filter(
        (a) => a.unlocked_at !== null && !a.seen && !queued.current.has(a.id),
      )
      .sort((a, b) => (a.unlocked_at ?? 0) - (b.unlocked_at ?? 0));
    if (fresh.length === 0) return;
    fresh.forEach((a) => queued.current.add(a.id));
    commands.markAchievementsSeen(fresh.map((a) => a.id));

    const hiddenTotal = all.filter((a) => a.section === "hidden").length;
    let hiddenFound =
      all.filter((a) => a.section === "hidden" && a.unlocked_at !== null)
        .length - fresh.filter((a) => a.section === "hidden").length;
    const popups: Popup[] =
      fresh.length > 2
        ? [
            {
              key: fresh.map((a) => a.id).join(),
              items: fresh,
              hiddenFound: 0,
              hiddenTotal,
            },
          ]
        : fresh.map((a) => {
            if (a.section === "hidden") hiddenFound += 1;
            return { key: a.id, items: [a], hiddenFound, hiddenTotal };
          });
    setQueue((q) => [...q, ...popups]);
  }, []);

  useEffect(() => {
    check();
    const unlisten = listen(ACHIEVEMENTS_CHANGED, () => check());
    const onVisible = () => check();
    document.addEventListener("visibilitychange", onVisible);
    window.addEventListener("focus", onVisible);
    return () => {
      unlisten.then((stop) => stop());
      document.removeEventListener("visibilitychange", onVisible);
      window.removeEventListener("focus", onVisible);
    };
  }, [check]);

  const current = queue[0];
  const dismiss = useCallback(() => {
    setPaused(false);
    setQueue((q) => q.slice(1));
  }, []);

  // A chime as each pop-up appears (once, even if it re-renders).
  const chimedFor = useRef<string | null>(null);
  useEffect(() => {
    if (!current || chimedFor.current === current.key) return;
    chimedFor.current = current.key;
    if (soundRef.current.on) {
      playAchievementSound(soundRef.current.theme, soundRef.current.volume);
    }
  }, [current]);

  // Times out unless the pointer or focus is on it.
  useEffect(() => {
    if (!current || paused) return;
    const timer = window.setTimeout(dismiss, SHOW_FOR_MS);
    return () => window.clearTimeout(timer);
  }, [current, paused, dismiss]);

  if (!current) return null;

  const single = current.items.length === 1 ? current.items[0] : null;
  let eyebrow: string;
  let title: string;
  let description: string;
  if (!single) {
    eyebrow = t("dispatch.achievements.popup.many", {
      count: current.items.length,
    });
    title = t("dispatch.achievements.popup.manyTitle");
    description = current.items.map((a) => nameOf(a, t)).join(", ");
  } else if (single.section === "voice_data") {
    eyebrow = t("dispatch.achievements.popup.tier");
    title = t("dispatch.achievements.popup.tierTitle", {
      tier: t(`dispatch.achievements.tiers.${single.id}.name`),
      amount: t(`dispatch.achievements.tiers.${single.id}.amount`),
    });
    description = t(`dispatch.achievements.tiers.${single.id}.meaning`);
  } else {
    eyebrow =
      single.section === "hidden"
        ? t("dispatch.achievements.popup.hidden", {
            found: current.hiddenFound,
            total: current.hiddenTotal,
          })
        : t("dispatch.achievements.popup.unlocked");
    title = t(`dispatch.achievements.items.${single.id}.title`);
    description = t(`dispatch.achievements.items.${single.id}.desc`);
  }

  return (
    <div
      key={current.key}
      role="status"
      aria-live="polite"
      onMouseEnter={() => setPaused(true)}
      onMouseLeave={() => setPaused(false)}
      onFocus={() => setPaused(true)}
      onBlur={() => setPaused(false)}
      className="dispatch-pop-in fixed bottom-16 right-5 z-50 flex w-[460px] max-w-[calc(100vw-2.5rem)] items-center gap-4 rounded-2xl border border-logo-primary/50 bg-[var(--color-card)] py-4 pe-4 ps-5 shadow-[0_24px_48px_-24px_rgba(0,0,0,0.8),0_0_0_4px_color-mix(in_srgb,var(--color-logo-primary),transparent_93%)]"
    >
      <SignalLamps state="lit" size="popup" flashing />
      <div className="flex min-w-0 flex-1 flex-col gap-1">
        <span className="text-xs font-semibold uppercase tracking-[0.8px] text-logo-primary">
          {eyebrow}
        </span>
        <span className="font-display text-[21px] font-bold leading-tight tracking-[-0.2px]">
          {title}
        </span>
        <span className="text-sm leading-[1.45] text-ink-2">{description}</span>
      </div>
      <div className="flex flex-col items-end justify-between gap-2 self-stretch">
        <button
          type="button"
          onClick={dismiss}
          aria-label={t("dispatch.achievements.popup.dismiss")}
          className="grid h-8 w-8 cursor-pointer place-items-center rounded-lg text-ink-3 hover:bg-line hover:text-text focus:outline-none focus-visible:ring-2 focus-visible:ring-logo-primary"
        >
          <X size={14} strokeWidth={1.8} />
        </button>
        <button
          type="button"
          onClick={() => {
            onView();
            dismiss();
          }}
          className="cursor-pointer whitespace-nowrap rounded-lg bg-logo-primary px-3 py-2 text-[13px] font-semibold text-[var(--color-on-accent)] hover:brightness-110 focus:outline-none focus-visible:ring-2 focus-visible:ring-text"
        >
          {t("dispatch.achievements.popup.view")}
        </button>
      </div>
    </div>
  );
};

const nameOf = (
  a: Achievement,
  t: ReturnType<typeof useTranslation>["t"],
): string =>
  TIERS.some((tier) => tier.id === a.id)
    ? t(`dispatch.achievements.tiers.${a.id}.name`)
    : t(`dispatch.achievements.items.${a.id}.title`);

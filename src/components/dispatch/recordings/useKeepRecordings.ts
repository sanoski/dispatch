// Dispatch: keep every recording, or only the newest few (dispatch_audio.rs).
// Switching to "newest" first asks, showing what would move to the Recycle
// Bin, so a library of recordings can't vanish with one click.
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { commands, type AudioCleanup } from "@/bindings";
import { notify } from "@/stores/noticeStore";
import { useSettingsStore } from "@/stores/settingsStore";

/** Matches NEWEST_KEPT in dispatch_audio.rs. */
export const NEWEST_KEPT = 10;

export const useKeepRecordings = () => {
  const { t } = useTranslation();
  const keepAll = useSettingsStore(
    (s) => s.settings?.dispatch_keep_all_audio ?? true,
  );
  const [pending, setPending] = useState<AudioCleanup | null>(null);
  const [busy, setBusy] = useState(false);

  const apply = async (value: boolean) => {
    setBusy(true);
    try {
      const result = await commands.changeKeepAllAudioSetting(value);
      if (result.status === "error") {
        notify({ title: t("dispatch.recordings.error") });
      }
      await useSettingsStore.getState().refreshSettings();
    } finally {
      setBusy(false);
      setPending(null);
    }
  };

  /**
   * Keep all at once; "newest" asks first if anything would be moved.
   * Resolves to "asking" while the dialog is open, else "done" or "failed".
   */
  const choose = async (
    value: boolean,
  ): Promise<"done" | "asking" | "failed"> => {
    if (value) {
      await apply(true);
      return "done";
    }
    const preview = await commands.getAudioCleanupPreview();
    if (preview.status === "error") {
      notify({ title: t("dispatch.recordings.error") });
      return "failed";
    }
    if (preview.data.recordings > 0) {
      setPending(preview.data);
      return "asking";
    }
    await apply(false);
    return "done";
  };

  return {
    keepAll,
    busy,
    choose,
    /** What "newest" would move, while the user is being asked. */
    pending,
    confirm: () => apply(false),
    cancel: () => setPending(null),
  };
};

/** "39 minutes", "under a minute", "2 hours 5 minutes". */
export const lengthLabel = (
  ms: number,
  t: (key: string, options?: Record<string, unknown>) => string,
) => {
  const minutes = Math.round(ms / 60_000);
  if (minutes < 1) return t("dispatch.recordings.underAMinute");
  if (minutes < 60) return t("dispatch.recordings.minutes", { count: minutes });
  const hours = Math.floor(minutes / 60);
  const rest = minutes % 60;
  return rest === 0
    ? t("dispatch.recordings.hours", { count: hours })
    : t("dispatch.recordings.hoursMinutes", { hours, count: rest });
};

/** "74 MB", "1.2 GB". */
export const sizeLabel = (bytes: number) =>
  bytes >= 1e9
    ? `${(bytes / 1e9).toFixed(1)} GB`
    : `${Math.max(1, Math.round(bytes / 1e6))} MB`;

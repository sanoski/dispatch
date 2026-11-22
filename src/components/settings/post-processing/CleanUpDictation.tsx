import React from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../../ui/ToggleSwitch";
import { useSettings } from "../../../hooks/useSettings";

interface CleanUpDictationProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

/** Dispatch: one hotkey, with cleanup switched on or off here. */
export const CleanUpDictation: React.FC<CleanUpDictationProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();

    // Needs Cleanup itself switched on (Debug settings).
    if (!(getSetting("post_process_enabled") ?? false)) return null;

    return (
      <ToggleSwitch
        checked={getSetting("dispatch_clean_up_dictation") ?? true}
        onChange={(enabled) =>
          updateSetting("dispatch_clean_up_dictation", enabled)
        }
        isUpdating={isUpdating("dispatch_clean_up_dictation")}
        label={t("dispatch.cleanUpDictation.label")}
        description={t("dispatch.cleanUpDictation.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
      />
    );
  },
);

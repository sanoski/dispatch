import React from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { useSettings } from "../../hooks/useSettings";

interface SmartSpacingProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

/** Dispatch: a space before pasted text when it would butt against a word. */
export const SmartSpacing: React.FC<SmartSpacingProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();

    return (
      <ToggleSwitch
        checked={getSetting("dispatch_smart_spacing") ?? true}
        onChange={(enabled) => updateSetting("dispatch_smart_spacing", enabled)}
        isUpdating={isUpdating("dispatch_smart_spacing")}
        label={t("dispatch.smartSpacing.label")}
        description={t("dispatch.smartSpacing.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
      />
    );
  },
);

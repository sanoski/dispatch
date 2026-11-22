import React from "react";
import { useTranslation } from "react-i18next";
import { useSettings } from "../../../hooks/useSettings";
import { Input } from "../../ui/Input";
import { SettingContainer } from "../../ui/SettingContainer";

// Matches POST_PROCESS_TIMEOUT_RANGE_SECS in src-tauri/src/dispatch.rs.
const MIN_SECONDS = 0.5;
const MAX_SECONDS = 60;

interface PostProcessTimeoutProps {
  descriptionMode?: "tooltip" | "inline";
  grouped?: boolean;
}

/** Dispatch: how long to wait for the cleanup model before pasting raw text. */
export const PostProcessTimeout: React.FC<PostProcessTimeoutProps> = ({
  descriptionMode = "tooltip",
  grouped = false,
}) => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();

  const seconds = getSetting("post_process_timeout_secs") ?? 3;

  const handleChange = (event: React.ChangeEvent<HTMLInputElement>) => {
    const value = parseFloat(event.target.value);
    if (!isNaN(value) && value >= MIN_SECONDS && value <= MAX_SECONDS) {
      updateSetting("post_process_timeout_secs", value);
    }
  };

  return (
    <SettingContainer
      title={t("dispatch.postProcessTimeout.title")}
      description={t("dispatch.postProcessTimeout.description")}
      descriptionMode={descriptionMode}
      grouped={grouped}
      layout="horizontal"
    >
      <div className="flex items-center space-x-2">
        <Input
          type="number"
          min={MIN_SECONDS}
          max={MAX_SECONDS}
          step="0.5"
          value={seconds}
          onChange={handleChange}
          disabled={isUpdating("post_process_timeout_secs")}
          className="w-20"
        />
        <span className="text-sm text-text">
          {t("dispatch.postProcessTimeout.seconds")}
        </span>
      </div>
    </SettingContainer>
  );
};

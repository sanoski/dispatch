// Dispatch: Settings > Advanced > History, in place of Handy's "History
// Limit" and "Auto-Delete Recordings" (those delete whole dictations).
import React from "react";
import { useTranslation } from "react-i18next";
import { Dropdown } from "../../ui/Dropdown";
import { SettingContainer } from "../../ui/SettingContainer";
import { KeepRecordingsDialog } from "./KeepRecordingsDialog";
import { NEWEST_KEPT, useKeepRecordings } from "./useKeepRecordings";

interface KeepRecordingsSettingProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const KeepRecordingsSetting: React.FC<KeepRecordingsSettingProps> = ({
  descriptionMode = "tooltip",
  grouped = false,
}) => {
  const { t } = useTranslation();
  const keep = useKeepRecordings();
  return (
    <>
      <SettingContainer
        title={t("dispatch.recordings.setting.title")}
        description={t("dispatch.recordings.setting.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
      >
        <Dropdown
          options={[
            { value: "all", label: t("dispatch.recordings.options.all") },
            {
              value: "newest",
              label: t("dispatch.recordings.options.newest", {
                count: NEWEST_KEPT,
              }),
            },
          ]}
          selectedValue={keep.keepAll ? "all" : "newest"}
          onSelect={(value) => {
            void keep.choose(value === "all");
          }}
          disabled={keep.busy}
        />
      </SettingContainer>
      <KeepRecordingsDialog
        pending={keep.pending}
        busy={keep.busy}
        onConfirm={keep.confirm}
        onCancel={keep.cancel}
      />
    </>
  );
};

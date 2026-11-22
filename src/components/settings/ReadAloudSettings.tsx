import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Play } from "lucide-react";
import { commands } from "@/bindings";
import { SettingsGroup } from "../ui/SettingsGroup";
import { SettingContainer } from "../ui/SettingContainer";
import { Dropdown } from "../ui/Dropdown";
import { Button } from "../ui/Button";
import { Slider } from "../ui/Slider";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { ShortcutInput } from "./ShortcutInput";
import { useSettings } from "../../hooks/useSettings";

const DEFAULT_VOICE = "bm_george";

/** Dispatch: the read-aloud hotkey, voice, speed and read-back switch
 * (dispatch_tts.rs), on the Read Aloud page. */
export const ReadAloudSettings: React.FC = () => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();
  const [voices, setVoices] = useState<string[]>([]);

  useEffect(() => {
    commands.getReadAloudVoices().then(setVoices);
  }, []);

  const voice = getSetting("dispatch_read_aloud_voice") ?? DEFAULT_VOICE;
  const speed = getSetting("dispatch_read_aloud_speed") ?? 1;

  // "bm_george" -> "George (British, male)"
  const describe = (id: string) => {
    const [kind, rawName = id] = id.split("_");
    const name = rawName.charAt(0).toUpperCase() + rawName.slice(1);
    return t("dispatch.readAloud.voice.option", {
      name,
      accent: t(
        kind?.startsWith("b")
          ? "dispatch.readAloud.voice.british"
          : "dispatch.readAloud.voice.american",
      ),
      gender: t(
        kind?.[1] === "m"
          ? "dispatch.readAloud.voice.male"
          : "dispatch.readAloud.voice.female",
      ),
    });
  };

  const options = voices
    .map((id) => ({ value: id, label: describe(id) }))
    .sort((a, b) => a.label.localeCompare(b.label));

  return (
    <SettingsGroup title={t("dispatch.readAloud.title")}>
      <ShortcutInput shortcutId="dispatch_read_aloud" grouped={true} />
      <SettingContainer
        title={t("dispatch.readAloud.voice.label")}
        description={t("dispatch.readAloud.voice.description")}
        descriptionMode="tooltip"
        grouped={true}
      >
        <div className="flex items-center gap-2">
          <Dropdown
            options={options}
            selectedValue={voice}
            onSelect={(value) =>
              updateSetting("dispatch_read_aloud_voice", value)
            }
            disabled={isUpdating("dispatch_read_aloud_voice")}
          />
          <Button
            variant="secondary"
            size="md"
            onClick={() => commands.previewReadAloudVoice(voice)}
            aria-label={t("dispatch.readAloud.voice.playLabel", {
              voice: describe(voice),
            })}
            title={t("dispatch.readAloud.voice.play")}
          >
            <Play className="h-4 w-4" />
          </Button>
        </div>
      </SettingContainer>
      <Slider
        value={speed}
        onChange={(value: number) =>
          updateSetting("dispatch_read_aloud_speed", value)
        }
        min={0.5}
        max={2}
        step={0.1}
        label={t("dispatch.readAloud.speed.label")}
        description={t("dispatch.readAloud.speed.description")}
        descriptionMode="tooltip"
        grouped
        formatValue={(value) => `${value.toFixed(1)}x`}
      />
      <ToggleSwitch
        checked={getSetting("dispatch_read_back") ?? false}
        onChange={(enabled) => updateSetting("dispatch_read_back", enabled)}
        isUpdating={isUpdating("dispatch_read_back")}
        label={t("dispatch.readAloud.readBack.label")}
        description={t("dispatch.readAloud.readBack.description")}
        descriptionMode="tooltip"
        grouped
      />
    </SettingsGroup>
  );
};

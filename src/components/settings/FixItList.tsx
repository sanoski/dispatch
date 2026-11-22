import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { ArrowRight, X } from "lucide-react";
import { notify } from "@/stores/noticeStore";
import type { FixRule } from "@/bindings";
import { useSettings } from "../../hooks/useSettings";
import { isComposingKeyEvent } from "../../lib/utils/keyboard";
import { Input } from "../ui/Input";
import { Button } from "../ui/Button";
import { SettingContainer } from "../ui/SettingContainer";

// Matches MAX_FIX_LEN in src-tauri/src/dispatch_fixes.rs.
const MAX_LENGTH = 100;

const tidy = (text: string) => text.replace(/\s+/g, " ").trim();

interface FixItListProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

/** Dispatch: exact "heard this, meant that" fixes applied to every dictation. */
export const FixItList: React.FC<FixItListProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();
    const [heard, setHeard] = useState("");
    const [meant, setMeant] = useState("");
    const rules: FixRule[] = getSetting("dispatch_fixes") ?? [];
    const busy = isUpdating("dispatch_fixes");
    const newHeard = tidy(heard);
    const newMeant = tidy(meant);
    const canAdd =
      !busy &&
      newHeard.length > 0 &&
      newMeant.length > 0 &&
      newHeard.length <= MAX_LENGTH &&
      newMeant.length <= MAX_LENGTH;

    const add = () => {
      if (!canAdd) return;
      const lower = newHeard.toLowerCase();
      if (rules.some((rule) => rule.heard.toLowerCase() === lower)) {
        notify({
          title: t("dispatch.fixItList.duplicate", { heard: newHeard }),
        });
        return;
      }
      updateSetting("dispatch_fixes", [
        ...rules,
        { heard: newHeard, meant: newMeant },
      ]);
      setHeard("");
      setMeant("");
    };

    const remove = (rule: FixRule) =>
      updateSetting(
        "dispatch_fixes",
        rules.filter((r) => r.heard !== rule.heard),
      );

    const onKeyDown = (e: React.KeyboardEvent) => {
      if (isComposingKeyEvent(e.nativeEvent)) return;
      if (e.key === "Enter") {
        e.preventDefault();
        add();
      }
    };

    return (
      <SettingContainer
        title={t("dispatch.fixItList.title")}
        description={t("dispatch.fixItList.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
        layout="stacked"
      >
        <div className="space-y-3">
          {rules.length > 0 && (
            <ul className="divide-y divide-line overflow-hidden rounded-lg border border-line">
              {rules.map((rule) => (
                <li
                  key={rule.heard}
                  className="group flex items-center gap-3 px-3 py-2 text-sm"
                >
                  <span className="min-w-0 flex-1 truncate text-ink-2 line-through decoration-ink-3/60">
                    {rule.heard}
                  </span>
                  <ArrowRight
                    className="h-4 w-4 shrink-0 text-logo-primary"
                    aria-hidden="true"
                  />
                  <span className="min-w-0 flex-1 truncate font-medium">
                    {rule.meant}
                  </span>
                  <button
                    type="button"
                    onClick={() => remove(rule)}
                    disabled={busy}
                    className="shrink-0 rounded-md p-1 text-ink-3 opacity-60 transition hover:bg-wash hover:text-text hover:opacity-100 focus-visible:opacity-100 focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-logo-primary group-hover:opacity-100 disabled:cursor-not-allowed"
                    aria-label={t("dispatch.fixItList.remove", {
                      heard: rule.heard,
                    })}
                  >
                    <X className="h-4 w-4" />
                  </button>
                </li>
              ))}
            </ul>
          )}
          <div className="flex items-center gap-2">
            <Input
              type="text"
              className="min-w-0 flex-1"
              value={heard}
              onChange={(e) => setHeard(e.target.value)}
              onKeyDown={onKeyDown}
              placeholder={t("dispatch.fixItList.heardPlaceholder")}
              aria-label={t("dispatch.fixItList.heardPlaceholder")}
              variant="compact"
              disabled={busy}
            />
            <ArrowRight
              className="h-4 w-4 shrink-0 text-ink-3"
              aria-hidden="true"
            />
            <Input
              type="text"
              className="min-w-0 flex-1"
              value={meant}
              onChange={(e) => setMeant(e.target.value)}
              onKeyDown={onKeyDown}
              placeholder={t("dispatch.fixItList.meantPlaceholder")}
              aria-label={t("dispatch.fixItList.meantPlaceholder")}
              variant="compact"
              disabled={busy}
            />
            <Button
              onClick={add}
              disabled={!canAdd}
              variant="primary"
              size="md"
            >
              {t("dispatch.fixItList.add")}
            </Button>
          </div>
        </div>
      </SettingContainer>
    );
  },
);

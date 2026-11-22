// Dispatch review window: shown before the first dictation, with Joshua's
// artwork (ARTWORK.md, "History empty state").
import React from "react";
import art from "@/assets/dispatch/art/history-empty.webp";
import { useTranslation } from "react-i18next";
import { useSettings } from "@/hooks/useSettings";
import { hotkeyKeys } from "./reviewUtils";

export const ReviewEmpty: React.FC = () => {
  const { t } = useTranslation();
  const { getSetting } = useSettings();
  const keys = hotkeyKeys(getSetting("bindings")?.transcribe?.current_binding);

  return (
    <div className="flex h-full flex-col items-center justify-center gap-6 p-8 text-center">
      <img
        src={art}
        alt=""
        width={240}
        height={240}
        draggable={false}
        className="h-[240px] w-[240px] rounded-[28px] border border-line object-cover shadow-[0_24px_48px_-28px_color-mix(in_srgb,var(--color-logo-primary),transparent_35%)]"
      />
      <div className="flex max-w-md flex-col gap-2">
        <h2 className="font-[Space_Grotesk,sans-serif] text-[26px] font-bold tracking-tight text-text">
          {t("dispatch.review.empty.title")}
        </h2>
        <p className="text-[15px] leading-relaxed text-ink-2">
          {t("dispatch.review.empty.before")}{" "}
          {keys.map((key, i) => (
            <React.Fragment key={key}>
              {i > 0 && ` ${t("dispatch.review.empty.plus")} `}
              <kbd className="rounded-md border border-line-strong bg-[var(--color-card)] px-1.5 py-px font-sans text-[13px] font-semibold text-text">
                {key}
              </kbd>
            </React.Fragment>
          ))}{" "}
          {t("dispatch.review.empty.after")}
        </p>
      </div>
    </div>
  );
};

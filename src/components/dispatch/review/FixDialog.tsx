// Dispatch review window: fixing a word (or a few) Dispatch misheard.
import React, { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Dialog } from "../../ui/Dialog";
import { Button } from "../../ui/Button";
import { InfoTooltip } from "../../ui/InfoTooltip";

interface FixDialogProps {
  open: boolean;
  /** What Dispatch heard, as shown. */
  heard: string;
  onCancel: () => void;
  onSave: (meant: string, everyTime: boolean) => Promise<void>;
}

/** Edge punctuation off, for showing the Fix-It rule ("coat," -> "coat"). */
const bare = (text: string) =>
  text
    .split(/\s+/)
    .map((w) => w.replace(/^[^\p{L}\p{N}]+|[^\p{L}\p{N}]+$/gu, ""))
    .filter(Boolean)
    .join(" ");

/** Everyday words: a rule on one of these would change it everywhere. */
const COMMON_WORDS = new Set(
  (
    "a an the and or but so if then than that this these those it its i me my " +
    "we us our you your he him his she her they them their is are was were be " +
    "been am do does did have has had will would can could should shall may " +
    "might must not no yes to of in on at by for from with about as into over " +
    "up down out off just like there here what which who when where why how " +
    "all any some one two get got go going make know think say said see want " +
    "okay ok well oh um uh yeah right now also too very really"
  ).split(" "),
);

/**
 * Why a Fix-It rule from this fix would likely do harm, or null. It would
 * apply to every dictation from now on, so a rule on an everyday word, or
 * one that only adds or drops words around what was heard ("for" to "for
 * it"), is almost never what's wanted.
 */
const ruleRisk = (ruleHeard: string, ruleMeant: string): boolean => {
  const heard = ruleHeard.toLowerCase().split(" ").filter(Boolean);
  const meant = ruleMeant.toLowerCase().split(" ").filter(Boolean);
  if (heard.every((w) => COMMON_WORDS.has(w) || w.length <= 2)) return true;
  const h = ` ${heard.join(" ")} `;
  const m = ` ${meant.join(" ")} `;
  return m.includes(h) || h.includes(m);
};

export const FixDialog: React.FC<FixDialogProps> = ({
  open,
  heard,
  onCancel,
  onSave,
}) => {
  const { t } = useTranslation();
  const inputRef = useRef<HTMLInputElement>(null);
  const [meant, setMeant] = useState(heard);
  // Off until ticked (Joshua, 2026-11-22): most mishearings are one-offs,
  // and a rule changes every dictation from then on. ruleRisk adds a
  // warning when a ticked rule looks likely to do harm.
  const [everyTime, setEveryTime] = useState(false);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (open) {
      setMeant(heard);
      setEveryTime(false);
      setSaving(false);
    }
  }, [open, heard]);

  const changed = meant.trim() !== heard.trim();
  const ruleHeard = bare(heard);
  const ruleMeant = bare(meant);
  const sameWords = ruleHeard.toLowerCase() === ruleMeant.toLowerCase();
  const canAddRule =
    changed && ruleHeard !== "" && ruleMeant !== "" && !sameWords;
  const risky = canAddRule && ruleRisk(ruleHeard, ruleMeant);
  const addRule = canAddRule && everyTime;

  const save = async () => {
    if (!changed || saving) return;
    setSaving(true);
    try {
      await onSave(meant, addRule);
    } finally {
      setSaving(false);
    }
  };

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => !next && onCancel()}
      title={t("dispatch.review.fix.title")}
      closeLabel={t("dispatch.review.fix.cancel")}
      initialFocusRef={inputRef as React.RefObject<HTMLElement>}
      footer={
        <div className="flex w-full flex-wrap items-center gap-3">
          <span className="min-w-48 flex-1 text-xs leading-relaxed text-ink-3">
            {t("dispatch.review.fix.note")}
          </span>
          <Button variant="secondary" size="md" onClick={onCancel}>
            {t("dispatch.review.fix.cancel")}
          </Button>
          <Button
            variant="primary"
            size="md"
            onClick={save}
            disabled={!changed || saving}
          >
            {t("dispatch.review.fix.save")}
          </Button>
        </div>
      }
    >
      <form
        className="flex flex-col gap-4"
        onSubmit={(event) => {
          event.preventDefault();
          void save();
        }}
      >
        <div className="flex items-center gap-2.5 text-sm text-ink-2">
          <span>{t("dispatch.review.fix.heard")}</span>
          <span className="text-text line-through decoration-warning/85">
            {heard}
          </span>
        </div>
        <label className="flex flex-col gap-1.5">
          <span className="text-xs font-semibold uppercase tracking-wide text-ink-3">
            {t("dispatch.review.fix.meant")}
          </span>
          <input
            ref={inputRef}
            type="text"
            value={meant}
            onChange={(event) => setMeant(event.target.value)}
            onFocus={(event) => event.target.select()}
            className="h-11 rounded-lg border border-logo-primary bg-[var(--color-card)] px-3 text-base text-text shadow-[0_0_0_3px_color-mix(in_srgb,var(--color-logo-primary),transparent_82%)] focus:outline-none"
          />
        </label>
        <div className="flex flex-col gap-0.5">
          <div className="flex items-center gap-2">
            <label
              className={`flex items-center gap-2.5 ${canAddRule ? "cursor-pointer" : "opacity-50"}`}
            >
              <input
                type="checkbox"
                checked={addRule}
                disabled={!canAddRule}
                onChange={(event) => setEveryTime(event.target.checked)}
                className="h-4.5 w-4.5 accent-[var(--color-logo-primary)]"
              />
              <span className="text-sm font-semibold text-text">
                {t("dispatch.review.fix.everyTime")}
              </span>
            </label>
            <InfoTooltip
              title={t("dispatch.review.fix.everyTimeHelpTitle")}
              text={t("dispatch.review.fix.everyTimeHelp")}
            />
          </div>
          <span
            className={`pl-7 text-[13px] leading-snug ${canAddRule ? "" : "opacity-50"} ${addRule && risky ? "text-warning" : "text-ink-2"}`}
          >
            {!canAddRule
              ? changed && sameWords && ruleHeard !== ""
                ? t("dispatch.review.fix.everyTimeSameWords")
                : t("dispatch.review.fix.everyTimeUnavailable")
              : addRule && risky
                ? t("dispatch.review.fix.everyTimeRisky", {
                    heard: ruleHeard,
                    meant: ruleMeant,
                  })
                : t("dispatch.review.fix.everyTimeDescription", {
                    heard: ruleHeard,
                    meant: ruleMeant,
                  })}
          </span>
        </div>
      </form>
    </Dialog>
  );
};

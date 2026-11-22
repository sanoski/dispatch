// Dispatch review window: one dictation, its waveform and its words.
import React, { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  ArrowLeft,
  ArrowRight,
  Check,
  CheckCheck,
  Copy,
  Loader2,
  Pause,
  PencilLine,
  Play,
  RotateCcw,
  Star,
  Trash2,
} from "lucide-react";
import {
  commands,
  type DictationReview,
  type HistoryEntry,
  type ReviewWord,
} from "@/bindings";
import { notify } from "@/stores/noticeStore";
import { useSettingsStore } from "@/stores/settingsStore";
import type { ActiveJobStatus } from "@/stores/transcriptionJobsStore";
import { copyToClipboard } from "../../settings/history/clipboard";
import { FixDialog } from "./FixDialog";
import { Waveform } from "./Waveform";
import { useDictationAudio } from "./useDictationAudio";
import { clock, dayLabel, length, timeLabel } from "./reviewUtils";

type View = "cleaned" | "heard";

interface DictationDetailProps {
  entry: HistoryEntry;
  job?: ActiveJobStatus;
  /** Narrow layout: the list is hidden, so offer a way back to it. */
  onBack?: () => void;
  onToggleSaved: () => void;
  onDelete: () => Promise<void>;
  /** A fix was saved or the "Looks right" mark changed. */
  onFixesChanged: (
    fileName: string,
    fixCount: number,
    checked: boolean,
  ) => void;
}

const iconButton =
  "grid h-9.5 w-9.5 place-items-center rounded-[9px] border border-line-strong bg-[var(--color-card)] text-ink-2 transition-colors hover:text-text cursor-pointer disabled:cursor-not-allowed disabled:opacity-40 focus:outline-none focus-visible:ring-2 focus-visible:ring-logo-primary";

const chip =
  "rounded-md border border-line bg-[var(--color-card)] px-2.5 py-0.5 text-xs text-ink-2";

const switchTrack =
  "relative h-5 w-9 shrink-0 rounded-full bg-line-strong transition-colors peer-checked:bg-logo-primary peer-focus-visible:ring-2 peer-focus-visible:ring-logo-primary after:absolute after:left-0.5 after:top-0.5 after:h-4 after:w-4 after:rounded-full after:bg-[var(--color-on-accent)] after:transition-transform peer-checked:after:translate-x-4";

/** "Quiet background" is remembered on this computer. */
const QUIET_KEY = "dispatch.review.quietBackground";

const readQuietSetting = (): boolean => {
  try {
    return localStorage.getItem(QUIET_KEY) === "1";
  } catch {
    return false;
  }
};

const saveQuietSetting = (on: boolean) => {
  try {
    localStorage.setItem(QUIET_KEY, on ? "1" : "0");
  } catch {
    // Not remembered; it still works for now.
  }
};

export const DictationDetail: React.FC<DictationDetailProps> = ({
  entry,
  job,
  onBack,
  onToggleSaved,
  onDelete,
  onFixesChanged,
}) => {
  const { t, i18n } = useTranslation();
  const [review, setReview] = useState<DictationReview | null>(null);
  const [quiet, setQuiet] = useState(readQuietSetting);
  const audio = useDictationAudio(entry.file_name, review?.peak ?? null, quiet);
  const [loadError, setLoadError] = useState(false);
  const [view, setView] = useState<View>("cleaned");
  const [selection, setSelection] = useState<{
    first: number;
    last: number;
  } | null>(null);
  const [showTimes, setShowTimes] = useState(false);
  const [fixing, setFixing] = useState(false);
  const [copied, setCopied] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);

  useEffect(() => {
    if (audio.quietFailed) {
      notify({ title: t("dispatch.review.player.quietFailed") });
    }
  }, [audio.quietFailed, t]);

  const heardText = entry.transcription_text;
  const cleanedText = entry.post_processed_text?.trim()
    ? entry.post_processed_text
    : null;

  useEffect(() => {
    let cancelled = false;
    setReview(null);
    setLoadError(false);
    setSelection(null);
    if (!heardText.trim()) return;
    commands
      .getDictationReview(entry.file_name, heardText, cleanedText)
      .then((result) => {
        if (cancelled) return;
        if (result.status === "ok") setReview(result.data);
        else setLoadError(true);
      })
      .catch(() => !cancelled && setLoadError(true));
    return () => {
      cancelled = true;
    };
  }, [entry.file_name, heardText, cleanedText]);

  useEffect(
    () => setView(cleanedText ? "cleaned" : "heard"),
    [entry.id, cleanedText],
  );
  useEffect(() => setConfirmDelete(false), [entry.id]);

  const words: ReviewWord[] = useMemo(() => {
    if (!review) return [];
    return view === "cleaned" && review.cleaned ? review.cleaned : review.heard;
  }, [review, view]);
  // Fixes are made only to what Dispatch heard: that's the text the voice
  // training learns from. The Cleaned tab is for reading and listening, with
  // a way across.
  const editable = !(view === "cleaned" && review?.cleaned);

  const range = useMemo(() => {
    if (!selection || words.length === 0) return null;
    const first = words[selection.first];
    const last = words[selection.last];
    if (!first || !last) return null;
    return { startMs: first.start_ms, endMs: last.end_ms };
  }, [selection, words]);

  // The heard words the selection stands for: what a fix replaces.
  const heardRange = useMemo(() => {
    if (!selection || !review) return null;
    const picked = words.slice(selection.first, selection.last + 1);
    const firsts = picked
      .map((w) => w.heard_first)
      .filter((n): n is number => n !== null);
    const lasts = picked
      .map((w) => w.heard_last)
      .filter((n): n is number => n !== null);
    if (firsts.length === 0) return null;
    const first = Math.min(...firsts);
    const last = Math.max(...lasts);
    const text = review.heard
      .filter(
        (w) =>
          w.heard_first !== null &&
          w.heard_first >= first &&
          (w.heard_last ?? 0) <= last,
      )
      .map((w) => w.text)
      .join(" ");
    return { first, last, text };
  }, [selection, words, review]);

  const playingIndex = audio.playing
    ? words.findIndex(
        (w) => audio.positionMs >= w.start_ms && audio.positionMs < w.end_ms,
      )
    : -1;

  const pick = (index: number, extend: boolean) => {
    const next =
      extend && selection
        ? {
            first: Math.min(selection.first, index),
            last: Math.max(selection.last, index),
          }
        : { first: index, last: index };
    setSelection(next);
    const first = words[next.first];
    const last = words[next.last];
    if (first && last) audio.playRange(first.start_ms, last.end_ms);
  };

  const saveFix = useCallback(
    async (meant: string, everyTime: boolean) => {
      if (!heardRange) return;
      const result = await commands.saveDictationFix(
        entry.file_name,
        heardText,
        cleanedText,
        heardRange.first,
        heardRange.last,
        meant,
        everyTime,
      );
      if (result.status !== "ok") {
        notify({ title: t("dispatch.review.fix.error") });
        return;
      }
      setReview(result.data);
      setFixing(false);
      setView("heard");
      const fixedIndex = result.data.heard.findIndex(
        (w) => w.heard_first === heardRange.first,
      );
      setSelection(
        fixedIndex >= 0 ? { first: fixedIndex, last: fixedIndex } : null,
      );
      onFixesChanged(
        entry.file_name,
        result.data.fix_count,
        result.data.checked,
      );
      // The rule went straight into the settings file; Settings' copy of the
      // Fix-It List must catch up, or saving there would drop it.
      if (everyTime) void useSettingsStore.getState().refreshSettings();
      notify({
        title: everyTime
          ? t("dispatch.review.fix.savedWithRule")
          : t("dispatch.review.fix.saved"),
      });
    },
    [heardRange, entry.file_name, heardText, cleanedText, onFixesChanged, t],
  );

  // From the Cleaned tab: the same words in What Dispatch heard, picked.
  const fixInHeard = () => {
    if (!heardRange || !review) return;
    const indices = review.heard
      .map((w, i) => ({ w, i }))
      .filter(
        ({ w }) =>
          w.heard_first !== null &&
          w.heard_first >= heardRange.first &&
          (w.heard_last ?? w.heard_first) <= heardRange.last,
      )
      .map(({ i }) => i);
    setView("heard");
    if (indices.length === 0) {
      setSelection(null);
      return;
    }
    const first = Math.min(...indices);
    const last = Math.max(...indices);
    setSelection({ first, last });
    audio.playRange(review.heard[first].start_ms, review.heard[last].end_ms);
  };

  // "Looks right": the text (after any fixes) is exactly what was said.
  const [checking, setChecking] = useState(false);
  const toggleChecked = async () => {
    if (!review || checking) return;
    setChecking(true);
    try {
      const result = await commands.setDictationChecked(
        entry.file_name,
        heardText,
        cleanedText,
        !review.checked,
      );
      if (result.status !== "ok") {
        notify({ title: t("dispatch.review.checked.error") });
        return;
      }
      setReview(result.data);
      onFixesChanged(
        entry.file_name,
        result.data.fix_count,
        result.data.checked,
      );
    } finally {
      setChecking(false);
    }
  };

  const copyText = async () => {
    const text =
      view === "cleaned" && cleanedText
        ? cleanedText
        : (review?.heard.map((w) => w.text).join(" ") ?? heardText);
    if (await copyToClipboard(text)) {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } else {
      notify({ title: t("settings.history.copyError") });
    }
  };

  const deleteIt = async () => {
    if (!confirmDelete) {
      setConfirmDelete(true);
      setTimeout(() => setConfirmDelete(false), 3000);
      return;
    }
    try {
      await onDelete();
    } catch {
      notify({ title: t("settings.history.deleteError") });
    }
  };

  const selectedWords = selection
    ? words.slice(selection.first, selection.last + 1)
    : [];
  const selectedLabel = selectedWords.map((w) => w.text).join(" ");
  const selectedFix =
    selectedWords.length === 1 ? selectedWords[0].fixed_from : null;
  const selectedNote = !selection
    ? t("dispatch.review.selected.none")
    : selectedFix
      ? t("dispatch.review.selected.fixed", { heard: selectedFix })
      : !heardRange
        ? t("dispatch.review.selected.addedByCleanup")
        : !editable
          ? t("dispatch.review.selected.fixInHeard")
          : selectedWords.some((w) => w.precision === "estimated")
            ? t("dispatch.review.selected.estimated")
            : t("dispatch.review.selected.aligned");

  const title = t("dispatch.review.at", {
    day: dayLabel(entry.timestamp, t, i18n.language),
    time: timeLabel(entry.timestamp, i18n.language),
  });
  const timedByModel = review && review.timing_source !== "estimate";
  const busy = Boolean(job);

  const transcriptBody = () => {
    if (busy) {
      return (
        <p className="text-sm italic text-ink-2">
          {job?.state === "queued"
            ? t("settings.history.waiting")
            : t("settings.history.transcribing")}
        </p>
      );
    }
    if (!heardText.trim()) {
      return (
        <p className="text-sm italic text-ink-2">
          {t("settings.history.notTranscribed")}
        </p>
      );
    }
    if (loadError) {
      return (
        <p className="text-sm text-ink-2">{t("dispatch.review.loadFailed")}</p>
      );
    }
    if (!review) {
      return (
        <p className="text-sm text-ink-3">{t("settings.history.loading")}</p>
      );
    }
    return (
      <div className="flex flex-wrap content-start gap-x-1 gap-y-1.5 text-[17px] leading-relaxed">
        {words.map((word, i) => {
          const selected =
            selection !== null && i >= selection.first && i <= selection.last;
          const playing = i === playingIndex;
          return (
            <button
              key={`${view}-${i}`}
              type="button"
              onClick={(event) => pick(i, event.shiftKey)}
              onDoubleClick={() => {
                setSelection({ first: i, last: i });
                if (editable && words[i].heard_first !== null) setFixing(true);
              }}
              title={
                word.fixed_from
                  ? t("dispatch.review.word.fixedFrom", {
                      heard: word.fixed_from,
                    })
                  : undefined
              }
              className={`flex cursor-pointer flex-col items-start rounded-md px-1 py-0.5 text-left transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-logo-primary ${
                selected
                  ? "bg-logo-primary font-medium text-[var(--color-on-accent)]"
                  : playing
                    ? "bg-logo-primary/18 text-text"
                    : "text-text hover:bg-logo-primary/10"
              }`}
            >
              {showTimes && (
                <span
                  className={`text-[10px] tabular-nums ${selected ? "text-[var(--color-on-accent)]/75" : "text-ink-3"}`}
                >
                  {clock(word.start_ms)}
                </span>
              )}
              <span
                className={`${word.fixed_from && !selected ? "underline decoration-logo-primary decoration-dotted decoration-2 underline-offset-4" : ""} ${word.precision === "estimated" && !selected ? "opacity-80" : ""}`}
              >
                {word.text}
              </span>
            </button>
          );
        })}
      </div>
    );
  };

  return (
    <div className="flex flex-auto flex-col gap-4">
      <div className="flex flex-wrap items-start justify-between gap-4">
        <div className="flex min-w-0 items-start gap-3">
          {onBack && (
            <button
              type="button"
              onClick={onBack}
              aria-label={t("dispatch.review.back")}
              className={iconButton}
            >
              <ArrowLeft className="h-4.5 w-4.5" />
            </button>
          )}
          <div className="flex min-w-0 flex-col gap-2">
            <h2 className="font-[Space_Grotesk,sans-serif] text-[22px] font-bold tracking-tight text-text">
              {title}
            </h2>
            <div className="flex flex-wrap gap-2">
              {review && (
                <span className={chip}>{length(review.duration_ms)}</span>
              )}
              {review && (
                <span className={chip}>
                  {timedByModel
                    ? t("dispatch.review.chips.timedByParakeet")
                    : t("dispatch.review.chips.timesEstimated")}
                </span>
              )}
              {cleanedText && (
                <span className={chip}>
                  {t("dispatch.review.chips.cleaned")}
                </span>
              )}
              {review && review.fix_count > 0 && (
                <span className="rounded-md border border-logo-primary/35 bg-logo-primary/12 px-2.5 py-0.5 text-xs text-text">
                  {t("dispatch.review.chips.fixes", {
                    count: review.fix_count,
                  })}
                </span>
              )}
              {review?.checked && (
                <span className="flex items-center gap-1 rounded-md border border-[color-mix(in_srgb,var(--color-approved),transparent_60%)] bg-[color-mix(in_srgb,var(--color-approved),transparent_90%)] px-2.5 py-0.5 text-xs text-text">
                  <CheckCheck
                    className="h-3.5 w-3.5 text-[var(--color-approved)]"
                    aria-hidden="true"
                  />
                  {t("dispatch.review.chips.checked")}
                </span>
              )}
            </div>
          </div>
        </div>
        <div className="flex gap-2">
          <button
            type="button"
            onClick={copyText}
            className={iconButton}
            aria-label={t("settings.history.copyToClipboard")}
            title={t("settings.history.copyToClipboard")}
          >
            {copied ? (
              <Check className="h-4.5 w-4.5" />
            ) : (
              <Copy className="h-4.5 w-4.5" />
            )}
          </button>
          <button
            type="button"
            onClick={onToggleSaved}
            className={`${iconButton} ${entry.saved ? "text-logo-primary" : ""}`}
            aria-pressed={entry.saved}
            aria-label={
              entry.saved
                ? t("settings.history.unsave")
                : t("settings.history.save")
            }
            title={
              entry.saved
                ? t("settings.history.unsave")
                : t("settings.history.save")
            }
          >
            <Star
              className="h-4.5 w-4.5"
              fill={entry.saved ? "currentColor" : "none"}
            />
          </button>
          <button
            type="button"
            onClick={() =>
              void commands.retryHistoryEntryTranscription(entry.id)
            }
            disabled={busy}
            className={iconButton}
            aria-label={t("settings.history.retranscribe")}
            title={t("settings.history.retranscribe")}
          >
            <RotateCcw className="h-4.5 w-4.5" />
          </button>
          <button
            type="button"
            onClick={deleteIt}
            className={`${iconButton} ${confirmDelete ? "border-error text-error hover:text-error" : ""}`}
            aria-label={
              confirmDelete
                ? t("dispatch.review.confirmDelete")
                : t("settings.history.delete")
            }
            title={
              confirmDelete
                ? t("dispatch.review.confirmDelete")
                : t("settings.history.delete")
            }
          >
            <Trash2 className="h-4.5 w-4.5" />
          </button>
        </div>
      </div>

      <section
        aria-label={t("dispatch.review.player.label")}
        className="flex flex-col gap-3 rounded-[14px] border border-line bg-[var(--color-card)] px-5 pb-4 pt-4.5"
      >
        <div className="flex items-center gap-3.5">
          <button
            type="button"
            onClick={audio.toggle}
            disabled={!audio.ready}
            aria-label={
              audio.playing
                ? t("dispatch.review.player.pause")
                : t("dispatch.review.player.play")
            }
            className="grid h-10 w-10 shrink-0 cursor-pointer place-items-center rounded-full bg-logo-primary text-[var(--color-on-accent)] disabled:cursor-not-allowed disabled:opacity-40 focus:outline-none focus-visible:ring-2 focus-visible:ring-logo-primary focus-visible:ring-offset-2"
          >
            {audio.playing ? (
              <Pause className="h-4 w-4" fill="currentColor" />
            ) : (
              <Play className="h-4 w-4 translate-x-px" fill="currentColor" />
            )}
          </button>
          <span className="text-sm tabular-nums text-text">
            {clock(audio.positionMs, true)}{" "}
            <span className="text-ink-3">
              {t("dispatch.review.player.ofDuration", {
                duration: clock(review?.duration_ms ?? 0, true),
              })}
            </span>
          </span>
          <button
            type="button"
            onClick={audio.nextSpeed}
            aria-label={t("dispatch.review.player.speed", {
              speed: audio.speed,
            })}
            className="cursor-pointer rounded-md border border-line-strong px-2 py-1 text-xs font-semibold text-ink-2 hover:text-text"
          >
            {t("dispatch.review.player.speedValue", { speed: audio.speed })}
          </button>
          <div className="flex-1" />
          <div className="flex flex-wrap items-center justify-end gap-x-5 gap-y-2">
            <label
              className="flex cursor-pointer items-center gap-2.5 text-[13px] text-ink-2"
              title={t("dispatch.review.player.quietHint")}
            >
              {audio.quieting && (
                <Loader2
                  className="h-3.5 w-3.5 animate-spin text-ink-3"
                  aria-label={t("dispatch.review.player.quieting")}
                />
              )}
              {t("dispatch.review.player.quiet")}
              <input
                type="checkbox"
                role="switch"
                checked={quiet}
                aria-describedby="dispatch-quiet-hint"
                onChange={(event) => {
                  setQuiet(event.target.checked);
                  saveQuietSetting(event.target.checked);
                }}
                className="peer sr-only"
              />
              <span className={switchTrack} />
              <span id="dispatch-quiet-hint" className="sr-only">
                {t("dispatch.review.player.quietHint")}
              </span>
            </label>
            <label className="flex cursor-pointer items-center gap-2.5 text-[13px] text-ink-2">
              {t("dispatch.review.player.showTimes")}
              <input
                type="checkbox"
                role="switch"
                checked={showTimes}
                onChange={(event) => setShowTimes(event.target.checked)}
                className="peer sr-only"
              />
              <span className={switchTrack} />
            </label>
          </div>
        </div>
        {review && !review.has_audio ? (
          <p className="py-8 text-center text-sm text-ink-2">
            {t("dispatch.review.noAudio")}
          </p>
        ) : audio.failed ? (
          <p className="py-8 text-center text-sm text-ink-2">
            {t("dispatch.review.loadFailed")}
          </p>
        ) : (
          <Waveform
            waveform={review?.waveform ?? []}
            durationMs={review?.duration_ms ?? 0}
            positionMs={audio.positionMs}
            words={words}
            selection={range}
            onSeek={audio.seek}
          />
        )}
      </section>

      <section
        aria-label={t("dispatch.review.transcriptLabel")}
        className="flex flex-auto flex-col gap-3.5 rounded-[14px] border border-line bg-[var(--color-card)] px-5 py-4"
      >
        <div className="flex flex-wrap items-center justify-between gap-3">
          {cleanedText ? (
            <div
              role="group"
              aria-label={t("dispatch.review.tabs.label")}
              className="flex rounded-[9px] border border-line bg-background p-0.5"
            >
              {(["cleaned", "heard"] as const).map((v) => (
                <button
                  key={v}
                  type="button"
                  aria-pressed={view === v}
                  onClick={() => {
                    setView(v);
                    setSelection(null);
                  }}
                  className={`cursor-pointer rounded-md px-3 py-1.5 text-[13px] font-semibold transition-colors ${
                    view === v
                      ? "bg-logo-primary text-[var(--color-on-accent)]"
                      : "text-ink-2 hover:text-text"
                  }`}
                >
                  {t(`dispatch.review.tabs.${v}`)}
                </button>
              ))}
            </div>
          ) : (
            <span className="text-[13px] font-semibold text-ink-2">
              {t("dispatch.review.tabs.heard")}
            </span>
          )}
          <div className="flex flex-wrap items-center justify-end gap-x-4 gap-y-2">
            <span className="text-xs text-ink-3">
              {editable
                ? t("dispatch.review.hint")
                : t("dispatch.review.hintCleaned")}
            </span>
            {review && (
              <button
                type="button"
                onClick={toggleChecked}
                disabled={checking || busy}
                aria-pressed={review.checked}
                title={
                  review.checked
                    ? t("dispatch.review.checked.undoHint")
                    : t("dispatch.review.checked.hint")
                }
                className={`flex cursor-pointer items-center gap-1.5 rounded-full px-3.5 py-1.5 text-[13px] font-semibold transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-logo-primary disabled:cursor-not-allowed disabled:opacity-50 ${
                  review.checked
                    ? "bg-[var(--color-approved)] text-[var(--color-on-accent)] hover:brightness-110"
                    : "border border-line-strong text-text hover:border-[var(--color-approved)]"
                }`}
              >
                {review.checked ? (
                  <CheckCheck className="h-4 w-4" aria-hidden="true" />
                ) : (
                  <Check className="h-4 w-4" aria-hidden="true" />
                )}
                {review.checked
                  ? t("dispatch.review.checked.done")
                  : t("dispatch.review.checked.button")}
              </button>
            )}
          </div>
        </div>

        {/* Sized by its words; the page scrolls when there are many. */}
        <div className="min-h-24 flex-auto">{transcriptBody()}</div>

        <div className="flex flex-wrap items-center gap-4 rounded-[10px] border border-line bg-background px-3.5 py-3">
          <div className="flex min-w-36 max-w-64 flex-col gap-0.5">
            <span className="truncate font-[Space_Grotesk,sans-serif] text-lg font-bold text-text">
              {selection
                ? selectedLabel
                : t("dispatch.review.selected.nothing")}
            </span>
            <span className="text-xs tabular-nums text-ink-3">
              {range
                ? t("dispatch.review.selected.range", {
                    start: clock(range.startMs, true),
                    end: clock(range.endMs, true),
                  })
                : ""}
            </span>
          </div>
          <span className="min-w-40 flex-1 text-[13px] leading-snug text-ink-2">
            {selectedNote}
          </span>
          <button
            type="button"
            disabled={!range || !audio.ready}
            onClick={() => range && audio.playRange(range.startMs, range.endMs)}
            className="flex cursor-pointer items-center gap-1.5 rounded-lg bg-logo-primary px-3.5 py-2 text-[13px] font-semibold text-[var(--color-on-accent)] disabled:cursor-not-allowed disabled:opacity-40"
          >
            <Play className="h-3.5 w-3.5" fill="currentColor" />
            {t("dispatch.review.playWord")}
          </button>
          <button
            type="button"
            disabled={!heardRange || busy}
            onClick={() => (editable ? setFixing(true) : fixInHeard())}
            className="flex cursor-pointer items-center gap-1.5 rounded-lg border border-line-strong px-3.5 py-2 text-[13px] font-semibold text-text disabled:cursor-not-allowed disabled:opacity-40"
          >
            {editable ? (
              <PencilLine className="h-3.5 w-3.5" />
            ) : (
              <ArrowRight className="h-3.5 w-3.5" />
            )}
            {editable
              ? t("dispatch.review.fixButton")
              : t("dispatch.review.fixInHeard")}
          </button>
        </div>
      </section>

      <FixDialog
        open={fixing && heardRange !== null}
        heard={heardRange?.text ?? ""}
        onCancel={() => setFixing(false)}
        onSave={saveFix}
      />
    </div>
  );
};

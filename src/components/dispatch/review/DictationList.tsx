// Dispatch review window: the list of dictations, grouped by day.
import React from "react";
import { useTranslation } from "react-i18next";
import {
  CheckCheck,
  FileAudio,
  FilePlus,
  FolderOpen,
  Search,
  Star,
} from "lucide-react";
import type { DictationSummary, HistoryEntry } from "@/bindings";
import type { ActiveJobStatus } from "@/stores/transcriptionJobsStore";
import type { FileRead } from "../../settings/history/transcribeFiles";
import { groupByDay, length, shownText, timeLabel } from "./reviewUtils";

export type ListFilter = "all" | "cleaned" | "fixed" | "checked" | "starred";
const FILTERS: ListFilter[] = ["all", "cleaned", "fixed", "checked", "starred"];

interface DictationListProps {
  entries: HistoryEntry[];
  /** Every dictation in History, not just the pages loaded so far. */
  total: number | null;
  summaries: Record<string, DictationSummary>;
  jobs: Record<number, ActiveJobStatus>;
  reads: FileRead[];
  selectedId: number | null;
  onSelect: (id: number) => void;
  filter: ListFilter;
  onFilter: (filter: ListFilter) => void;
  query: string;
  onQuery: (query: string) => void;
  loading: boolean;
  sentinelRef: React.RefObject<HTMLDivElement>;
  onTranscribeFile: () => void;
  onOpenFolder: () => void;
}

const toolButton =
  "grid h-8 w-8 cursor-pointer place-items-center rounded-lg text-ink-2 hover:bg-logo-primary/10 hover:text-text focus:outline-none focus-visible:ring-2 focus-visible:ring-logo-primary";

export const DictationList: React.FC<DictationListProps> = ({
  entries,
  total,
  summaries,
  jobs,
  reads,
  selectedId,
  onSelect,
  filter,
  onFilter,
  query,
  onQuery,
  loading,
  sentinelRef,
  onTranscribeFile,
  onOpenFolder,
}) => {
  const { t, i18n } = useTranslation();
  const groups = groupByDay(entries, t, i18n.language);

  return (
    <section
      aria-label={t("dispatch.review.listLabel")}
      className="flex h-full min-h-0 flex-col"
    >
      <div className="flex flex-col gap-3 px-4.5 pb-3.5 pt-5">
        <div className="flex items-center justify-between gap-2">
          <div className="flex items-baseline gap-2.5">
            <h1 className="font-[Space_Grotesk,sans-serif] text-[22px] font-bold tracking-tight text-text">
              {t("dispatch.review.title")}
            </h1>
            <span className="text-xs text-ink-3">
              {total !== null && t("dispatch.review.count", { count: total })}
            </span>
          </div>
          <div className="flex gap-1">
            <button
              type="button"
              onClick={onTranscribeFile}
              className={toolButton}
              aria-label={t("settings.history.transcribeFile")}
              title={t("settings.history.transcribeFile")}
            >
              <FilePlus className="h-4 w-4" />
            </button>
            <button
              type="button"
              onClick={onOpenFolder}
              className={toolButton}
              aria-label={t("settings.history.openFolder")}
              title={t("settings.history.openFolder")}
            >
              <FolderOpen className="h-4 w-4" />
            </button>
          </div>
        </div>
        <label className="flex h-9 items-center gap-2 rounded-[9px] border border-line-strong bg-[var(--color-card)] px-3 focus-within:border-logo-primary">
          <Search className="h-4 w-4 shrink-0 text-ink-3" aria-hidden="true" />
          <input
            type="search"
            value={query}
            onChange={(event) => onQuery(event.target.value)}
            aria-label={t("dispatch.review.search")}
            placeholder={t("dispatch.review.search")}
            className="min-w-0 flex-1 bg-transparent text-sm text-text placeholder:text-ink-3 focus:outline-none"
          />
        </label>
        <div
          role="group"
          aria-label={t("dispatch.review.filters.label")}
          className="flex flex-wrap gap-1.5"
        >
          {FILTERS.map((f) => (
            <button
              key={f}
              type="button"
              aria-pressed={filter === f}
              onClick={() => onFilter(f)}
              className={`cursor-pointer rounded-full border px-3 py-1 text-xs font-semibold transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-logo-primary ${
                filter === f
                  ? "border-logo-primary bg-logo-primary/14 text-text"
                  : "border-line-strong text-ink-2 hover:text-text"
              }`}
            >
              {t(`dispatch.review.filters.${f}`)}
            </button>
          ))}
        </div>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto px-2.5 pb-3">
        {reads.map((read) => (
          <div
            key={`read-${read.id}`}
            className="mx-1 mb-1 flex items-center gap-2 rounded-xl px-3 py-3 text-sm text-ink-2"
          >
            <FileAudio className="h-4 w-4 shrink-0" aria-hidden="true" />
            <span className="min-w-0 flex-1 truncate">{read.name}</span>
            <span className="shrink-0 animate-pulse text-xs italic">
              {t("settings.history.readingFile")}
            </span>
          </div>
        ))}
        {loading && entries.length === 0 && (
          <p className="px-3 py-6 text-center text-sm text-ink-3">
            {t("settings.history.loading")}
          </p>
        )}
        {!loading && entries.length === 0 && reads.length === 0 && (
          <p className="px-3 py-6 text-center text-sm text-ink-3">
            {t("dispatch.review.noMatches")}
          </p>
        )}
        {groups.map((group) => (
          <div key={group.label} className="flex flex-col gap-1">
            <div className="px-2 pb-1.5 pt-2.5 text-[11px] font-semibold uppercase tracking-widest text-ink-3">
              {group.label}
            </div>
            {group.entries.map((entry) => {
              const summary = summaries[entry.file_name];
              const selected = entry.id === selectedId;
              const job = jobs[entry.id];
              // Marked "Looks right": green, so approved ones stand out.
              const approved = Boolean(summary?.checked);
              return (
                <button
                  key={entry.id}
                  type="button"
                  onClick={() => onSelect(entry.id)}
                  aria-current={selected ? "true" : undefined}
                  className={`flex w-full cursor-pointer flex-col gap-1.5 rounded-xl border px-3 py-2.5 text-left transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-logo-primary ${
                    selected
                      ? approved
                        ? "border-[color-mix(in_srgb,var(--color-approved),transparent_45%)] bg-[color-mix(in_srgb,var(--color-approved),var(--color-card)_90%)] shadow-[0_10px_24px_-18px_color-mix(in_srgb,var(--color-approved),transparent_40%)]"
                        : "border-logo-primary/45 bg-[var(--color-card)] shadow-[0_10px_24px_-18px_color-mix(in_srgb,var(--color-logo-primary),transparent_40%)]"
                      : approved
                        ? "border-[color-mix(in_srgb,var(--color-approved),transparent_70%)] bg-[color-mix(in_srgb,var(--color-approved),transparent_93%)] hover:bg-[color-mix(in_srgb,var(--color-approved),transparent_88%)]"
                        : "border-transparent hover:bg-[var(--color-card)]"
                  }`}
                >
                  <div className="flex items-center justify-between gap-2">
                    <span className="flex items-center gap-1.5 text-xs tabular-nums text-ink-3">
                      {entry.source_name && (
                        <FileAudio className="h-3.5 w-3.5" aria-hidden="true" />
                      )}
                      {summary
                        ? t("dispatch.review.itemMeta", {
                            time: timeLabel(entry.timestamp, i18n.language),
                            length: length(summary.duration_ms),
                          })
                        : timeLabel(entry.timestamp, i18n.language)}
                    </span>
                    {summary && (
                      <span
                        className="flex h-4 items-center gap-px"
                        aria-hidden="true"
                      >
                        {summary.spark.map((v, i) => (
                          <span
                            key={i}
                            className={`w-0.5 rounded-sm ${selected ? "bg-logo-primary" : "bg-ink-3/50"}`}
                            style={{ height: `${Math.max(12, v * 100)}%` }}
                          />
                        ))}
                      </span>
                    )}
                  </div>
                  <div className="line-clamp-2 text-sm leading-snug text-text">
                    {shownText(entry) || (
                      <span className="italic text-ink-3">
                        {t("settings.history.notTranscribed")}
                      </span>
                    )}
                  </div>
                  {(job ||
                    entry.saved ||
                    entry.post_processed_text ||
                    summary?.checked ||
                    (summary?.fix_count ?? 0) > 0) && (
                    <div className="flex flex-wrap items-center gap-1.5">
                      {job && (
                        <span className="animate-pulse text-xs italic text-ink-2">
                          {t("settings.history.transcribing")}
                        </span>
                      )}
                      {entry.saved && (
                        <Star
                          className="h-3.5 w-3.5 text-logo-primary"
                          fill="currentColor"
                          aria-label={t("settings.history.unsave")}
                        />
                      )}
                      {entry.post_processed_text && (
                        <span className="rounded-md border border-line px-1.5 py-px text-[11px] text-ink-2">
                          {t("dispatch.review.chips.cleaned")}
                        </span>
                      )}
                      {(summary?.fix_count ?? 0) > 0 && (
                        <span className="rounded-md border border-logo-primary/35 bg-logo-primary/12 px-1.5 py-px text-[11px] text-text">
                          {t("dispatch.review.chips.fixes", {
                            count: summary?.fix_count ?? 0,
                          })}
                        </span>
                      )}
                      {approved && (
                        <span className="flex items-center gap-1 rounded-md border border-[color-mix(in_srgb,var(--color-approved),transparent_60%)] px-1.5 py-px text-[11px] text-[var(--color-approved)]">
                          <CheckCheck className="h-3 w-3" aria-hidden="true" />
                          {t("dispatch.review.chips.checked")}
                        </span>
                      )}
                    </div>
                  )}
                </button>
              );
            })}
          </div>
        ))}
        <div ref={sentinelRef} className="h-1" />
      </div>
    </section>
  );
};

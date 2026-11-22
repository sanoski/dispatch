// Dispatch review window (HANDOFF.md, "Review window"): Dispatch's History
// page. Every dictation with its waveform and its words: click a word to hear
// it, fix what Dispatch misheard. Wide windows show the list and the
// dictation side by side; narrow ones show one at a time.
//
// Keeps everything Handy's History page did (paging, live updates,
// transcribing files, starring, transcribing again, deleting) using the same
// commands and stores.
import React, {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { useTranslation } from "react-i18next";
import {
  commands,
  events,
  type DictationSummary,
  type HistoryEntry,
  type HistoryUpdatePayload,
} from "@/bindings";
import { notify } from "@/stores/noticeStore";
import { useTranscriptionJobsStore } from "@/stores/transcriptionJobsStore";
import {
  pickAndTranscribeFiles,
  useFileReadsStore,
} from "../../settings/history/transcribeFiles";
import { DictationDetail } from "./DictationDetail";
import { DictationList, type ListFilter } from "./DictationList";
import { ReviewEmpty } from "./ReviewEmpty";
import { shownText } from "./reviewUtils";
import { KeepRecordingsQuestion } from "../recordings/KeepRecordingsQuestion";

const PAGE_SIZE = 40;
/** Below this width, the list and the dictation take turns. */
const SIDE_BY_SIDE_MIN_WIDTH = 860;

// Grow the window to fit the review window once per run, not on every visit.
let windowFitted = false;

export const ReviewPage: React.FC = () => {
  const { t } = useTranslation();
  const jobs = useTranscriptionJobsStore((state) => state.jobs);
  const reads = useFileReadsStore((state) => state.reads);
  const rootRef = useRef<HTMLDivElement>(null);
  const sentinelRef = useRef<HTMLDivElement>(null);
  const entriesRef = useRef<HistoryEntry[]>([]);
  const loadingRef = useRef(false);
  const [entries, setEntries] = useState<HistoryEntry[]>([]);
  const [summaries, setSummaries] = useState<Record<string, DictationSummary>>(
    {},
  );
  const [loading, setLoading] = useState(true);
  const [hasMore, setHasMore] = useState(true);
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [filter, setFilter] = useState<ListFilter>("all");
  const [query, setQuery] = useState("");
  // Every dictation in History; the list itself only has the pages loaded.
  const [total, setTotal] = useState<number | null>(null);
  const loadTotal = useCallback(async () => {
    const result = await commands.getDictationCount();
    if (result.status === "ok") setTotal(result.data);
  }, []);
  const [wide, setWide] = useState(true);
  // Narrow layout: showing one dictation instead of the list.
  const [showingDetail, setShowingDetail] = useState(false);

  useEffect(() => {
    entriesRef.current = entries;
  }, [entries]);

  useEffect(() => {
    if (windowFitted) return;
    windowFitted = true;
    void commands.fitReviewWindow();
  }, []);

  useLayoutEffect(() => {
    const root = rootRef.current;
    if (!root) return;
    const observer = new ResizeObserver(([entry]) =>
      setWide(entry.contentRect.width >= SIDE_BY_SIDE_MIN_WIDTH),
    );
    observer.observe(root);
    return () => observer.disconnect();
  }, []);

  const loadSummaries = useCallback(async (list: HistoryEntry[]) => {
    const wanted = list.filter((e) => e.transcription_text !== undefined);
    if (wanted.length === 0) return;
    const result = await commands.getDictationSummaries(
      wanted.map((e) => ({
        file_name: e.file_name,
        heard_text: e.transcription_text,
      })),
    );
    if (result.status === "ok") {
      setSummaries((prev) => {
        const next = { ...prev };
        for (const s of result.data) next[s.file_name] = s;
        return next;
      });
    }
  }, []);

  const loadPage = useCallback(
    async (cursor?: number) => {
      const first = cursor === undefined;
      if (!first && loadingRef.current) return;
      loadingRef.current = true;
      if (first) setLoading(true);
      try {
        const result = await commands.getHistoryEntries(
          cursor ?? null,
          PAGE_SIZE,
        );
        if (result.status === "ok") {
          const { entries: page, has_more } = result.data;
          setEntries((prev) => {
            if (first) return page;
            const known = new Set(prev.map((e) => e.id));
            return [...prev, ...page.filter((e) => !known.has(e.id))];
          });
          setHasMore(has_more);
          if (first && page.length > 0) {
            setSelectedId((current) => current ?? page[0].id);
          }
          void loadSummaries(page);
        }
      } catch (error) {
        console.error("Failed to load history entries:", error);
      } finally {
        setLoading(false);
        loadingRef.current = false;
      }
    },
    [loadSummaries],
  );

  useEffect(() => {
    void loadPage();
    void loadTotal();
  }, [loadPage, loadTotal]);

  // Load more as the list's end scrolls into view.
  useEffect(() => {
    const sentinel = sentinelRef.current;
    if (loading || !hasMore || !sentinel) return;
    const observer = new IntersectionObserver(([hit]) => {
      if (!hit.isIntersecting) return;
      const last = entriesRef.current[entriesRef.current.length - 1];
      if (last) void loadPage(last.id);
    });
    observer.observe(sentinel);
    return () => observer.disconnect();
  }, [loading, hasMore, loadPage, entries.length, wide, showingDetail]);

  // New and changed dictations, live.
  useEffect(() => {
    const unlisten = events.historyUpdatePayload.listen((event) => {
      const payload: HistoryUpdatePayload = event.payload;
      if (payload.action === "added" || payload.action === "deleted") {
        void loadTotal();
      }
      if (payload.action === "added") {
        setEntries((prev) => [
          payload.entry,
          ...prev.filter((e) => e.id !== payload.entry.id),
        ]);
        void loadSummaries([payload.entry]);
      } else if (payload.action === "updated") {
        setEntries((prev) =>
          prev.map((e) => (e.id === payload.entry.id ? payload.entry : e)),
        );
        void loadSummaries([payload.entry]);
      }
    });
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, [loadSummaries, loadTotal]);

  const visible = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return entries.filter((entry) => {
      if (filter === "cleaned" && !entry.post_processed_text?.trim())
        return false;
      if (filter === "fixed" && !(summaries[entry.file_name]?.fix_count ?? 0))
        return false;
      if (filter === "checked" && !summaries[entry.file_name]?.checked)
        return false;
      if (filter === "starred" && !entry.saved) return false;
      if (!needle) return true;
      return (
        shownText(entry).toLowerCase().includes(needle) ||
        entry.transcription_text.toLowerCase().includes(needle)
      );
    });
  }, [entries, filter, query, summaries]);

  const selected = entries.find((e) => e.id === selectedId) ?? null;

  const select = (id: number) => {
    setSelectedId(id);
    setShowingDetail(true);
  };

  const toggleSaved = async (id: number) => {
    const flip = () =>
      setEntries((prev) =>
        prev.map((e) => (e.id === id ? { ...e, saved: !e.saved } : e)),
      );
    flip();
    try {
      const result = await commands.toggleHistoryEntrySaved(id);
      if (result.status !== "ok") flip();
    } catch {
      flip();
    }
  };

  const deleteEntry = async (id: number) => {
    const index = visible.findIndex((e) => e.id === id);
    const neighbour = visible[index + 1] ?? visible[index - 1] ?? null;
    setEntries((prev) => prev.filter((e) => e.id !== id));
    setSelectedId(neighbour?.id ?? null);
    if (!neighbour) setShowingDetail(false);
    const result = await commands.deleteHistoryEntry(id);
    if (result.status !== "ok") {
      void loadPage();
      throw new Error(String(result.error));
    }
  };

  const onFixesChanged = useCallback(
    (fileName: string, fixCount: number, checked: boolean) => {
      setSummaries((prev) =>
        prev[fileName]
          ? {
              ...prev,
              [fileName]: { ...prev[fileName], fix_count: fixCount, checked },
            }
          : prev,
      );
    },
    [],
  );

  const transcribeFile = async () => {
    try {
      await pickAndTranscribeFiles();
    } catch (error) {
      console.error("Failed to pick a file to transcribe:", error);
      notify({ title: t("settings.history.transcribeFailed") });
    }
  };

  const openFolder = async () => {
    const result = await commands.openRecordingsFolder();
    if (result.status !== "ok")
      console.error("Failed to open recordings folder:", result.error);
  };

  if (!loading && entries.length === 0 && reads.length === 0) {
    return (
      <div ref={rootRef} className="h-full w-full">
        <ReviewEmpty />
      </div>
    );
  }

  const list = (
    <DictationList
      entries={visible}
      total={total}
      summaries={summaries}
      jobs={jobs}
      reads={reads}
      selectedId={selectedId}
      onSelect={select}
      filter={filter}
      onFilter={setFilter}
      query={query}
      onQuery={setQuery}
      loading={loading}
      sentinelRef={sentinelRef}
      onTranscribeFile={transcribeFile}
      onOpenFolder={openFolder}
    />
  );

  const detail = selected ? (
    <DictationDetail
      key={selected.id}
      entry={selected}
      job={jobs[selected.id]}
      onBack={wide ? undefined : () => setShowingDetail(false)}
      onToggleSaved={() => void toggleSaved(selected.id)}
      onDelete={() => deleteEntry(selected.id)}
      onFixesChanged={onFixesChanged}
    />
  ) : (
    <p className="m-auto text-sm text-ink-3">{t("dispatch.review.pickOne")}</p>
  );

  return (
    <div className="flex h-full min-h-0 w-full flex-col">
      <KeepRecordingsQuestion />
      <div
        ref={rootRef}
        className="flex min-h-0 w-full flex-1 overflow-hidden rounded-xl border border-line"
      >
        {wide ? (
          <>
            <div className="w-79 shrink-0 border-r border-line bg-[var(--color-chrome)]">
              {list}
            </div>
            <main className="flex min-h-0 min-w-0 flex-1 flex-col overflow-y-auto px-7 py-5.5">
              {detail}
            </main>
          </>
        ) : showingDetail && selected ? (
          <main className="flex min-h-0 min-w-0 flex-1 flex-col overflow-y-auto px-5 py-5">
            {detail}
          </main>
        ) : (
          <div className="min-w-0 flex-1 bg-[var(--color-chrome)]">{list}</div>
        )}
      </div>
    </div>
  );
};

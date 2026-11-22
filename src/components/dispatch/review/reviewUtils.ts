// Dispatch review window: small formatting helpers.
import type { TFunction } from "i18next";
import type { HistoryEntry } from "@/bindings";

/** 7600 -> "0:07.6"; with `hundredths`, "0:07.60". */
export const clock = (ms: number, hundredths = false): string => {
  const total = Math.max(0, ms) / 1000;
  const minutes = Math.floor(total / 60);
  const seconds = total - minutes * 60;
  const digits = hundredths ? 2 : 1;
  const padded = seconds.toFixed(digits).padStart(digits + 3, "0");
  return `${minutes}:${padded}`;
};

/** 7600 -> "7.6 s", 84000 -> "1:24". */
export const length = (ms: number): string => {
  const seconds = ms / 1000;
  if (seconds < 60) return `${seconds.toFixed(1)} s`;
  const minutes = Math.floor(seconds / 60);
  const rest = Math.round(seconds - minutes * 60);
  return `${minutes}:${String(rest).padStart(2, "0")}`;
};

const startOfDay = (d: Date) =>
  new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();

/** "Today", "Yesterday", or a date like "Nov 19". */
export const dayLabel = (
  timestampSecs: number,
  t: TFunction,
  locale: string,
): string => {
  const date = new Date(timestampSecs * 1000);
  const days = Math.round(
    (startOfDay(new Date()) - startOfDay(date)) / 86_400_000,
  );
  if (days === 0) return t("dispatch.review.today");
  if (days === 1) return t("dispatch.review.yesterday");
  return date.toLocaleDateString(locale, {
    month: "short",
    day: "numeric",
    year: days > 300 ? "numeric" : undefined,
  });
};

/** "4:12 PM" in the user's locale. */
export const timeLabel = (timestampSecs: number, locale: string): string =>
  new Date(timestampSecs * 1000).toLocaleTimeString(locale, {
    hour: "numeric",
    minute: "2-digit",
  });

/** Entries grouped by day, newest first, in the order given. */
export const groupByDay = (
  entries: HistoryEntry[],
  t: TFunction,
  locale: string,
): { label: string; entries: HistoryEntry[] }[] => {
  const groups: { label: string; entries: HistoryEntry[] }[] = [];
  for (const entry of entries) {
    const label = dayLabel(entry.timestamp, t, locale);
    const last = groups[groups.length - 1];
    if (last && last.label === label) last.entries.push(entry);
    else groups.push({ label, entries: [entry] });
  }
  return groups;
};

const KEY_NAMES: Record<string, string> = {
  alt: "Alt",
  option: "Option",
  ctrl: "Ctrl",
  control: "Ctrl",
  shift: "Shift",
  super: "Win",
  meta: "Win",
  cmd: "Cmd",
  command: "Cmd",
  space: "Space",
  escape: "Esc",
};

/** "alt_left+super_left" -> ["Alt", "Win"]. */
export const hotkeyKeys = (binding: string | undefined): string[] =>
  (binding ?? "")
    .split("+")
    .filter(Boolean)
    .map((key) => {
      const base = key.toLowerCase().replace(/_(left|right)$/, "");
      return KEY_NAMES[base] ?? base.charAt(0).toUpperCase() + base.slice(1);
    });

/** The text a list item shows: the cleaned text when there is one. */
export const shownText = (entry: HistoryEntry): string =>
  entry.post_processed_text?.trim() || entry.transcription_text;

/** Shrinks or stretches `values` to `count` bars, keeping each slice's peak. */
export const resample = (values: number[], count: number): number[] => {
  if (count <= 0 || values.length === 0) return [];
  return Array.from({ length: count }, (_, i) => {
    const from = Math.floor((i * values.length) / count);
    const to = Math.max(
      from + 1,
      Math.floor(((i + 1) * values.length) / count),
    );
    let peak = 0;
    for (let j = from; j < to && j < values.length; j++) {
      peak = Math.max(peak, values[j]);
    }
    return peak;
  });
};

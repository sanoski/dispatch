// Dispatch review window: the recording's waveform, with the selected words
// highlighted, a playhead, a strip marking every word, and time ticks.
import React, { useLayoutEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { ReviewWord } from "@/bindings";
import { clock, resample } from "./reviewUtils";

const BAR_WIDTH = 3;
const BAR_GAP = 2;

interface WaveformProps {
  waveform: number[];
  durationMs: number;
  positionMs: number;
  words: ReviewWord[];
  /** The selected words' time range, highlighted. */
  selection: { startMs: number; endMs: number } | null;
  onSeek: (ms: number) => void;
}

export const Waveform: React.FC<WaveformProps> = ({
  waveform,
  durationMs,
  positionMs,
  words,
  selection,
  onSeek,
}) => {
  const { t } = useTranslation();
  const boxRef = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(0);

  useLayoutEffect(() => {
    const box = boxRef.current;
    if (!box) return;
    const observer = new ResizeObserver(([entry]) =>
      setWidth(entry.contentRect.width),
    );
    observer.observe(box);
    return () => observer.disconnect();
  }, []);

  const bars = useMemo(
    () =>
      resample(waveform, Math.floor((width + BAR_GAP) / (BAR_WIDTH + BAR_GAP))),
    [waveform, width],
  );
  const duration = Math.max(durationMs, 1);
  const pct = (ms: number) =>
    `${Math.min(100, Math.max(0, (ms / duration) * 100))}%`;

  const ticks = useMemo(() => {
    const seconds = duration / 1000;
    const step =
      [0.5, 1, 2, 5, 10, 15, 30, 60].find((s) => seconds / s <= 8) ?? 120;
    const out: number[] = [];
    for (let s = 0; s <= seconds + 1e-6; s += step) out.push(s * 1000);
    return out;
  }, [duration]);

  const seekTo = (event: React.MouseEvent<HTMLDivElement>) => {
    const box = event.currentTarget.getBoundingClientRect();
    onSeek(((event.clientX - box.left) / box.width) * duration);
  };

  const seekByKey = (event: React.KeyboardEvent<HTMLDivElement>) => {
    const step = event.shiftKey ? 5000 : 1000;
    if (event.key === "ArrowRight")
      onSeek(Math.min(duration, positionMs + step));
    else if (event.key === "ArrowLeft") onSeek(Math.max(0, positionMs - step));
    else return;
    event.preventDefault();
  };

  return (
    <div className="flex flex-col gap-2">
      <div
        ref={boxRef}
        role="slider"
        tabIndex={0}
        aria-label={t("dispatch.review.player.position")}
        aria-valuemin={0}
        aria-valuemax={Math.round(duration / 1000)}
        aria-valuenow={Math.round(positionMs / 1000)}
        aria-valuetext={clock(positionMs)}
        onClick={seekTo}
        onKeyDown={seekByKey}
        className="relative h-28 cursor-pointer rounded-md focus:outline-none focus-visible:ring-2 focus-visible:ring-logo-primary"
      >
        {selection && (
          <div
            className="absolute inset-y-0 rounded-md bg-logo-primary/12"
            style={{
              left: pct(selection.startMs),
              width: `calc(${pct(selection.endMs)} - ${pct(selection.startMs)})`,
            }}
          />
        )}
        <div
          className="absolute inset-0 flex items-center"
          style={{ gap: BAR_GAP }}
        >
          {bars.map((value, i) => {
            const ms = ((i + 0.5) / bars.length) * duration;
            const inSelection =
              selection && ms >= selection.startMs && ms <= selection.endMs;
            const played = ms <= positionMs;
            return (
              <span
                key={i}
                className={`shrink-0 rounded-sm ${
                  inSelection
                    ? "bg-logo-primary"
                    : played
                      ? "bg-logo-primary/55"
                      : "bg-line-strong"
                }`}
                style={{
                  width: BAR_WIDTH,
                  height: `${Math.max(4, value * 100)}%`,
                }}
              />
            );
          })}
        </div>
        <div
          className="pointer-events-none absolute inset-y-0 w-0.5 bg-text"
          style={{ left: pct(positionMs) }}
        >
          <span className="absolute -left-1 -top-1.5 h-2.5 w-2.5 rounded-full bg-text" />
        </div>
      </div>

      <div className="relative h-1.5" aria-hidden="true">
        {words.map((word, i) => (
          <span
            key={i}
            className={`absolute inset-y-0 rounded-full ${
              word.precision === "estimated" ? "bg-line-strong" : "bg-ink-3/60"
            }`}
            style={{
              left: pct(word.start_ms),
              width: `max(2px, calc(${pct(word.end_ms)} - ${pct(word.start_ms)} - 2px))`,
            }}
          />
        ))}
      </div>
      <div
        className="relative h-4 text-[11px] tabular-nums text-ink-3"
        aria-hidden="true"
      >
        {ticks.map((ms) => (
          <span
            key={ms}
            className="absolute -translate-x-1/2 first:translate-x-0"
            style={{ left: pct(ms) }}
          >
            {clock(ms).replace(/\.0$/, "")}
          </span>
        ))}
      </div>
    </div>
  );
};

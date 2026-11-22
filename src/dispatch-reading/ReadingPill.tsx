// Dispatch: the read-aloud pill. "Warming up George" while Kokoro loads,
// then "Reading aloud" with a timer and a Stop button, or "Select some text
// first" while George answers an empty selection. Rust shows and hides the
// window; this follows its state.
import React, { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { commands, type PillState } from "@/bindings";
import "./ReadingPill.css";

/** Rust's event (dispatch_reading_pill.rs, EVENT). */
const PILL_EVENT = "dispatch-reading-pill";
const BAR_DELAYS = [0, -0.3, -0.6, -0.15, -0.45];

const clock = (ms: number) => {
  const seconds = Math.max(0, Math.floor(ms / 1000));
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
};

export const ReadingPill: React.FC = () => {
  const { t } = useTranslation();
  const [pill, setPill] = useState<PillState>({ state: "hidden" });
  const [startedAt, setStartedAt] = useState(0);
  const [now, setNow] = useState(() => Date.now());
  const shown = useRef<PillState["state"]>("hidden");

  useEffect(() => {
    const apply = (next: PillState) => {
      if (next.state === "reading" && shown.current !== "reading") {
        const time = Date.now();
        setStartedAt(time);
        setNow(time);
      }
      shown.current = next.state;
      setPill(next);
    };
    commands.getReadingPillState().then(apply);
    const unlisten = listen<PillState>(PILL_EVENT, (event) =>
      apply(event.payload),
    );
    return () => {
      unlisten.then((stop) => stop());
    };
  }, []);

  useEffect(() => {
    if (pill.state !== "reading") return;
    const timer = window.setInterval(() => setNow(Date.now()), 250);
    return () => window.clearInterval(timer);
  }, [pill.state]);

  if (pill.state === "hidden") return null;

  return (
    <div className="dr-stage">
      <div
        key={pill.state}
        className={`dr-pill ${pill.state === "reading" || pill.state === "needs_voice" ? "dr-pill--reading" : ""}`}
        role="status"
        aria-live="polite"
      >
        {pill.state === "warming" && (
          <>
            <span className="dr-dots" aria-hidden="true">
              <span />
              <span />
              <span />
            </span>
            <span className="dr-label">
              {t("dispatch.readingPill.warming", { voice: pill.voice })}
            </span>
          </>
        )}
        {pill.state === "reading" && (
          <>
            <span className="dr-bars" aria-hidden="true">
              {BAR_DELAYS.map((delay, i) => (
                <span key={i} style={{ animationDelay: `${delay}s` }} />
              ))}
            </span>
            <span className="dr-label">
              {t("dispatch.readingPill.reading")}
            </span>
            <span className="dr-meta">
              {t("dispatch.readingPill.voiceTime", {
                voice: pill.voice,
                time: clock(now - startedAt),
              })}
            </span>
            <button
              type="button"
              className="dr-stop"
              aria-label={t("dispatch.readingPill.stopLabel")}
              onClick={() => commands.stopReadAloud()}
            >
              <svg
                width="10"
                height="10"
                viewBox="0 0 10 10"
                aria-hidden="true"
              >
                <rect width="10" height="10" rx="2" fill="currentColor" />
              </svg>
              {t("dispatch.readingPill.stop")}
            </button>
          </>
        )}
        {pill.state === "nothing_selected" && (
          <>
            <svg
              width="16"
              height="16"
              viewBox="0 0 24 24"
              fill="none"
              stroke="var(--color-warning)"
              strokeWidth="2"
              strokeLinecap="round"
              aria-hidden="true"
            >
              <circle cx="12" cy="12" r="9" />
              <path d="M12 8v5M12 16h.01" />
            </svg>
            <span className="dr-label">
              {t("dispatch.readingPill.nothingSelected")}
            </span>
          </>
        )}
        {pill.state === "needs_voice" && (
          <>
            <svg
              width="16"
              height="16"
              viewBox="0 0 24 24"
              fill="none"
              stroke="var(--color-logo-primary)"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
              aria-hidden="true"
            >
              <path d="M12 4v11M7.5 10.5 12 15l4.5-4.5M5 19h14" />
            </svg>
            <span className="dr-label">
              {t("dispatch.readingPill.needsVoice")}
            </span>
            <button
              type="button"
              className="dr-stop"
              onClick={() => commands.openReadAloudPage()}
            >
              {t("dispatch.readingPill.setUp")}
            </button>
          </>
        )}
      </div>
    </div>
  );
};

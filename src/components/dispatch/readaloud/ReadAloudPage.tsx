// Dispatch: the Read Aloud page (HANDOFF.md, "Read aloud", stage 2). Paste
// text, press Play, and George reads it. Below the box: the hotkey, voice,
// speed and the read-back switch. Play turns into Stop while anything is
// being read, from here or from the hotkey, by following the pill's state.
// The loudspeaker art is ARTWORK.md's "Read Aloud" piece.
import React, { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { Play, Square, X } from "lucide-react";
import { commands, type PillState } from "@/bindings";
import { useSettings } from "@/hooks/useSettings";
import { ReadAloudSettings } from "@/components/settings/ReadAloudSettings";
import { hotkeyKeys } from "../review/reviewUtils";
import { VoiceSetup, useVoiceFiles } from "./VoiceSetup";
import art from "@/assets/dispatch/art/read-aloud.webp";

/** Rust's pill event (dispatch_reading_pill.rs, EVENT). */
const PILL_EVENT = "dispatch-reading-pill";
/** Matches MAX_PASTED_CHARS in dispatch_tts.rs. */
const MAX_CHARS = 100_000;
/** George's pace at 1.0x, roughly. */
const WORDS_PER_MINUTE = 165;

// Kept while switching pages, cleared when Dispatch restarts.
let draft = "";

export const ReadAloudPage: React.FC = () => {
  const { t } = useTranslation();
  const { getSetting } = useSettings();
  const [text, setText] = useState(draft);
  const [pill, setPill] = useState<PillState["state"]>("hidden");
  const [error, setError] = useState<string | null>(null);
  const box = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    commands.getReadingPillState().then((state) => setPill(state.state));
    const unlisten = listen<PillState>(PILL_EVENT, (event) =>
      setPill(event.payload.state),
    );
    return () => {
      unlisten.then((stop) => stop());
    };
  }, []);

  const update = (value: string) => {
    draft = value;
    setText(value);
    setError(null);
  };

  const reading = pill === "warming" || pill === "reading";
  const voice = useVoiceFiles();
  const canPlay = !voice || voice.installed || voice.using_dev_copy;
  const speed = getSetting("dispatch_read_aloud_speed") ?? 1;
  const words = text.split(/\s+/).filter((w) => /\w/.test(w)).length;
  const minutes = words / (WORDS_PER_MINUTE * speed);
  const keys = hotkeyKeys(
    getSetting("bindings")?.dispatch_read_aloud?.current_binding,
  );

  const play = async () => {
    if (reading) {
      await commands.stopReadAloud();
      return;
    }
    if (!text.trim()) {
      box.current?.focus();
      return;
    }
    const result = await commands.readTextAloud(text);
    if (result.status === "error") setError(result.error);
  };

  return (
    <div className="mx-auto flex w-full max-w-3xl flex-col gap-6">
      <header className="flex items-center gap-5 px-1">
        <img
          src={art}
          alt=""
          width={88}
          height={88}
          draggable={false}
          className="h-[88px] w-[88px] shrink-0 rounded-[20px] border border-line object-cover shadow-[0_16px_32px_-22px_color-mix(in_srgb,var(--color-logo-primary),transparent_30%)]"
        />
        <div className="flex min-w-0 flex-col gap-1.5">
          <h1 className="m-0 font-display text-[28px] font-bold tracking-[-0.4px]">
            {t("dispatch.readAloud.page.title")}
          </h1>
          <p className="m-0 flex flex-wrap items-center gap-1.5 text-[15px] text-ink-2">
            {t("dispatch.readAloud.page.subtitle")}
            {keys.length > 0 && (
              <>
                <span>{t("dispatch.readAloud.page.orHotkey")}</span>
                {keys.map((key) => (
                  <kbd
                    key={key}
                    className="rounded-md border border-line-strong bg-[var(--color-card)] px-1.5 py-0.5 font-sans text-xs font-semibold text-text"
                  >
                    {key}
                  </kbd>
                ))}
              </>
            )}
          </p>
        </div>
      </header>

      {voice && <VoiceSetup status={voice} />}

      <section className="flex flex-col overflow-hidden rounded-2xl border border-line bg-[var(--color-card)] focus-within:border-logo-primary/60">
        <label htmlFor="dispatch-read-aloud-text" className="sr-only">
          {t("dispatch.readAloud.page.boxLabel")}
        </label>
        <textarea
          id="dispatch-read-aloud-text"
          ref={box}
          value={text}
          maxLength={MAX_CHARS}
          onChange={(event) => update(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
              event.preventDefault();
              play();
            }
          }}
          placeholder={t("dispatch.readAloud.page.placeholder")}
          spellCheck
          className="min-h-[260px] w-full resize-y select-text bg-transparent px-5 py-4 text-[15px] leading-relaxed text-text placeholder:text-ink-3 focus:outline-none"
        />
        <div className="flex flex-wrap items-center justify-between gap-3 border-t border-line px-4 py-3">
          <span className="text-[13px] tabular-nums text-ink-3">
            {error ??
              (words === 0
                ? t("dispatch.readAloud.page.shortcutHint")
                : t("dispatch.readAloud.page.length", {
                    words: t("dispatch.readAloud.page.words", {
                      count: words,
                    }),
                    time:
                      minutes < 1
                        ? t("dispatch.readAloud.page.underAMinute")
                        : t("dispatch.readAloud.page.minutes", {
                            count: Math.round(minutes),
                          }),
                  }))}
          </span>
          <div className="flex items-center gap-2">
            {text.length > 0 && (
              <button
                type="button"
                onClick={() => {
                  update("");
                  box.current?.focus();
                }}
                className="flex h-9 cursor-pointer items-center gap-1.5 rounded-full px-3 text-[13px] font-semibold text-ink-2 hover:bg-line hover:text-text focus:outline-none focus-visible:ring-2 focus-visible:ring-logo-primary"
              >
                <X size={14} aria-hidden="true" />
                {t("dispatch.readAloud.page.clear")}
              </button>
            )}
            <button
              type="button"
              onClick={play}
              disabled={!reading && !canPlay}
              title={
                !reading && !canPlay
                  ? t("dispatch.readAloud.page.needsVoice")
                  : undefined
              }
              aria-pressed={reading}
              className={`flex h-9 cursor-pointer items-center gap-2 rounded-full px-4 text-[13px] font-semibold transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-logo-primary focus-visible:ring-offset-2 focus-visible:ring-offset-[var(--color-card)] ${
                reading
                  ? "border border-line-strong bg-background text-text hover:border-logo-primary"
                  : "bg-logo-primary text-[var(--color-on-accent)] hover:brightness-110 disabled:cursor-not-allowed disabled:opacity-50"
              }`}
            >
              {reading ? (
                <Square size={12} fill="currentColor" aria-hidden="true" />
              ) : (
                <Play size={14} fill="currentColor" aria-hidden="true" />
              )}
              {reading
                ? pill === "warming"
                  ? t("dispatch.readAloud.page.warming")
                  : t("dispatch.readAloud.page.stop")
                : t("dispatch.readAloud.page.play")}
            </button>
          </div>
        </div>
      </section>

      <ReadAloudSettings />
    </div>
  );
};

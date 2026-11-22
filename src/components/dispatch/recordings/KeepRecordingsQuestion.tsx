// Dispatch: asked once, at the top of History (where recordings live), so
// the choice to keep every recording is the user's, not a silent default.
// The answer can be changed any time in Settings > Advanced.
import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { AudioLines } from "lucide-react";
import { KeepRecordingsDialog } from "./KeepRecordingsDialog";
import { NEWEST_KEPT, useKeepRecordings } from "./useKeepRecordings";

const ASKED_KEY = "dispatch.recordings.asked";

const wasAsked = () => {
  try {
    return localStorage.getItem(ASKED_KEY) === "1";
  } catch {
    return false;
  }
};

const markAsked = () => {
  try {
    localStorage.setItem(ASKED_KEY, "1");
  } catch {
    // Asked again next time; harmless.
  }
};

export const KeepRecordingsQuestion: React.FC = () => {
  const { t } = useTranslation();
  const keep = useKeepRecordings();
  const [asked, setAsked] = useState(wasAsked);
  if (asked) return null;

  const answer = async (keepAll: boolean) => {
    // "Newest" with old recordings opens the dialog; its answer ends it.
    if ((await keep.choose(keepAll)) === "done") done();
  };
  const done = () => {
    markAsked();
    setAsked(true);
  };

  return (
    <section
      aria-labelledby="dispatch-keep-recordings-title"
      className="dispatch-pop-in mb-3 flex flex-wrap items-center gap-x-5 gap-y-3 rounded-xl border border-logo-primary/35 bg-[var(--color-card)] px-5 py-4"
    >
      <span className="grid h-10 w-10 shrink-0 place-items-center rounded-full bg-logo-primary/15 text-logo-primary">
        <AudioLines className="h-5 w-5" aria-hidden="true" />
      </span>
      <div className="flex min-w-60 flex-1 flex-col gap-1">
        <h2
          id="dispatch-keep-recordings-title"
          className="m-0 font-display text-base font-bold text-text"
        >
          {t("dispatch.recordings.question.title")}
        </h2>
        <p className="m-0 text-[13px] leading-snug text-ink-2">
          {t("dispatch.recordings.question.body")}
        </p>
        <p className="m-0 text-xs text-ink-3">
          {t("dispatch.recordings.question.later")}
        </p>
      </div>
      <div className="flex flex-wrap items-center gap-2">
        <button
          type="button"
          disabled={keep.busy}
          onClick={() => void answer(false)}
          className="cursor-pointer rounded-full border border-line-strong px-4 py-2 text-[13px] font-semibold text-text hover:border-logo-primary disabled:cursor-not-allowed disabled:opacity-50 focus:outline-none focus-visible:ring-2 focus-visible:ring-logo-primary"
        >
          {t("dispatch.recordings.question.newest", { count: NEWEST_KEPT })}
        </button>
        <button
          type="button"
          disabled={keep.busy}
          onClick={() => void answer(true)}
          className="cursor-pointer rounded-full bg-logo-primary px-4 py-2 text-[13px] font-semibold text-[var(--color-on-accent)] hover:brightness-110 disabled:cursor-not-allowed disabled:opacity-50 focus:outline-none focus-visible:ring-2 focus-visible:ring-logo-primary focus-visible:ring-offset-2"
        >
          {t("dispatch.recordings.question.keepAll")}
        </button>
      </div>
      <KeepRecordingsDialog
        pending={keep.pending}
        busy={keep.busy}
        onConfirm={async () => {
          await keep.confirm();
          done();
        }}
        onCancel={keep.cancel}
      />
    </section>
  );
};

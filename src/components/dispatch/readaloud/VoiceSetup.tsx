// Dispatch: downloading George's voice (dispatch_voice_files.rs). Shown at
// the top of the Read Aloud page until the files are in Dispatch's data
// folder. A dev build reading the repo's copy says so, and can still
// download, to try it out.
import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { Download } from "lucide-react";
import { commands, type VoiceFilesStatus } from "@/bindings";

/** Rust's event (dispatch_voice_files.rs, EVENT). */
const VOICE_FILES_EVENT = "dispatch-voice-files";

const megabytes = (bytes: number) => Math.round(bytes / 1_000_000);

/** The voice files' status, kept fresh while a download runs. */
export const useVoiceFiles = () => {
  const [status, setStatus] = useState<VoiceFilesStatus | null>(null);
  useEffect(() => {
    commands.getVoiceFilesStatus().then(setStatus);
    const unlisten = listen<VoiceFilesStatus>(VOICE_FILES_EVENT, (event) =>
      setStatus(event.payload),
    );
    return () => {
      unlisten.then((stop) => stop());
    };
  }, []);
  return status;
};

export const VoiceSetup: React.FC<{ status: VoiceFilesStatus }> = ({
  status,
}) => {
  const { t } = useTranslation();
  if (status.installed) return null;

  const total = megabytes(status.total_bytes);
  const done = megabytes(status.downloaded_bytes);
  const share =
    status.total_bytes > 0 ? status.downloaded_bytes / status.total_bytes : 0;

  return (
    <section
      aria-labelledby="dispatch-voice-setup"
      className="flex flex-col gap-4 rounded-2xl border border-logo-primary/40 bg-[var(--color-card)] px-5 py-5"
    >
      <div className="flex items-start gap-4">
        <span
          className="grid h-11 w-11 shrink-0 place-items-center rounded-full bg-logo-primary/14 text-logo-primary"
          aria-hidden="true"
        >
          <Download size={20} />
        </span>
        <div className="flex min-w-0 flex-1 flex-col gap-1">
          <h2
            id="dispatch-voice-setup"
            className="m-0 font-display text-lg font-bold"
          >
            {t("dispatch.readAloud.setup.title")}
          </h2>
          <p className="m-0 text-sm leading-normal text-ink-2">
            {t("dispatch.readAloud.setup.explain", { size: total })}
          </p>
          {status.using_dev_copy && !status.downloading && (
            <p className="m-0 text-[13px] text-ink-3">
              {t("dispatch.readAloud.setup.devCopy")}
            </p>
          )}
        </div>
        {!status.downloading && (
          <button
            type="button"
            onClick={() => commands.downloadVoiceFiles()}
            className="flex h-9 shrink-0 cursor-pointer items-center gap-2 rounded-full bg-logo-primary px-4 text-[13px] font-semibold text-[var(--color-on-accent)] hover:brightness-110 focus:outline-none focus-visible:ring-2 focus-visible:ring-logo-primary focus-visible:ring-offset-2 focus-visible:ring-offset-[var(--color-card)]"
          >
            {status.error || status.downloaded_bytes > 0
              ? t("dispatch.readAloud.setup.resume")
              : t("dispatch.readAloud.setup.download", { size: total })}
          </button>
        )}
      </div>

      {(status.downloading || status.downloaded_bytes > 0) && (
        <div className="flex flex-col gap-2">
          <div
            role="progressbar"
            aria-label={t("dispatch.readAloud.setup.progressLabel")}
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={Math.round(share * 100)}
            className="h-2 overflow-hidden rounded-full bg-[var(--color-chrome)]"
          >
            <div
              className="h-full rounded-full bg-logo-primary transition-[width] duration-200"
              style={{ width: `${(share * 100).toFixed(1)}%` }}
            />
          </div>
          <div className="flex items-center justify-between gap-3 text-[13px] tabular-nums text-ink-3">
            <span>
              {t("dispatch.readAloud.setup.progress", {
                done,
                total,
                percent: Math.floor(share * 100),
              })}
            </span>
            {status.downloading && (
              <button
                type="button"
                onClick={() => commands.cancelVoiceFilesDownload()}
                className="cursor-pointer rounded-full px-3 py-1 font-semibold text-ink-2 hover:bg-line hover:text-text focus:outline-none focus-visible:ring-2 focus-visible:ring-logo-primary"
              >
                {t("dispatch.readAloud.setup.cancel")}
              </button>
            )}
          </div>
        </div>
      )}

      {status.error && !status.downloading && (
        <p role="alert" className="m-0 text-[13px] text-error">
          {t("dispatch.readAloud.setup.failed", { error: status.error })}
        </p>
      )}
    </section>
  );
};

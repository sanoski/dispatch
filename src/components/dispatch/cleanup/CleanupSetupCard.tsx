// Dispatch: the cleanup setup card (src-tauri/src/dispatch_cleanup_setup.rs).
// On a new install without Ollama or its cleanup model, cleanup stays off
// and this card, under the Clean Up Dictation switch, says how to set it
// up. "Check again" asks Ollama now; once it's found, cleanup switches back
// on and the card goes away.
import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Check, Copy, ExternalLink, RefreshCcw, Sparkles } from "lucide-react";
import { commands, type CleanupSetup } from "@/bindings";
import { useSettingsStore } from "@/stores/settingsStore";

/** Rust's event (dispatch_cleanup_setup.rs, EVENT). */
const SETUP_EVENT = "dispatch-cleanup-setup";
const OLLAMA_URL = "https://ollama.com/download";

export const CleanupSetupCard: React.FC = () => {
  const { t } = useTranslation();
  const refreshSettings = useSettingsStore((state) => state.refreshSettings);
  const [setup, setSetup] = useState<CleanupSetup | null>(null);
  const [checking, setChecking] = useState(false);
  const [stillMissing, setStillMissing] = useState(false);
  const [fixed, setFixed] = useState(false);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    commands.getCleanupSetup().then(setSetup);
    const unlisten = listen<CleanupSetup>(SETUP_EVENT, (event) => {
      setSetup(event.payload);
      void refreshSettings();
    });
    return () => {
      unlisten.then((stop) => stop());
    };
  }, [refreshSettings]);

  if (fixed) {
    return (
      <p
        role="status"
        className="m-0 flex items-center gap-2 px-4 py-3 text-sm text-[var(--color-approved)]"
      >
        <Check size={16} aria-hidden="true" />
        {t("dispatch.cleanupSetup.ready")}
      </p>
    );
  }
  if (!setup?.show) return null;

  const command = `ollama pull ${setup.model}`;
  const noOllama = setup.problem === "no_ollama";

  const checkAgain = async () => {
    setChecking(true);
    setStillMissing(false);
    try {
      const result = await commands.checkCleanupSetup();
      setSetup(result);
      await refreshSettings();
      if (result.problem === null) setFixed(true);
      else setStillMissing(true);
    } finally {
      setChecking(false);
    }
  };

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(command);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // The command is on screen to type instead.
    }
  };

  return (
    <section
      aria-labelledby="dispatch-cleanup-setup"
      className="flex flex-col gap-3 px-4 py-4"
    >
      <div className="flex items-start gap-3">
        <span
          className="grid h-9 w-9 shrink-0 place-items-center rounded-full bg-logo-primary/14 text-logo-primary"
          aria-hidden="true"
        >
          <Sparkles size={17} />
        </span>
        <div className="flex min-w-0 flex-col gap-1">
          <h3
            id="dispatch-cleanup-setup"
            className="m-0 text-sm font-semibold text-text"
          >
            {noOllama
              ? t("dispatch.cleanupSetup.noOllamaTitle")
              : t("dispatch.cleanupSetup.noModelTitle")}
          </h3>
          <p className="m-0 text-[13px] leading-normal text-ink-2">
            {noOllama
              ? t("dispatch.cleanupSetup.noOllama")
              : t("dispatch.cleanupSetup.noModel", { model: setup.model })}
          </p>
        </div>
      </div>

      <ol className="m-0 flex list-none flex-col gap-2.5 pl-12">
        {noOllama && (
          <li className="flex flex-wrap items-center gap-2 text-[13px] text-ink-2">
            <span className="font-semibold text-text">
              {t("dispatch.cleanupSetup.stepOne")}
            </span>
            {t("dispatch.cleanupSetup.install")}
            <button
              type="button"
              onClick={() => openUrl(OLLAMA_URL).catch(console.error)}
              className="flex h-7 cursor-pointer items-center gap-1.5 rounded-full border border-line-strong px-3 text-xs font-semibold text-text hover:border-logo-primary focus:outline-none focus-visible:ring-2 focus-visible:ring-logo-primary"
            >
              {t("dispatch.cleanupSetup.getOllama")}
              <ExternalLink size={12} aria-hidden="true" />
            </button>
          </li>
        )}
        <li className="flex flex-col gap-1.5 text-[13px] text-ink-2">
          <span>
            {noOllama && (
              <span className="font-semibold text-text">
                {t("dispatch.cleanupSetup.stepTwo")}{" "}
              </span>
            )}
            {t("dispatch.cleanupSetup.pull")}
          </span>
          <div className="flex items-center gap-2">
            <code className="select-text rounded-md border border-line bg-background px-2.5 py-1.5 font-mono text-xs text-text">
              {command}
            </code>
            <button
              type="button"
              onClick={copy}
              aria-label={t("dispatch.cleanupSetup.copy")}
              title={t("dispatch.cleanupSetup.copy")}
              className="grid h-7 w-7 cursor-pointer place-items-center rounded-md text-ink-2 hover:bg-logo-primary/10 hover:text-text focus:outline-none focus-visible:ring-2 focus-visible:ring-logo-primary"
            >
              {copied ? (
                <Check size={14} aria-hidden="true" />
              ) : (
                <Copy size={14} aria-hidden="true" />
              )}
            </button>
          </div>
        </li>
      </ol>

      <div className="flex flex-wrap items-center gap-3 pl-12">
        <button
          type="button"
          onClick={checkAgain}
          disabled={checking}
          className="flex h-8 cursor-pointer items-center gap-1.5 rounded-full bg-logo-primary px-3.5 text-xs font-semibold text-[var(--color-on-accent)] hover:brightness-110 focus:outline-none focus-visible:ring-2 focus-visible:ring-logo-primary focus-visible:ring-offset-2 focus-visible:ring-offset-[var(--color-card)] disabled:cursor-wait disabled:opacity-60"
        >
          <RefreshCcw
            size={13}
            aria-hidden="true"
            className={checking ? "animate-spin" : undefined}
          />
          {checking
            ? t("dispatch.cleanupSetup.checking")
            : t("dispatch.cleanupSetup.checkAgain")}
        </button>
        <span role="status" className="text-xs text-ink-3">
          {stillMissing
            ? t("dispatch.cleanupSetup.stillMissing")
            : t("dispatch.cleanupSetup.autoCheck")}
        </span>
      </div>
    </section>
  );
};

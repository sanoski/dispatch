/**
 * Dispatch's wording, applied on top of Handy's translations at runtime so
 * the 27 locale files stay as upstream ships them (easy merges).
 *
 * 1. Every language: the app name "Handy" reads "Dispatch". Keys under
 *    `dispatch.` are skipped, since those credit Handy by name.
 * 2. English only: `dispatch-overrides.en.json` replaces individual strings
 *    ("Post Process" is "Cleanup") and holds Dispatch-only strings that the
 *    other languages pick up through the English fallback.
 *
 * build.rs reads the same overrides file for the tray's English titles.
 */
import type { i18n as I18n, PostProcessorModule } from "i18next";
import overridesEn from "./dispatch-overrides.en.json";

export const DISPATCH_POST_PROCESSOR = "dispatchRename";

const APP_NAME = /\bHandy\b/g;

export const dispatchPostProcessor: PostProcessorModule = {
  type: "postProcessor",
  name: DISPATCH_POST_PROCESSOR,
  process(value: string, key: string | string[]) {
    const keys = Array.isArray(key) ? key : [key];
    if (keys.some((k) => k.startsWith("dispatch."))) return value;
    return typeof value === "string"
      ? value.replace(APP_NAME, "Dispatch")
      : value;
  },
};

export function applyDispatchOverrides(i18n: I18n) {
  i18n.addResourceBundle("en", "translation", overridesEn, true, true);
}

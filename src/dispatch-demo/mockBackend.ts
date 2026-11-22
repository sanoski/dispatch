// Dispatch demo: the stand-in for the Rust side (see main.tsx). Each Tauri
// command the interface calls gets a demo answer from `HANDLERS`; anything
// else answers null and is listed in `window.__dispatchDemoMissing`, so a
// screenshot script can report what still needs an answer.
import {
  mockConvertFileSrc,
  mockIPC,
  mockWindows,
} from "@tauri-apps/api/mocks";
import demoSettings from "./demoSettings.json";
import {
  DICTATIONS,
  achievements,
  demoWav,
  fileName,
  historyEntry,
  review,
  summary,
} from "./demoData";

type Args = Record<string, unknown> | undefined;
type Handler = (args: Args) => unknown;

declare global {
  interface Window {
    __dispatchDemoMissing: string[];
  }
}

window.__dispatchDemoMissing = [];
window.__TAURI_OS_PLUGIN_INTERNALS__ = {
  platform: "windows",
  os_type: "windows",
  family: "windows",
  arch: "x86_64",
  version: "10.0.26100",
  exe_extension: "exe",
  eol: "\r\n",
};

const ok = (data: unknown) => data;

// The default hotkey, not the one on the computer the settings came from.
const settings = {
  ...demoSettings,
  bindings: {
    ...demoSettings.bindings,
    transcribe: {
      ...demoSettings.bindings.transcribe,
      current_binding: "ctrl+space",
    },
  },
};

const indexOf = (name: unknown) =>
  DICTATIONS.findIndex((_, i) => fileName(i) === name);

const model = (
  id: string,
  name: string,
  description: string,
  size_mb: number,
  downloaded: boolean,
  recommended: boolean,
) => ({
  id,
  name,
  description,
  filename: id.split("/").pop(),
  source: { Url: { url: "", sha256: null } },
  size_mb,
  is_downloaded: downloaded,
  is_downloading: false,
  partial_size: 0,
  accuracy_score: 0.9,
  speed_score: 0.8,
  supports_translation: false,
  is_recommended: recommended,
  supported_languages: ["en"],
  supports_language_selection: false,
  is_custom: false,
  supports_streaming: false,
  supports_language_detection: false,
});

const MODELS = [
  model(
    settings.selected_model,
    "Whisper Large v3 Turbo",
    "Accurate, and fast on a graphics card.",
    874,
    true,
    false,
  ),
  model(
    "parakeet-unified-en-0.6b",
    "Parakeet Unified English",
    "Fast and accurate, even without a graphics card.",
    480,
    true,
    true,
  ),
];

const HANDLERS: Record<string, Handler> = {
  get_app_settings: () => ok(settings),
  get_settings: () => ok(settings),
  get_default_settings: () => ok(settings),
  get_available_models: () => ok(MODELS),
  get_current_model: () => ok(settings.selected_model),
  get_retired_models: () => [],
  get_active_notices: () => [],
  check_custom_sounds: () => ({ start: false, stop: false }),
  is_update_checks_locked: () => false,
  main_window_ready: () => null,
  "plugin:app|version": () => "0.1.0",
  "plugin:app|name": () => "Dispatch",
  get_windows_microphone_permission_status: () => ({
    supported: false,
    overall_access: "allowed",
    device_access: "allowed",
    app_access: "allowed",
    desktop_app_access: "allowed",
  }),
  get_secure_input_status: () => ({
    enabled: false,
    sustained: false,
    culprit_pid: null,
    culprit_name: null,
  }),
  get_cleanup_setup: () => ({
    show: false,
    problem: null,
    model: "qwen3.5:4b",
  }),
  get_microphone_channels: () => [],
  get_available_microphones: () => [
    { index: "default", name: "Default", is_default: true },
  ],
  get_available_output_devices: () => [
    { index: "default", name: "Default", is_default: true },
  ],
  get_transcription_model_status: () => settings.selected_model,
  initialize_enigo: () => null,
  initialize_shortcuts: () => null,
  get_achievements: () => achievements(),
  mark_achievements_seen: () => null,
  get_history_entries: () => ({
    entries: DICTATIONS.map(historyEntry),
    has_more: false,
  }),
  // Everything in History; the list shows the newest few.
  get_dictation_count: () => 412,
  get_dictation_summaries: () => DICTATIONS.map(summary),
  get_dictation_review: (args) => {
    const i = indexOf(args?.fileName);
    return review(DICTATIONS[i], i);
  },
  get_audio_file_path: (args) => `C:/demo/${String(args?.fileName)}`,
  fit_review_window: () => null,
  get_quiet_dictation_audio: (args) => ({
    path: `C:/demo/${String(args?.fileName)}`,
    peak: 0.9,
  }),
  get_reading_pill_state: () => ({ state: "hidden" }),
  get_voice_files_status: () => ({
    installed: true,
    using_dev_copy: false,
    downloading: false,
    downloaded_bytes: 0,
    total_bytes: 0,
    error: null,
  }),
  get_read_aloud_voices: () => [
    "bm_george",
    "bf_emma",
    "am_michael",
    "af_heart",
  ],
};

// The History page asks about keeping recordings once; a demo has answered.
try {
  localStorage.setItem("dispatch.recordings.asked", "1");
} catch {
  // Private window: the question shows, which is harmless.
}

mockWindows("main");
mockConvertFileSrc("windows");

// Recordings: a stand-in WAV per demo dictation, made in the browser.
const audioUrls = new Map<number, string>();
const internals = (
  window as unknown as {
    __TAURI_INTERNALS__: { convertFileSrc: (path: string) => string };
  }
).__TAURI_INTERNALS__;
const assetUrl = internals.convertFileSrc;
internals.convertFileSrc = (path: string) => {
  const i = indexOf(path.split(/[\/]/).pop());
  if (i < 0) return assetUrl(path);
  if (!audioUrls.has(i)) audioUrls.set(i, URL.createObjectURL(demoWav(i)));
  return audioUrls.get(i) as string;
};
mockIPC(
  (cmd, args) => {
    const handler = HANDLERS[cmd];
    if (handler) return handler(args as Args);
    if (!window.__dispatchDemoMissing.includes(cmd)) {
      window.__dispatchDemoMissing.push(cmd);
    }
    return null;
  },
  { shouldMockEvents: true },
);

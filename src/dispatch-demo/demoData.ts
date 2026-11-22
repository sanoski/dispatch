// Dispatch demo: made-up dictations for the README screenshots and the
// video. None of this is anyone's real dictation. Times are relative to
// now, so the list always reads "Today" and "Yesterday".

export interface DemoDictation {
  heard: string;
  cleaned: string | null;
  /** Minutes ago. */
  ago: number;
  starred?: boolean;
  checked?: boolean;
  /** Heard word index -> what it was before the user fixed it. */
  fixes?: Record<number, string>;
}

export const DICTATIONS: DemoDictation[] = [
  {
    heard:
      "Okay so the plan for this afternoon is to finish the quarterly report, um, send it to Dana for review, and then book the train tickets for Thursday.",
    cleaned:
      "Okay, so the plan for this afternoon is to finish the quarterly report, send it to Dana for review, and then book the train tickets for Thursday.",
    ago: 4,
  },
  {
    heard:
      "Can you open up Claude Code and ask it to fix the failing test in the settings page",
    cleaned:
      "Can you open up Claude Code and ask it to fix the failing test in the settings page?",
    ago: 22,
    fixes: { 4: "quad", 5: "code" },
  },
  {
    heard:
      "Remind me to call the hardware store about the snow blower part before five, actually make it before four",
    cleaned:
      "Remind me to call the hardware store about the snow blower part before four.",
    ago: 51,
    checked: true,
  },
  {
    heard:
      "Dear Sam, thanks for sending over the photos from the trip. The one of the old station at dusk is my favorite.",
    cleaned:
      "Dear Sam, thanks for sending over the photos from the trip. The one of the old station at dusk is my favorite.",
    ago: 96,
    starred: true,
    checked: true,
  },
  {
    heard:
      "Note for the meeting, the new schedule starts on the first and everyone gets the updated timetable by Friday",
    cleaned:
      "Note for the meeting: the new schedule starts on the first, and everyone gets the updated timetable by Friday.",
    ago: 180,
    checked: true,
  },
  {
    heard: "Add milk, eggs, coffee and a bag of rock salt to the grocery list",
    cleaned:
      "Add milk, eggs, coffee, and a bag of rock salt to the grocery list.",
    ago: 1500,
  },
  {
    heard:
      "The review window lets me click any word and hear exactly what I said, which makes fixing mistakes a lot faster",
    cleaned:
      "The review window lets me click any word and hear exactly what I said, which makes fixing mistakes a lot faster.",
    ago: 1580,
    checked: true,
  },
  {
    heard:
      "Let's push the release to Monday so we have time to test the installer",
    cleaned:
      "Let's push the release to Monday so we have time to test the installer.",
    ago: 1660,
  },
];

const nowSecs = () => Math.floor(Date.now() / 1000);

/** A repeatable random number from a seed, 0 to 1. */
const noise = (seed: number) => {
  const x = Math.sin(seed * 12.9898) * 43758.5453;
  return x - Math.floor(x);
};

export const fileName = (i: number) => `dispatch-demo-${i + 1}.wav`;

const words = (text: string) => text.split(/\s+/).filter(Boolean);

/** Word times: longer words take longer, with a pause after punctuation. */
const timeWords = (list: string[]) => {
  let t = 350;
  return list.map((word) => {
    const length = 140 + word.replace(/\W/g, "").length * 55;
    const start = t;
    const end = t + length;
    t = end + (/[,.?!:]$/.test(word) ? 320 : 70);
    return { start, end };
  });
};

const durationOf = (d: DemoDictation) => {
  const times = timeWords(words(d.heard));
  return times[times.length - 1].end + 450;
};

/** 320 loudness bars: speech where the words are, quiet room noise between. */
const waveform = (d: DemoDictation, index: number, bars: number) => {
  const duration = durationOf(d);
  const times = timeWords(words(d.heard));
  return Array.from({ length: bars }, (_, b) => {
    const at = ((b + 0.5) / bars) * duration;
    const word = times.findIndex((w) => at >= w.start && at <= w.end);
    if (word < 0) return 0.03 + noise(index * 1000 + b) * 0.03;
    const w = times[word];
    const shape = Math.sin((Math.PI * (at - w.start)) / (w.end - w.start));
    // Louder and quieter words, and a ragged edge, like real speech.
    const loudness = 0.45 + 0.55 * noise(index * 77 + word);
    const jitter = 0.55 + 0.45 * noise(index * 1000 + b * 7);
    return Math.min(1, 0.12 + 0.85 * Math.sqrt(shape) * loudness * jitter);
  });
};

export const historyEntry = (d: DemoDictation, i: number) => ({
  id: DICTATIONS.length - i,
  file_name: fileName(i),
  timestamp: nowSecs() - d.ago * 60,
  saved: Boolean(d.starred),
  title: "",
  transcription_text: d.heard,
  post_processed_text: d.cleaned,
  post_process_prompt: d.cleaned ? "dispatch_cleanup" : null,
  post_process_requested: Boolean(d.cleaned),
  source_name: null,
  language: "en",
  language_source: null,
});

export const summary = (d: DemoDictation, i: number) => ({
  file_name: fileName(i),
  duration_ms: durationOf(d),
  spark: waveform(d, i, 24),
  fix_count: Object.keys(d.fixes ?? {}).length > 0 ? 1 : 0,
  checked: Boolean(d.checked),
});

export const review = (d: DemoDictation, i: number) => {
  const heardWords = words(d.heard);
  const times = timeWords(heardWords);
  const heard = heardWords.map((text, w) => ({
    text,
    start_ms: times[w].start,
    end_ms: times[w].end,
    precision: "aligned",
    heard_first: w,
    heard_last: w,
    fixed_from: d.fixes?.[w] ?? null,
  }));
  // The cleaned words, lined up with the heard ones by their letters.
  const squash = (s: string) => s.toLowerCase().replace(/[^a-z0-9']/g, "");
  let cursor = 0;
  const cleaned = d.cleaned
    ? words(d.cleaned).map((text) => {
        const match = heardWords.findIndex(
          (h, w) => w >= cursor && squash(h) === squash(text),
        );
        if (match >= 0) cursor = match + 1;
        const at = match >= 0 ? match : Math.min(cursor, heard.length - 1);
        return {
          text,
          start_ms: heard[at].start_ms,
          end_ms: heard[at].end_ms,
          precision: match >= 0 ? "aligned" : "estimated",
          heard_first: match >= 0 ? match : null,
          heard_last: match >= 0 ? match : null,
          fixed_from: null,
        };
      })
    : null;
  return {
    duration_ms: durationOf(d),
    waveform: waveform(d, i, 320),
    heard,
    cleaned,
    timing_source: "parakeet-tdt-0.6b-v3.gguf",
    fix_count: Object.keys(d.fixes ?? {}).length > 0 ? 1 : 0,
    peak: 0.9,
    checked: Boolean(d.checked),
    has_audio: true,
  };
};

const ACHIEVEMENTS: [string, string, boolean][] = [
  ["first_words", "milestones", true],
  ["getting_rolling", "milestones", true],
  ["thousand_club", "milestones", true],
  ["wordsmith", "milestones", true],
  ["novelist", "milestones", false],
  ["marathon", "milestones", false],
  ["on_a_roll", "habits", true],
  ["regular", "habits", false],
  ["proofreader", "features", true],
  ["fixer", "features", true],
  ["course_correction", "features", true],
  ["all_aboard", "hidden", true],
  ["pizza_in_the_oven", "hidden", false],
  ["might_could", "hidden", false],
  ["night_shift", "hidden", true],
  ["quad_code_no_more", "hidden", false],
  ["speed_talker", "hidden", false],
  ["pushing_his_buttons", "hidden", false],
  ["bedtime_story", "hidden", false],
  ["tier_1", "voice_data", true],
  ["tier_2", "voice_data", false],
  ["tier_3", "voice_data", false],
  ["tier_4", "voice_data", false],
];

const PROGRESS: Record<string, [number, number]> = {
  novelist: [31_200, 50_000],
  marathon: [41, 60],
  regular: [9, 14],
};

export const achievements = () => ({
  achievements: ACHIEVEMENTS.map(([id, section, unlocked], i) => ({
    id,
    section,
    unlocked_at: unlocked ? nowSecs() - 86_400 * (9 - (i % 9)) : null,
    seen: true,
    have: PROGRESS[id]?.[0] ?? null,
    need: PROGRESS[id]?.[1] ?? null,
  })),
  recorded_ms: 61 * 60_000,
  checked_ms: 47 * 60_000,
  keeps_all_audio: true,
  per_day_ms: 6 * 60_000,
  dictations: 412,
  words: 31_200,
  days_used: 9,
});

/**
 * A stand-in recording, made in the browser: a soft hum where each word is,
 * near-silence between, so the player loads and its timeline matches the
 * words. Not speech; the video uses real voiced takes instead.
 */
export const demoWav = (i: number): Blob => {
  const d = DICTATIONS[i];
  const rate = 16_000;
  const samples = Math.round((durationOf(d) / 1000) * rate);
  const times = timeWords(words(d.heard));
  const buffer = new ArrayBuffer(44 + samples * 2);
  const view = new DataView(buffer);
  const text = (at: number, s: string) =>
    [...s].forEach((c, k) => view.setUint8(at + k, c.charCodeAt(0)));
  text(0, "RIFF");
  view.setUint32(4, 36 + samples * 2, true);
  text(8, "WAVE");
  text(12, "fmt ");
  view.setUint32(16, 16, true);
  view.setUint16(20, 1, true);
  view.setUint16(22, 1, true);
  view.setUint32(24, rate, true);
  view.setUint32(28, rate * 2, true);
  view.setUint16(32, 2, true);
  view.setUint16(34, 16, true);
  text(36, "data");
  view.setUint32(40, samples * 2, true);
  let word = 0;
  for (let n = 0; n < samples; n++) {
    const ms = (n / rate) * 1000;
    while (word < times.length && ms > times[word].end) word++;
    const w = times[word];
    let level = 0.002 * (noise(n) - 0.5);
    if (w && ms >= w.start) {
      const shape = Math.sin((Math.PI * (ms - w.start)) / (w.end - w.start));
      const t = n / rate;
      const hum =
        Math.sin(2 * Math.PI * 140 * t) +
        0.5 * Math.sin(2 * Math.PI * 280 * t) +
        0.25 * Math.sin(2 * Math.PI * 420 * t);
      level += 0.18 * shape * hum;
    }
    view.setInt16(44 + n * 2, Math.max(-1, Math.min(1, level)) * 32767, true);
  }
  return new Blob([buffer], { type: "audio/wav" });
};

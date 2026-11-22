/**
 * Dispatch: generates candidate sound effects (railroad themed) with
 * ElevenLabs' sound-effects model, for Joshua to listen to and pick: which
 * plays when recording starts, when it stops, and when an achievement
 * unlocks (HANDOFF.md, "Sounds").
 *
 * Each sound is generated as an MP3, then trimmed of leading and trailing
 * silence, levelled to the same peak and saved as a WAV next to it, so they
 * can be compared fairly. Generated files are cached: run it again and only
 * new or changed prompts cost credits.
 *
 *   node scripts/dispatch/sound-candidates.mjs            # everything below
 *   node scripts/dispatch/sound-candidates.mjs steam bell # only those groups
 *
 * The API key comes from Dispatch's copy of the video kit (video/.env,
 * ELEVENLABS_API_KEY). Output: vendor/sound-samples/ (git-ignored).
 */
import { createHash } from "node:crypto";
import { execFileSync, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";

const ROOT = resolve(import.meta.dirname, "..", "..");
const OUT = join(ROOT, "vendor", "sound-samples");
const RAW = join(OUT, "raw");
const FFMPEG =
  process.env.FFMPEG ?? "C:/Program Files/Virtual Desktop Streamer/ffmpeg.exe";
const PEAK_DB = -3;

// Each group: a name, how many takes, seconds asked for, and the prompt.
// Feedback sounds must be short: the start sound plays before recording
// begins, so a long one delays the first words.
const GROUPS = [
  {
    group: "steam-release",
    takes: 3,
    seconds: 1,
    text: "A short, soft puff of steam released from an old steam locomotive, a quick gentle hiss that stops cleanly. Close up, no music.",
  },
  {
    group: "air-brake",
    takes: 3,
    seconds: 1,
    text: "A brief air brake release on a train, a quick pssht of compressed air, short and crisp. No music.",
  },
  {
    group: "steam-building",
    takes: 2,
    seconds: 2,
    text: "Steam pressure building in an old locomotive boiler, a rising hiss that swells then vents briefly. No music.",
  },
  {
    group: "train-horn",
    takes: 3,
    seconds: 1.5,
    text: "One short blast of an old-school diesel locomotive air horn in the distance, warm and not too loud. No music.",
  },
  {
    group: "steam-whistle",
    takes: 2,
    seconds: 1.5,
    text: "A single short toot of a vintage steam locomotive whistle, bright and clean. No music.",
  },
  {
    group: "crossing-bell",
    takes: 3,
    seconds: 2,
    text: "A railroad crossing bell ringing twice, ding ding, a clear mechanical bell, then silence. No music.",
  },
  {
    group: "bell-single",
    takes: 2,
    seconds: 1,
    text: "A single strike of a small railroad crossing bell, one clear ding. No music.",
  },
  {
    group: "rail-clack",
    takes: 2,
    seconds: 1,
    text: "Train wheels rolling over one rail joint, a quick clickety-clack, close up. No music.",
  },
  {
    group: "switch-lever",
    takes: 2,
    seconds: 1,
    text: "A heavy railroad switch lever being thrown, one solid metal clunk. No music.",
  },
];

function apiKey() {
  const env = readFileSync(join(ROOT, "video", ".env"), "utf8");
  const line = env
    .split(/\r?\n/)
    .find((l) => l.startsWith("ELEVENLABS_API_KEY="));
  const key = line
    ?.slice("ELEVENLABS_API_KEY=".length)
    .trim()
    .replace(/^["']|["']$/g, "");
  if (!key) throw new Error("No ELEVENLABS_API_KEY in video/.env");
  return key;
}

async function credits(key) {
  const r = await fetch("https://api.elevenlabs.io/v1/user/subscription", {
    headers: { "xi-api-key": key },
  });
  if (!r.ok) return null;
  const s = await r.json();
  return s.character_limit - s.character_count;
}

async function generate(key, text, seconds, file) {
  const r = await fetch(
    "https://api.elevenlabs.io/v1/sound-generation?output_format=mp3_44100_128",
    {
      method: "POST",
      headers: { "xi-api-key": key, "Content-Type": "application/json" },
      body: JSON.stringify({
        text,
        duration_seconds: seconds,
        prompt_influence: 0.6,
        model_id: "eleven_text_to_sound_v3",
      }),
    },
  );
  if (!r.ok) throw new Error(`ElevenLabs ${r.status}: ${await r.text()}`);
  writeFileSync(file, Buffer.from(await r.arrayBuffer()));
}

function ffmpeg(args) {
  return execFileSync(FFMPEG, ["-hide_banner", "-y", ...args], {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });
}

/** Trims silence at both ends, then levels the peak to PEAK_DB. */
function finish(mp3, wav) {
  const trim =
    "silenceremove=start_periods=1:start_threshold=-45dB," +
    "areverse,silenceremove=start_periods=1:start_threshold=-45dB,areverse";
  // volumedetect reports on stderr.
  const detect = spawnSync(
    FFMPEG,
    [
      "-hide_banner",
      "-i",
      mp3,
      "-af",
      `${trim},volumedetect`,
      "-f",
      "null",
      "-",
    ],
    { encoding: "utf8" },
  ).stderr;
  const max = Number(/max_volume:\s*(-?[\d.]+) dB/.exec(detect)?.[1] ?? 0);
  ffmpeg([
    "-i",
    mp3,
    "-af",
    `${trim},volume=${(PEAK_DB - max).toFixed(1)}dB,afade=t=in:d=0.005,areverse,afade=t=in:d=0.03,areverse`,
    "-ac",
    "1",
    "-ar",
    "44100",
    "-sample_fmt",
    "s16",
    wav,
  ]);
  const probe = execFileSync(
    FFMPEG.replace(/ffmpeg(\.exe)?$/i, "ffprobe$1"),
    ["-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", wav],
    { encoding: "utf8" },
  );
  return Number(probe.trim());
}

async function main() {
  const only = process.argv.slice(2);
  const key = apiKey();
  mkdirSync(RAW, { recursive: true });
  const before = await credits(key);
  const made = [];
  for (const g of GROUPS) {
    if (only.length && !only.some((o) => g.group.includes(o))) continue;
    for (let take = 1; take <= g.takes; take++) {
      const id = createHash("sha256")
        .update(`${g.text}|${g.seconds}|${take}`)
        .digest("hex")
        .slice(0, 10);
      const mp3 = join(RAW, `${g.group}-${take}-${id}.mp3`);
      if (!existsSync(mp3)) {
        process.stdout.write(`Generating ${g.group} ${take} ... `);
        await generate(key, g.text, g.seconds, mp3);
        process.stdout.write("done\n");
      }
      const wav = join(OUT, `${g.group}-${take}.wav`);
      const seconds = finish(mp3, wav);
      made.push({ file: `${g.group}-${take}.wav`, seconds });
    }
  }
  const after = await credits(key);
  console.log("\nCandidates (trimmed and levelled):");
  for (const m of made)
    console.log(`  ${m.file.padEnd(24)} ${m.seconds.toFixed(2)} s`);
  if (before !== null && after !== null) {
    console.log(
      `\nElevenLabs credits: ${before} before, ${after} after (${before - after} used).`,
    );
  }
  console.log(`\nFolder: ${OUT}`);
}

main().catch((e) => {
  console.error(e.message);
  process.exit(1);
});

// Dispatch achievements: the sound of an unlock. With the Railroad sound
// theme (Dispatch's default) it's a short, quiet steam whistle; with the
// others, a soft two-note bell made in code. Both follow Settings > Sound's
// feedback switch and volume.
import whistleUrl from "@/assets/dispatch/sounds/achievement-whistle.wav";
import type { SoundTheme } from "@/bindings";

const PARTIALS: [number, number][] = [
  [1, 1],
  [2.76, 0.32],
  [5.4, 0.1],
];

/** Plays the unlock sound for `theme` at `volume` (0 to 1). */
export const playAchievementSound = (
  theme: SoundTheme | undefined,
  volume = 1,
): void => {
  if (theme !== "railroad") {
    playChime(volume);
    return;
  }
  try {
    const whistle = new Audio(whistleUrl);
    whistle.volume = Math.max(0, Math.min(1, volume));
    void whistle.play().catch((error) => {
      console.warn("Couldn't play the achievement whistle:", error);
    });
  } catch (error) {
    console.warn("Couldn't play the achievement whistle:", error);
  }
};

/** Plays the chime at `volume` (0 to 1, the app's feedback volume). */
export const playChime = (volume = 1): void => {
  try {
    const context = new AudioContext();
    const master = context.createGain();
    master.gain.value = 0.1 * Math.max(0, Math.min(1, volume));
    master.connect(context.destination);

    const strike = (when: number, frequency: number) => {
      for (const [multiple, level] of PARTIALS) {
        const tone = context.createOscillator();
        tone.type = "sine";
        tone.frequency.value = frequency * multiple;
        const envelope = context.createGain();
        envelope.gain.setValueAtTime(0, when);
        envelope.gain.linearRampToValueAtTime(level, when + 0.006);
        envelope.gain.exponentialRampToValueAtTime(
          0.0001,
          when + 1.3 / Math.sqrt(multiple),
        );
        tone.connect(envelope).connect(master);
        tone.start(when);
        tone.stop(when + 1.5);
      }
    };

    const start = context.currentTime + 0.02;
    strike(start, 1318.5); // E6
    strike(start + 0.3, 1046.5); // C6
    window.setTimeout(() => context.close(), 2500);
  } catch (error) {
    console.warn("Couldn't play the achievement chime:", error);
  }
};

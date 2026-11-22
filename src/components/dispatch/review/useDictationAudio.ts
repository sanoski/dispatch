// Dispatch review window: plays one dictation's recording, all of it or one
// word with a little audio either side. Quiet recordings are turned up so
// they peak at a comfortable level (most microphones record well below it),
// and "Quiet background" plays a copy with the background noise turned down
// (dispatch_denoise.rs). The recording itself is never changed.
import { useCallback, useEffect, useRef, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { commands } from "@/bindings";

/** Audio played before and after a word, so it isn't clipped. */
export const WORD_PADDING_MS = 500;
export const SPEEDS = [1, 1.25, 1.5, 0.75] as const;
/** Playback aims for peaks about 3 dB below full scale. */
const TARGET_PEAK = 0.7;
/** At most 26 dB louder, so a near-silent clip isn't all hiss. */
const MAX_BOOST = 20;

/** How much to turn a recording up, from its loudest sample. */
export const playbackBoost = (peak: number | null | undefined): number =>
  peak && peak > 0 ? Math.min(MAX_BOOST, Math.max(1, TARGET_PEAK / peak)) : 1;

export interface DictationAudio {
  ready: boolean;
  failed: boolean;
  /** The cleaned copy is being made (the first time, about a second). */
  quieting: boolean;
  /** The cleaned copy couldn't be made; the recording plays as it is. */
  quietFailed: boolean;
  playing: boolean;
  positionMs: number;
  speed: number;
  toggle: () => void;
  seek: (ms: number) => void;
  /** Plays `startMs`..`endMs` with padding, then stops. */
  playRange: (startMs: number, endMs: number) => void;
  nextSpeed: () => void;
}

export const useDictationAudio = (
  fileName: string | null,
  /** The recording's loudest sample (DictationReview.peak), once known. */
  peak: number | null,
  /** Play the copy with the background quieted. */
  quiet: boolean,
): DictationAudio => {
  const audioRef = useRef<HTMLAudioElement | null>(null);
  const originalSrcRef = useRef<string | null>(null);
  const playingSrcRef = useRef<string | null>(null);
  const swappingRef = useRef(false);
  const [quietCopy, setQuietCopy] = useState<{
    fileName: string;
    src: string;
    peak: number;
  } | null>(null);
  const [quieting, setQuieting] = useState(false);
  const [quietFailed, setQuietFailed] = useState(false);
  const contextRef = useRef<AudioContext | null>(null);
  const gainRef = useRef<GainNode | null>(null);
  const boostRef = useRef(1);
  const stopAtRef = useRef<number | null>(null);
  const frameRef = useRef<number | null>(null);
  const [ready, setReady] = useState(false);
  const [failed, setFailed] = useState(false);
  const [playing, setPlaying] = useState(false);
  const [positionMs, setPositionMs] = useState(0);
  const [speed, setSpeed] = useState<number>(1);

  const stopFrames = () => {
    if (frameRef.current !== null) cancelAnimationFrame(frameRef.current);
    frameRef.current = null;
  };

  const tick = useCallback(() => {
    const audio = audioRef.current;
    if (!audio) return;
    const ms = audio.currentTime * 1000;
    if (stopAtRef.current !== null && ms >= stopAtRef.current) {
      audio.pause();
      stopAtRef.current = null;
    }
    setPositionMs(audio.currentTime * 1000);
    if (!audio.paused) frameRef.current = requestAnimationFrame(tick);
  }, []);

  useEffect(() => {
    setReady(false);
    setFailed(false);
    setPlaying(false);
    setPositionMs(0);
    stopAtRef.current = null;
    if (!fileName) return;
    let cancelled = false;
    let audio: HTMLAudioElement | null = null;
    commands
      .getAudioFilePath(fileName)
      .then((result) => {
        if (cancelled) return;
        if (result.status !== "ok") {
          setFailed(true);
          return;
        }
        audio = new Audio();
        // Lets the volume boost read the audio (the asset protocol allows
        // Dispatch's own window). Set before the source.
        const src = convertFileSrc(result.data, "asset");
        audio.crossOrigin = "anonymous";
        audio.src = src;
        originalSrcRef.current = src;
        playingSrcRef.current = src;
        audio.preload = "auto";
        audio.onloadedmetadata = () => setReady(true);
        audio.onerror = () => {
          swappingRef.current = false;
          setPlaying(false);
          // If the boosted way is ever refused, play it as recorded.
          if (audio && audio.crossOrigin !== null && !contextRef.current) {
            audio.crossOrigin = null;
            audio.src = src;
            playingSrcRef.current = src;
            return;
          }
          setFailed(true);
        };
        audio.onplay = () => {
          setPlaying(true);
          stopFrames();
          frameRef.current = requestAnimationFrame(tick);
        };
        audio.onpause = () => {
          stopFrames();
          // Switching copies pauses for a moment; it isn't the user's pause.
          if (swappingRef.current) return;
          setPlaying(false);
          if (audio) setPositionMs(audio.currentTime * 1000);
        };
        audio.onended = () => {
          stopAtRef.current = null;
        };
        audioRef.current = audio;
      })
      .catch(() => !cancelled && setFailed(true));
    return () => {
      cancelled = true;
      stopFrames();
      if (audio) {
        audio.pause();
        audio.src = "";
      }
      audioRef.current = null;
      originalSrcRef.current = null;
      playingSrcRef.current = null;
      void contextRef.current?.close();
      contextRef.current = null;
      gainRef.current = null;
    };
  }, [fileName, tick]);

  // The boost: recording -> gain -> limiter -> speakers, made on first play.
  const connectBoost = useCallback(() => {
    const audio = audioRef.current;
    if (!audio || contextRef.current) {
      void contextRef.current?.resume();
      return;
    }
    // Loaded without permission to read it (the fallback above): the boost
    // would only hear silence, so leave it out.
    if (audio.crossOrigin === null) return;
    try {
      const context = new AudioContext();
      const source = context.createMediaElementSource(audio);
      const gain = context.createGain();
      gain.gain.value = boostRef.current;
      const limiter = context.createDynamicsCompressor();
      limiter.threshold.value = -2;
      limiter.knee.value = 0;
      limiter.ratio.value = 20;
      limiter.attack.value = 0.002;
      limiter.release.value = 0.1;
      source.connect(gain).connect(limiter).connect(context.destination);
      contextRef.current = context;
      gainRef.current = gain;
    } catch (error) {
      // Plays at the recorded level instead.
      console.warn("Couldn't turn up the recording:", error);
    }
  }, []);

  // "Quiet background": the cleaned copy, made by Rust the first time.
  useEffect(() => {
    setQuietFailed(false);
    if (!quiet || !fileName) {
      setQuieting(false);
      return;
    }
    let cancelled = false;
    setQuieting(true);
    commands
      .getQuietDictationAudio(fileName)
      .then((result) => {
        if (cancelled) return;
        if (result.status === "ok") {
          setQuietCopy({
            fileName,
            src: convertFileSrc(result.data.path, "asset"),
            peak: result.data.peak,
          });
        } else {
          console.warn("Couldn't quiet the background:", result.error);
          setQuietFailed(true);
        }
      })
      .catch(() => !cancelled && setQuietFailed(true))
      .finally(() => !cancelled && setQuieting(false));
    return () => {
      cancelled = true;
    };
  }, [quiet, fileName]);

  const cleaned = quiet && quietCopy?.fileName === fileName ? quietCopy : null;

  // Switches between the recording and the cleaned copy in place: same
  // spot, still playing if it was. They line up sample for sample.
  useEffect(() => {
    const audio = audioRef.current;
    const original = originalSrcRef.current;
    if (!audio || !original || !ready) return;
    const want = cleaned?.src ?? original;
    if (playingSrcRef.current === want) return;
    const at = audio.currentTime;
    const wasPlaying = !audio.paused;
    const stopAt = stopAtRef.current;
    playingSrcRef.current = want;
    swappingRef.current = wasPlaying;
    audio.addEventListener(
      "loadedmetadata",
      () => {
        audio.currentTime = at;
        stopAtRef.current = stopAt;
        if (wasPlaying) {
          void audio
            .play()
            .catch(() => setFailed(true))
            .finally(() => {
              swappingRef.current = false;
            });
        }
      },
      { once: true },
    );
    audio.src = want;
  }, [cleaned, ready]);

  const activePeak = cleaned ? cleaned.peak : peak;
  useEffect(() => {
    boostRef.current = playbackBoost(activePeak);
    if (gainRef.current) gainRef.current.gain.value = boostRef.current;
  }, [activePeak]);

  useEffect(() => {
    if (audioRef.current) audioRef.current.playbackRate = speed;
  }, [speed, ready]);

  const toggle = useCallback(() => {
    const audio = audioRef.current;
    if (!audio) return;
    if (audio.paused) {
      stopAtRef.current = null;
      if (audio.ended) audio.currentTime = 0;
      connectBoost();
      void audio.play().catch(() => setFailed(true));
    } else {
      audio.pause();
    }
  }, [connectBoost]);

  const seek = useCallback((ms: number) => {
    const audio = audioRef.current;
    if (!audio) return;
    audio.currentTime = Math.max(0, ms / 1000);
    setPositionMs(audio.currentTime * 1000);
  }, []);

  const playRange = useCallback(
    (startMs: number, endMs: number) => {
      const audio = audioRef.current;
      if (!audio) return;
      audio.currentTime = Math.max(0, (startMs - WORD_PADDING_MS) / 1000);
      stopAtRef.current = endMs + WORD_PADDING_MS;
      connectBoost();
      void audio.play().catch(() => setFailed(true));
    },
    [connectBoost],
  );

  const nextSpeed = useCallback(() => {
    setSpeed((current) => {
      const index = SPEEDS.indexOf(current as (typeof SPEEDS)[number]);
      return SPEEDS[(index + 1) % SPEEDS.length];
    });
  }, []);

  return {
    ready,
    failed,
    quieting,
    quietFailed,
    playing,
    positionMs,
    speed,
    toggle,
    seek,
    playRange,
    nextSpeed,
  };
};

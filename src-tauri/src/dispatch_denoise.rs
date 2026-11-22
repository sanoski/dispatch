//! Dispatch: quiets the background (fans, hum, the room) in a dictation for
//! playback in the review window, so the voice is easier to hear. The saved
//! recording is never changed: transcription and the fine-tune both use it
//! as recorded (HANDOFF.md, "Quiet playback"). `dispatch_review` makes and
//! keeps the cleaned copies; this file is only the sound work, so the test
//! tool (`examples/dispatch_denoise_check.rs`) can use it too.
//!
//! RNNoise (nnnoiseless) works on 48 kHz audio, and Dispatch records at
//! 16 kHz, so the audio goes up to 48 kHz, through RNNoise, and back down.
//! The result lines up sample for sample with the recording, so word times
//! still match.

use nnnoiseless::DenoiseState;

/// Dispatch's recording rate.
pub const RATE: usize = 16_000;
/// 48 kHz over 16 kHz.
const FACTOR: usize = 3;
/// Low-pass filter for the rate changes (odd, so its delay is whole samples).
const TAPS: usize = 97;
/// The filter's delay, in 48 kHz samples.
const DELAY: usize = (TAPS - 1) / 2;

/// `pcm` (16 kHz, -1..1) with the background quieted, the same length.
pub fn clean(pcm: &[f32]) -> Vec<f32> {
    if pcm.is_empty() {
        return Vec::new();
    }
    let filter = low_pass();
    let frame = DenoiseState::FRAME_SIZE;

    // RNNoise works in 16-bit sample units, one 10 ms frame at a time. Its
    // output runs one frame behind, so one extra frame goes in and the first
    // comes out.
    let mut high = upsample(pcm, &filter);
    high.iter_mut().for_each(|s| *s *= 32768.0);
    high.resize(high.len().div_ceil(frame) * frame + frame, 0.0);
    let mut state = DenoiseState::new();
    let mut quiet = Vec::with_capacity(high.len());
    let mut output = vec![0.0f32; frame];
    for chunk in high.chunks(frame) {
        state.process_frame(&mut output, chunk);
        quiet.extend(output.iter().map(|s| s / 32768.0));
    }
    quiet.drain(..frame);
    quiet.truncate(pcm.len() * FACTOR);

    downsample(&quiet, &filter, pcm.len())
        .into_iter()
        .map(|s| s.clamp(-1.0, 1.0))
        .collect()
}

/// 16 kHz to 48 kHz, lined up with the input: each sample becomes three
/// (two of them zero to start), smoothed by the low-pass. Only the taps that
/// land on a real sample are added up.
fn upsample(pcm: &[f32], filter: &[f32]) -> Vec<f32> {
    (0..pcm.len() * FACTOR)
        .map(|n| {
            let at = n + DELAY;
            let mut sum = 0.0f32;
            let mut k = at % FACTOR;
            while k < filter.len() && k <= at {
                if let Some(sample) = pcm.get((at - k) / FACTOR) {
                    sum += filter[k] * sample;
                }
                k += FACTOR;
            }
            sum * FACTOR as f32
        })
        .collect()
}

/// 48 kHz to 16 kHz, `len` samples: the low-pass, worked out only at every
/// third sample (the ones kept).
fn downsample(high: &[f32], filter: &[f32], len: usize) -> Vec<f32> {
    (0..len)
        .map(|i| {
            let at = i * FACTOR + DELAY;
            filter
                .iter()
                .enumerate()
                .filter_map(|(k, tap)| Some(tap * high.get(at.checked_sub(k)?)?))
                .sum()
        })
        .collect()
}

/// A windowed-sinc low-pass at 7.5 kHz for 48 kHz audio (just under 16 kHz
/// audio's 8 kHz top), unity gain.
fn low_pass() -> Vec<f32> {
    use std::f64::consts::PI;
    let cutoff = 7_500.0 / (RATE * FACTOR) as f64;
    let mut taps: Vec<f64> = (0..TAPS)
        .map(|n| {
            let x = n as f64 - DELAY as f64;
            let sinc = if x == 0.0 {
                2.0 * cutoff
            } else {
                (2.0 * PI * cutoff * x).sin() / (PI * x)
            };
            let phase = 2.0 * PI * n as f64 / (TAPS - 1) as f64;
            let blackman = 0.42 - 0.5 * phase.cos() + 0.08 * (2.0 * phase).cos();
            sinc * blackman
        })
        .collect();
    let sum: f64 = taps.iter().sum();
    taps.iter_mut().for_each(|t| *t /= sum);
    taps.into_iter().map(|t| t as f32).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(hz: f32, seconds: f32, level: f32) -> Vec<f32> {
        (0..(RATE as f32 * seconds) as usize)
            .map(|i| level * (2.0 * std::f32::consts::PI * hz * i as f32 / RATE as f32).sin())
            .collect()
    }

    #[test]
    fn keeps_the_length() {
        for len in [0, 1, 479, 480, 16_001] {
            assert_eq!(clean(&vec![0.01; len]).len(), len);
        }
    }

    #[test]
    fn the_rate_change_alone_leaves_audio_in_place() {
        // Up to 48 kHz and back with nothing in between: a 1 kHz tone comes
        // back where it was, at the same level.
        let filter = low_pass();
        let input = tone(1_000.0, 0.2, 0.5);
        let back = downsample(&upsample(&input, &filter), &filter, input.len());
        for i in 100..input.len() - 100 {
            assert!(
                (back[i] - input[i]).abs() < 0.01,
                "{i}: {} vs {}",
                back[i],
                input[i]
            );
        }
    }

    #[test]
    fn quiets_steady_noise() {
        // A low, steady hum of many tones comes out much quieter.
        let mut noise = vec![0.0f32; RATE * 2];
        for (k, hz) in [210.0, 470.0, 830.0, 1_330.0, 2_710.0, 5_090.0]
            .iter()
            .enumerate()
        {
            for (s, t) in noise.iter_mut().zip(tone(*hz, 2.0, 0.004)) {
                *s += t * (1.0 + k as f32 * 0.1);
            }
        }
        let energy = |x: &[f32]| x.iter().map(|s| s * s).sum::<f32>();
        let out = clean(&noise);
        assert!(energy(&out[RATE / 2..]) < energy(&noise[RATE / 2..]) * 0.5);
    }
}

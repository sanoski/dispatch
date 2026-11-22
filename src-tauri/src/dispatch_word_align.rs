//! Dispatch: word timings for a transcript, borrowed from another model's
//! transcript of the same audio (HANDOFF.md, "Word timings with Whisper").
//!
//! Whisper writes Dispatch's text but only times whole sentences. Parakeet
//! times every word. This lines Parakeet's words up with Whisper's (a
//! word-level edit-distance alignment, the same idea as a diff), copies the
//! times across where they line up, and spreads the rest over the gaps
//! between their neighbours. Every word says how it got its time.
//!
//! Self-contained (std and serde only), so the timing test tool in
//! `examples/` can use the same code.

use serde::{Deserialize, Serialize};

/// How a word got its time.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Precision {
    /// Straight from the model that wrote the text.
    Exact,
    /// From the timing model's matching word.
    Aligned,
    /// Shared out from the gap around it; within a word or two.
    Estimated,
}

/// One word of the transcript and where it is in the audio.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct TimedWord {
    pub text: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub precision: Precision,
}

/// A timed word from the timing model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceWord {
    pub text: String,
    pub start_ms: i64,
    pub end_ms: i64,
}

/// Matching identical words is free; these are the other costs.
const SIMILAR_COST: f32 = 0.4;
const DIFFERENT_COST: f32 = 1.0;
const GAP_COST: f32 = 0.8;
/// Words at least this alike count as the same word misheard ("Olama" and
/// "Ollama").
const SIMILAR_RATIO: f32 = 0.5;

/// Times every word of `text` (split on whitespace, punctuation kept) using
/// `source`. Words without a match are spread over the gap between their
/// neighbours, or over `span` (the stretch of the clip with speech in it,
/// in ms) at the edges.
pub fn align(text: &str, source: &[SourceWord], span: (i64, i64)) -> Vec<TimedWord> {
    align_indexed(text, source, span)
        .into_iter()
        .map(|(word, _)| word)
        .collect()
}

/// `align`, plus for each word the index in `source` of the word it lined
/// up with, if any.
pub fn align_indexed(
    text: &str,
    source: &[SourceWord],
    span: (i64, i64),
) -> Vec<(TimedWord, Option<usize>)> {
    let target: Vec<&str> = text.split_whitespace().collect();
    let target_keys: Vec<String> = target.iter().map(|w| key(w)).collect();
    // Punctuation-only source words take no part; `kept` maps back.
    let kept: Vec<usize> = (0..source.len())
        .filter(|&i| !key(&source[i].text).is_empty())
        .collect();
    let source_keys: Vec<String> = kept.iter().map(|&i| key(&source[i].text)).collect();

    let matches = match_words(&target_keys, &source_keys);
    let mut out: Vec<TimedWord> = target
        .iter()
        .zip(&matches)
        .map(|(word, m)| match m {
            Some(j) => {
                let s = &source[kept[*j]];
                TimedWord {
                    text: word.to_string(),
                    start_ms: s.start_ms,
                    end_ms: s.end_ms.max(s.start_ms),
                    precision: Precision::Aligned,
                }
            }
            None => TimedWord {
                text: word.to_string(),
                start_ms: 0,
                end_ms: 0,
                precision: Precision::Estimated,
            },
        })
        .collect();
    fill_gaps(&mut out, span);
    tidy_ends(&mut out, span);
    out.into_iter()
        .zip(matches.into_iter().map(|m| m.map(|j| kept[j])))
        .collect()
}

/// Gaps up to this long between two words are closed by stretching the first.
const STRETCH_GAP_MS: i64 = 400;
/// How far the last word may run on past its time, within the speech span.
const LAST_WORD_TAIL_MS: i64 = 250;

/// Parakeet times a word by its first sound, so its end comes too early
/// (often 80 ms after the start). Runs each word on to the next one when
/// they're close, trims the odd overlap, and lets the last word ring out a
/// little, so a highlighted word covers the whole word.
fn tidy_ends(words: &mut [TimedWord], span: (i64, i64)) {
    for i in 0..words.len().saturating_sub(1) {
        let next_start = words[i + 1].start_ms;
        let word = &mut words[i];
        if word.end_ms > next_start {
            word.end_ms = next_start.max(word.start_ms);
        } else if next_start - word.end_ms <= STRETCH_GAP_MS {
            word.end_ms = next_start;
        }
    }
    if let Some(last) = words.last_mut() {
        let tail = (last.end_ms + LAST_WORD_TAIL_MS).min(span.1);
        last.end_ms = last.end_ms.max(tail);
    }
}

/// Times every word of `text` by spreading `span` over them by length. For
/// when no timing model is available.
pub fn estimate(text: &str, span: (i64, i64)) -> Vec<TimedWord> {
    align(text, &[], span)
}

/// The part of a clip with speech in it, in ms, found by loudness: from a
/// little before the first loud 20 ms frame to a little after the last.
/// The whole clip when it's all quiet.
pub fn speech_span(samples: &[f32], sample_rate: u32) -> (i64, i64) {
    const FRAME_MS: usize = 20;
    const PAD_MS: i64 = 120;
    let total_ms = samples.len() as i64 * 1000 / sample_rate.max(1) as i64;
    let frame = (sample_rate as usize * FRAME_MS / 1000).max(1);
    let rms: Vec<f32> = samples
        .chunks(frame)
        .map(|c| (c.iter().map(|s| s * s).sum::<f32>() / c.len() as f32).sqrt())
        .collect();
    let peak = rms.iter().cloned().fold(0.0f32, f32::max);
    let threshold = (peak * 0.1).max(0.003);
    let loud = |r: &&f32| **r >= threshold;
    let (Some(first), Some(last)) = (
        rms.iter().position(|r| loud(&r)),
        rms.iter().rposition(|r| loud(&r)),
    ) else {
        return (0, total_ms);
    };
    let start = (first * FRAME_MS) as i64 - PAD_MS;
    let end = ((last + 1) * FRAME_MS) as i64 + PAD_MS;
    (start.max(0), end.min(total_ms).max(start.max(0)))
}

/// Lower case, letters and digits only: "Ollama," and "ollama" match.
fn key(word: &str) -> String {
    word.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// For each target word, the source word it lines up with, if any. A
/// monotonic alignment: matches never cross.
fn match_words(target: &[String], source: &[String]) -> Vec<Option<usize>> {
    let (n, m) = (target.len(), source.len());
    // cost[i][j]: cheapest way to line up target[..i] with source[..j].
    let mut cost = vec![vec![0.0f32; m + 1]; n + 1];
    for (i, row) in cost.iter_mut().enumerate() {
        row[0] = i as f32 * GAP_COST;
    }
    for (j, cell) in cost[0].iter_mut().enumerate() {
        *cell = j as f32 * GAP_COST;
    }
    for i in 1..=n {
        for j in 1..=m {
            let pair = cost[i - 1][j - 1] + pair_cost(&target[i - 1], &source[j - 1]);
            let skip_target = cost[i - 1][j] + GAP_COST;
            let skip_source = cost[i][j - 1] + GAP_COST;
            cost[i][j] = pair.min(skip_target).min(skip_source);
        }
    }
    let mut matches = vec![None; n];
    let (mut i, mut j) = (n, m);
    while i > 0 && j > 0 {
        let here = cost[i][j];
        if (here - (cost[i - 1][j - 1] + pair_cost(&target[i - 1], &source[j - 1]))).abs() < 1e-4 {
            // An empty key (a lone dash, say) never takes a source word's time.
            if !target[i - 1].is_empty() {
                matches[i - 1] = Some(j - 1);
            }
            i -= 1;
            j -= 1;
        } else if (here - (cost[i - 1][j] + GAP_COST)).abs() < 1e-4 {
            i -= 1;
        } else {
            j -= 1;
        }
    }
    matches
}

fn pair_cost(a: &str, b: &str) -> f32 {
    if a.is_empty() || b.is_empty() {
        DIFFERENT_COST + GAP_COST
    } else if a == b {
        0.0
    } else if similarity(a, b) >= SIMILAR_RATIO {
        SIMILAR_COST
    } else {
        DIFFERENT_COST
    }
}

/// 1 for the same word, 0 for nothing in common (by edit distance).
fn similarity(a: &str, b: &str) -> f32 {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let above = row[j + 1];
            row[j + 1] = if ca == cb {
                diagonal
            } else {
                1 + diagonal.min(above).min(row[j])
            };
            diagonal = above;
        }
    }
    let longest = a.len().max(b.len()).max(1);
    1.0 - row[b.len()] as f32 / longest as f32
}

/// Gives every `Estimated` word a share of the time between the timed words
/// around it, by length, in order.
fn fill_gaps(words: &mut [TimedWord], span: (i64, i64)) {
    let mut i = 0;
    while i < words.len() {
        if words[i].precision != Precision::Estimated {
            i += 1;
            continue;
        }
        let run_start = i;
        while i < words.len() && words[i].precision == Precision::Estimated {
            i += 1;
        }
        let from = if run_start == 0 {
            span.0
        } else {
            words[run_start - 1].end_ms
        };
        let to = if i == words.len() {
            span.1
        } else {
            words[i].start_ms
        };
        let to = to.max(from);
        let run = &mut words[run_start..i];
        let weights: Vec<i64> = run
            .iter()
            .map(|w| key(&w.text).chars().count().max(1) as i64)
            .collect();
        let total: i64 = weights.iter().sum();
        let mut at = from;
        let mut used = 0;
        for (word, weight) in run.iter_mut().zip(weights) {
            used += weight;
            let end = from + (to - from) * used / total;
            word.start_ms = at;
            word.end_ms = end;
            at = end;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn src(words: &[(&str, i64, i64)]) -> Vec<SourceWord> {
        words
            .iter()
            .map(|(t, s, e)| SourceWord {
                text: t.to_string(),
                start_ms: *s,
                end_ms: *e,
            })
            .collect()
    }

    #[test]
    fn identical_words_take_the_timing_models_times() {
        let timed = align(
            "Hello there, world.",
            &src(&[
                ("hello", 100, 400),
                ("there", 450, 700),
                ("world", 800, 1200),
            ]),
            (0, 1500),
        );
        let times: Vec<(i64, i64)> = timed.iter().map(|w| (w.start_ms, w.end_ms)).collect();
        // Starts copied; ends run on to the next word, the last one rings out.
        assert_eq!(times, vec![(100, 450), (450, 800), (800, 1450)]);
        assert!(timed.iter().all(|w| w.precision == Precision::Aligned));
        assert_eq!(timed[1].text, "there,", "punctuation stays on the word");
    }

    #[test]
    fn a_misheard_word_still_lines_up() {
        // Whisper heard "Ollama", Parakeet heard "Alama".
        let timed = align(
            "running on Ollama now",
            &src(&[
                ("running", 0, 300),
                ("on", 320, 400),
                ("Alama", 420, 900),
                ("now", 950, 1100),
            ]),
            (0, 1200),
        );
        assert_eq!((timed[2].start_ms, timed[2].end_ms), (420, 950));
        assert_eq!(timed[2].precision, Precision::Aligned);
    }

    #[test]
    fn far_apart_words_keep_their_gap_and_overlaps_are_trimmed() {
        let timed = align(
            "one two three",
            &src(&[("one", 0, 80), ("two", 1000, 1300), ("three", 1250, 1500)]),
            (0, 1600),
        );
        assert_eq!(timed[0].end_ms, 80, "a long pause stays a pause");
        assert_eq!(timed[1].end_ms, 1250, "an overlap is trimmed");
        assert_eq!(
            timed[2].end_ms, 1600,
            "the last word runs on, within the span"
        );
    }

    #[test]
    fn words_only_whisper_heard_share_the_gap() {
        // Parakeet dropped "really" and "very".
        let timed = align(
            "it is really very fast",
            &src(&[("it", 0, 100), ("is", 120, 200), ("fast", 900, 1200)]),
            (0, 1300),
        );
        assert_eq!(timed[2].precision, Precision::Estimated);
        assert_eq!(timed[3].precision, Precision::Estimated);
        assert_eq!(timed[2].start_ms, 200);
        assert_eq!(timed[3].end_ms, 900);
        assert!(timed[2].end_ms <= timed[3].start_ms);
        // "really" is longer than "very", so it gets more of the gap.
        assert!(timed[2].end_ms - timed[2].start_ms > timed[3].end_ms - timed[3].start_ms);
    }

    #[test]
    fn extra_words_from_the_timing_model_are_skipped() {
        let timed = align(
            "so we start",
            &src(&[
                ("um", 0, 200),
                ("so", 250, 400),
                ("uh", 420, 500),
                ("we", 520, 600),
                ("start", 650, 900),
            ]),
            (0, 1000),
        );
        let starts: Vec<i64> = timed.iter().map(|w| w.start_ms).collect();
        assert_eq!(starts, vec![250, 520, 650]);
    }

    #[test]
    fn with_no_timing_model_every_word_is_estimated_in_order() {
        let timed = estimate("one two three", (1000, 2500));
        assert!(timed.iter().all(|w| w.precision == Precision::Estimated));
        assert_eq!(timed[0].start_ms, 1000);
        assert_eq!(timed[2].end_ms, 2500);
        assert!(timed.windows(2).all(|p| p[0].end_ms <= p[1].start_ms));
    }

    #[test]
    fn empty_text_gives_no_words() {
        assert!(align("   ", &src(&[("hi", 0, 100)]), (0, 200)).is_empty());
    }

    #[test]
    fn speech_span_skips_quiet_edges() {
        let rate = 16_000;
        let mut samples = vec![0.0f32; rate as usize]; // 1 s quiet
        samples.extend(std::iter::repeat_n(0.5f32, rate as usize / 2)); // 0.5 s loud
        samples.extend(vec![0.0f32; rate as usize]); // 1 s quiet
        let (start, end) = speech_span(&samples, rate);
        assert!((850..=1000).contains(&start), "{start}");
        assert!((1500..=1650).contains(&end), "{end}");
        assert_eq!(speech_span(&vec![0.0; 16_000], rate), (0, 1000));
    }
}

//! Dispatch: what the review window shows for a dictation, and the fixes made
//! there (HANDOFF.md, "Review window").
//!
//! For one dictation: a waveform drawn from its recording, every word of what
//! the speech model heard with its time (`dispatch_timings`), and the cleaned
//! text lined up with those words, so both can be clicked word by word.
//!
//! Fixes made in the review window are kept in Dispatch's own file per
//! recording (`dispatch\corrections\<recording>.json`), never in Handy's
//! history: the original transcript stays as it was, next to the corrected
//! one. That pair (audio, corrected text) is what the voice fine-tune learns
//! from. A fix can also add a rule to the Fix-It List.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::AppHandle;

use crate::dispatch_timings::{self, WordTimings};
use crate::dispatch_word_align::{self as align, Precision, SourceWord, TimedWord};

const SAMPLE_RATE: u32 = 16_000;
/// Bars in a list item's little waveform.
const SPARK_BARS: usize = 24;
/// Bars sent for the big waveform; the window draws as many as fit.
const WAVEFORM_BARS: usize = 320;
const CORRECTIONS_VERSION: u32 = 1;

/// One word as the review window shows it.
#[derive(Serialize, Debug, Clone, PartialEq, Type)]
pub struct ReviewWord {
    pub text: String,
    pub start_ms: f64,
    pub end_ms: f64,
    /// "aligned" or "estimated": how sure the time is.
    pub precision: String,
    /// The range of words in what the model heard that this word stands for
    /// (more than one when a fix replaced several). In the cleaned view, the
    /// heard words it lines up with; `None` when it lines up with none.
    pub heard_first: Option<u32>,
    pub heard_last: Option<u32>,
    /// Set when this word is a fix: what the model heard there.
    pub fixed_from: Option<String>,
}

/// Everything the review window shows for one dictation.
#[derive(Serialize, Debug, Clone, Type)]
pub struct DictationReview {
    pub duration_ms: f64,
    /// Loudness per slice of the clip, 0 to 1.
    pub waveform: Vec<f32>,
    /// What the speech model heard, with the user's fixes applied.
    pub heard: Vec<ReviewWord>,
    /// The cleaned text, lined up with the heard words; `None` when the
    /// dictation wasn't cleaned up.
    pub cleaned: Option<Vec<ReviewWord>>,
    /// What the times came from: a model's file name, or "estimate".
    pub timing_source: String,
    pub fix_count: u32,
    /// The recording's loudest sample, 0 to 1. Playback is turned up by
    /// this much so quiet recordings are easy to hear; the file isn't changed.
    pub peak: f32,
    /// Marked "Looks right": the user checked the text (after any fixes) and
    /// it's what they said.
    pub checked: bool,
    /// False when the recording was deleted to save space (`dispatch_audio`):
    /// the words are shown, but there's nothing to play.
    pub has_audio: bool,
}

/// A list item's extras.
#[derive(Serialize, Debug, Clone, Type)]
pub struct DictationSummary {
    pub file_name: String,
    pub duration_ms: f64,
    pub spark: Vec<f32>,
    pub fix_count: u32,
    pub checked: bool,
}

/// One fix: the heard words `first..=last` (indexes into the saved timings)
/// replaced by `meant`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct WordFix {
    first: usize,
    last: usize,
    heard: String,
    meant: String,
    /// Unix seconds.
    at: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct Corrections {
    version: u32,
    file_name: String,
    /// The heard text the fixes apply to. If the dictation is transcribed
    /// again, the fixes no longer fit and are set aside.
    original_text: String,
    /// The heard text with every fix applied: the fine-tune's target.
    corrected_text: String,
    fixes: Vec<WordFix>,
    updated_at: i64,
    /// When the user marked it "Looks right" (Unix seconds); training uses
    /// checked dictations. Older files don't have it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    checked_at: Option<i64>,
}

pub fn review(
    app: &AppHandle,
    file_name: &str,
    heard_text: &str,
    cleaned_text: Option<&str>,
) -> Result<DictationReview, String> {
    let audio = read_recording(app, file_name)?;
    let has_audio = audio.is_some();
    let pcm = audio.unwrap_or_default();
    let (timed, timing_source) = heard_words(app, file_name, heard_text, &pcm);
    let duration_ms = if has_audio {
        pcm.len() as f64 * 1000.0 / SAMPLE_RATE as f64
    } else {
        timed.last().map_or(0.0, |w| w.end_ms as f64)
    };
    let corrections = load_corrections(app, file_name).filter(|c| c.original_text == heard_text);
    let fixes = corrections
        .as_ref()
        .map(|c| c.fixes.as_slice())
        .unwrap_or(&[]);
    let heard = apply_fixes(&timed, fixes);
    let span = align::speech_span(&pcm, SAMPLE_RATE);
    let cleaned = cleaned_text
        .filter(|t| !t.trim().is_empty())
        .map(|text| line_up_cleaned(text, &heard, span));
    Ok(DictationReview {
        duration_ms,
        waveform: loudness(&pcm, WAVEFORM_BARS),
        heard,
        cleaned,
        timing_source,
        fix_count: fixes.len() as u32,
        peak: pcm.iter().fold(0.0f32, |peak, s| peak.max(s.abs())),
        checked: corrections.as_ref().is_some_and(|c| c.checked_at.is_some()),
        has_audio,
    })
}

/// Marks a dictation "Looks right", or takes the mark off. A dictation with
/// no fixes gets a corrections file of its own, with the heard text as its
/// corrected text.
pub fn set_checked(
    app: &AppHandle,
    file_name: &str,
    heard_text: &str,
    cleaned_text: Option<&str>,
    checked: bool,
) -> Result<DictationReview, String> {
    read_recording(app, file_name)?; // checks the name
    let mut corrections = load_corrections(app, file_name)
        .filter(|c| c.original_text == heard_text)
        .unwrap_or_else(|| new_corrections(file_name, heard_text));
    let now = chrono::Utc::now().timestamp();
    corrections.checked_at = checked.then_some(now);
    corrections.updated_at = now;
    write_corrections(app, &corrections)?;
    crate::dispatch_achievements::refresh();
    review(app, file_name, heard_text, cleaned_text)
}

fn new_corrections(file_name: &str, heard_text: &str) -> Corrections {
    Corrections {
        version: CORRECTIONS_VERSION,
        file_name: file_name.to_string(),
        original_text: heard_text.to_string(),
        corrected_text: heard_text.to_string(),
        fixes: Vec::new(),
        updated_at: 0,
        checked_at: None,
    }
}

/// A recording with its background quieted, for the review window's "Quiet
/// background" switch (`dispatch_denoise`).
#[derive(Serialize, Debug, Clone, Type)]
pub struct QuietCopy {
    /// The cleaned copy, a WAV file the window plays instead.
    pub path: String,
    /// Its loudest sample, for the playback boost.
    pub peak: f32,
}

/// Cleaned copies kept (`dispatch\quiet-playback\`); older ones are deleted.
/// Each is as big as its recording, so this stays a few megabytes.
const QUIET_COPIES_KEPT: usize = 30;

/// The cleaned copy of `file_name`, made the first time it's asked for.
pub fn quiet_copy(app: &AppHandle, file_name: &str) -> Result<QuietCopy, String> {
    let pcm = read_recording(app, file_name)?.ok_or("This recording wasn't kept")?;
    let dir = crate::portable::app_data_dir(app)
        .map_err(|e| e.to_string())?
        .join("dispatch")
        .join("quiet-playback");
    let path = dir.join(file_name);
    let cleaned = match crate::audio_toolkit::read_wav_samples(&path) {
        Ok(cleaned) if cleaned.len() == pcm.len() => {
            // Used again: it counts as new, so it's kept longest.
            if let Ok(file) = std::fs::File::options().append(true).open(&path) {
                let _ = file.set_modified(std::time::SystemTime::now());
            }
            cleaned
        }
        _ => {
            let cleaned = crate::dispatch_denoise::clean(&pcm);
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            let partial = path.with_extension("wav.partial");
            crate::audio_toolkit::save_wav_file(&partial, &cleaned)
                .map_err(|e| format!("Couldn't save the cleaned copy: {e}"))?;
            std::fs::rename(&partial, &path).map_err(|e| e.to_string())?;
            prune(&dir, QUIET_COPIES_KEPT);
            cleaned
        }
    };
    Ok(QuietCopy {
        path: path.to_string_lossy().into_owned(),
        peak: cleaned.iter().fold(0.0f32, |peak, s| peak.max(s.abs())),
    })
}

/// Deletes all but the `keep` newest files in `dir`.
fn prune(dir: &Path, keep: usize) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = entries
        .flatten()
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    files.sort_by_key(|f| std::cmp::Reverse(f.0));
    for (_, path) in files.into_iter().skip(keep) {
        let _ = std::fs::remove_file(path);
    }
}

/// How many dictations History holds in all (transcribed files included).
/// The list loads them a page at a time, so it can't count them itself.
pub fn dictation_count(app: &AppHandle) -> Result<u64, String> {
    let data_dir = crate::portable::app_data_dir(app).map_err(|e| e.to_string())?;
    count_dictations(&data_dir.join("history.db"))
}

fn count_dictations(db: &Path) -> Result<u64, String> {
    if !db.is_file() {
        return Ok(0);
    }
    let conn =
        rusqlite::Connection::open_with_flags(db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| e.to_string())?;
    let _ = conn.busy_timeout(std::time::Duration::from_millis(500));
    conn.query_row("SELECT COUNT(*) FROM transcription_history", [], |row| {
        row.get::<_, i64>(0)
    })
    .map(|n| n.max(0) as u64)
    .map_err(|e| e.to_string())
}

pub fn summaries(app: &AppHandle, entries: &[(String, String)]) -> Vec<DictationSummary> {
    entries
        .iter()
        .filter_map(|(file_name, heard_text)| {
            let pcm = read_recording(app, file_name).ok()?.unwrap_or_default();
            let corrections =
                load_corrections(app, file_name).filter(|c| &c.original_text == heard_text);
            Some(DictationSummary {
                file_name: file_name.clone(),
                duration_ms: pcm.len() as f64 * 1000.0 / SAMPLE_RATE as f64,
                spark: loudness(&pcm, SPARK_BARS),
                fix_count: corrections.as_ref().map_or(0, |c| c.fixes.len() as u32),
                checked: corrections.is_some_and(|c| c.checked_at.is_some()),
            })
        })
        .collect()
}

/// Replaces the heard words `first..=last` with `meant`, replacing any fix
/// it overlaps. With `add_rule`, also adds "heard means meant" to the Fix-It
/// List. Returns the updated review.
#[allow(clippy::too_many_arguments)]
pub fn save_fix(
    app: &AppHandle,
    file_name: &str,
    heard_text: &str,
    cleaned_text: Option<&str>,
    first: usize,
    last: usize,
    meant: &str,
    add_rule: bool,
) -> Result<DictationReview, String> {
    let typed = meant.split_whitespace().collect::<Vec<_>>().join(" ");
    let pcm = read_recording(app, file_name)?.unwrap_or_default();
    let (timed, _) = heard_words(app, file_name, heard_text, &pcm);
    if first > last || last >= timed.len() {
        return Err("That word isn't in this dictation any more".into());
    }

    let mut corrections = load_corrections(app, file_name)
        .filter(|c| c.original_text == heard_text)
        .unwrap_or_else(|| new_corrections(file_name, heard_text));
    let (first, meant) = keep_end_mark(&timed, &corrections.fixes, first, last, &typed);
    let heard = timed[first..=last]
        .iter()
        .map(|w| w.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    corrections
        .fixes
        .retain(|f| f.last < first || f.first > last);
    if meant != heard {
        corrections.fixes.push(WordFix {
            first,
            last,
            heard: heard.clone(),
            meant: meant.clone(),
            at: chrono::Utc::now().timestamp(),
        });
    }
    corrections.fixes.sort_by_key(|f| f.first);
    corrections.corrected_text = apply_fixes(&timed, &corrections.fixes)
        .iter()
        .map(|w| w.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    corrections.updated_at = chrono::Utc::now().timestamp();
    write_corrections(app, &corrections)?;

    // A deletion never makes a rule, even when it kept an end mark.
    if add_rule && !typed.is_empty() {
        add_fix_rule(app, &bare(&heard), &bare(&meant))?;
    }
    review(app, file_name, heard_text, cleaned_text)
}

/// The heard words with their times: the saved timings when they're for this
/// text, else estimated on the spot (the background job catches up later).
fn heard_words(
    app: &AppHandle,
    file_name: &str,
    heard_text: &str,
    pcm: &[f32],
) -> (Vec<TimedWord>, String) {
    match dispatch_timings::load(app, file_name) {
        Some(WordTimings {
            text,
            words,
            source,
            ..
        }) if text == heard_text => (words, source),
        _ => (
            align::estimate(heard_text, align::speech_span(pcm, SAMPLE_RATE)),
            "estimate".to_string(),
        ),
    }
}

/// The timed words with each fix's range replaced by one word carrying the
/// fix. `fixes` must not overlap.
/// Marks that end a sentence, kept when the word carrying one is deleted.
const END_MARKS: &[char] = &['.', '?', '!', '…'];

/// Deleting the last word of a sentence ("it?" in "for it?") would take its
/// question mark or full stop with it. Instead the fix grows to cover the
/// word before as well, which keeps the mark: "for it?" becomes "for?".
/// Returns the fix's first word and its text; anything other than such a
/// deletion comes back as it was.
fn keep_end_mark(
    timed: &[TimedWord],
    fixes: &[WordFix],
    first: usize,
    last: usize,
    meant: &str,
) -> (usize, String) {
    let unchanged = (first, meant.to_string());
    if !meant.is_empty() || first == 0 {
        return unchanged;
    }
    let deleted = &timed[last].text;
    let mark: String = deleted
        .chars()
        .rev()
        .take_while(|c| !c.is_alphanumeric())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    if !mark.contains(END_MARKS) {
        return unchanged;
    }
    // The word shown just before the deleted ones, with any fix it has.
    let Some(before) = apply_fixes(timed, fixes)
        .into_iter()
        .rev()
        .find(|w| w.heard_last.is_some_and(|l| (l as usize) < first))
    else {
        return unchanged;
    };
    let ends_marked = before
        .text
        .chars()
        .last()
        .is_some_and(|c| END_MARKS.contains(&c) || matches!(c, ',' | ';' | ':'));
    match before.heard_first {
        Some(from) if !ends_marked => (from as usize, format!("{}{mark}", before.text)),
        _ => unchanged,
    }
}

fn apply_fixes(timed: &[TimedWord], fixes: &[WordFix]) -> Vec<ReviewWord> {
    let mut out = Vec::with_capacity(timed.len());
    let mut i = 0;
    while i < timed.len() {
        if let Some(fix) = fixes.iter().find(|f| f.first == i && f.last < timed.len()) {
            out.push(ReviewWord {
                text: fix.meant.clone(),
                start_ms: timed[fix.first].start_ms as f64,
                end_ms: timed[fix.last].end_ms as f64,
                precision: precision_name(timed[fix.first].precision).into(),
                heard_first: Some(fix.first as u32),
                heard_last: Some(fix.last as u32),
                fixed_from: Some(fix.heard.clone()),
            });
            i = fix.last + 1;
            continue;
        }
        let w = &timed[i];
        out.push(ReviewWord {
            text: w.text.clone(),
            start_ms: w.start_ms as f64,
            end_ms: w.end_ms as f64,
            precision: precision_name(w.precision).into(),
            heard_first: Some(i as u32),
            heard_last: Some(i as u32),
            fixed_from: None,
        });
        i += 1;
    }
    out.retain(|w| !w.text.is_empty());
    out
}

/// The cleaned text's words, timed by lining them up with the heard words.
fn line_up_cleaned(text: &str, heard: &[ReviewWord], span: (i64, i64)) -> Vec<ReviewWord> {
    let source: Vec<SourceWord> = heard
        .iter()
        .map(|w| SourceWord {
            text: w.text.clone(),
            start_ms: w.start_ms as i64,
            end_ms: w.end_ms as i64,
        })
        .collect();
    align::align_indexed(text, &source, span)
        .into_iter()
        .map(|(word, matched)| {
            let heard_word = matched.map(|j| &heard[j]);
            ReviewWord {
                text: word.text,
                start_ms: word.start_ms as f64,
                end_ms: word.end_ms as f64,
                precision: precision_name(word.precision).into(),
                heard_first: heard_word.and_then(|h| h.heard_first),
                heard_last: heard_word.and_then(|h| h.heard_last),
                fixed_from: None,
            }
        })
        .collect()
}

fn precision_name(p: Precision) -> &'static str {
    match p {
        Precision::Exact | Precision::Aligned => "aligned",
        Precision::Estimated => "estimated",
    }
}

/// Peak loudness per slice, scaled so the loudest slice is 1 (square-rooted,
/// so quiet speech still shows).
fn loudness(pcm: &[f32], bars: usize) -> Vec<f32> {
    if pcm.is_empty() || bars == 0 {
        return vec![0.0; bars];
    }
    let peaks: Vec<f32> = (0..bars)
        .map(|b| {
            let from = b * pcm.len() / bars;
            let to = ((b + 1) * pcm.len() / bars).max(from + 1).min(pcm.len());
            pcm[from..to].iter().fold(0.0f32, |m, s| m.max(s.abs()))
        })
        .collect();
    let top = peaks.iter().cloned().fold(0.0f32, f32::max).max(1e-4);
    peaks.iter().map(|p| (p / top).sqrt().min(1.0)).collect()
}

/// Lower case without edge punctuation, for a Fix-It rule ("Coat," -> "coat").
fn bare(text: &str) -> String {
    text.split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn add_fix_rule(app: &AppHandle, heard: &str, meant: &str) -> Result<(), String> {
    if heard.is_empty() || meant.is_empty() || heard.eq_ignore_ascii_case(meant) {
        return Ok(());
    }
    let mut settings = crate::settings::get_settings(app);
    let mut rules = settings.dispatch_fixes.clone();
    rules.retain(|r| !r.heard.eq_ignore_ascii_case(heard));
    rules.push(crate::dispatch_fixes::FixRule {
        heard: heard.to_string(),
        meant: meant.to_string(),
    });
    settings.dispatch_fixes = crate::dispatch_fixes::clean_rules(rules)?;
    crate::settings::write_settings(app, settings);
    Ok(())
}

/// The recording's samples, or None if it isn't there any more (deleted to
/// save space, `dispatch_audio`). An error for a bad name or a broken file.
fn read_recording(app: &AppHandle, file_name: &str) -> Result<Option<Vec<f32>>, String> {
    if file_name.contains(['/', '\\']) || file_name.contains("..") {
        return Err("Not a recording name".into());
    }
    let path = crate::portable::app_data_dir(app)
        .map_err(|e| e.to_string())?
        .join("recordings")
        .join(file_name);
    if !path.is_file() {
        return Ok(None);
    }
    crate::audio_toolkit::audio::read_wav_samples(&path)
        .map(Some)
        .map_err(|e| format!("Couldn't read the recording: {e}"))
}

fn corrections_path(app: &AppHandle, file_name: &str) -> Option<PathBuf> {
    let stem = Path::new(file_name)
        .file_stem()?
        .to_string_lossy()
        .into_owned();
    Some(
        crate::portable::app_data_dir(app)
            .ok()?
            .join("dispatch")
            .join("corrections")
            .join(format!("{stem}.json")),
    )
}

fn load_corrections(app: &AppHandle, file_name: &str) -> Option<Corrections> {
    let bytes = std::fs::read(corrections_path(app, file_name)?).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn write_corrections(app: &AppHandle, corrections: &Corrections) -> Result<(), String> {
    let path = corrections_path(app, &corrections.file_name).ok_or("no app data folder")?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_vec_pretty(corrections).map_err(|e| e.to_string())?;
    let partial = path.with_extension("json.partial");
    std::fs::write(&partial, json).map_err(|e| e.to_string())?;
    std::fs::rename(&partial, &path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn counts_every_dictation_not_just_a_page() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("history.db");
        assert_eq!(super::count_dictations(&db).unwrap(), 0);
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute_batch("CREATE TABLE transcription_history (file_name TEXT)")
            .unwrap();
        for i in 0..137 {
            conn.execute(
                "INSERT INTO transcription_history (file_name) VALUES (?1)",
                [format!("{i}.wav")],
            )
            .unwrap();
        }
        assert_eq!(super::count_dictations(&db).unwrap(), 137);
    }

    use super::*;

    #[test]
    fn deleting_a_sentences_last_word_keeps_its_mark() {
        let words = timed(&[
            ("enough", 0, 100),
            ("data", 100, 200),
            ("for", 200, 300),
            ("it?", 300, 400),
        ]);
        let (first, meant) = keep_end_mark(&words, &[], 3, 3, "");
        assert_eq!((first, meant.as_str()), (2, "for?"));
        // The word before was already fixed: its fix is kept in the text.
        let (first, meant) = keep_end_mark(&words, &[fix(2, 2, "four")], 3, 3, "");
        assert_eq!((first, meant.as_str()), (2, "four?"));
    }

    #[test]
    fn other_fixes_leave_marks_alone() {
        let words = timed(&[("done.", 0, 100), ("okay.", 100, 200), ("so", 200, 300)]);
        // The word before already ends a sentence.
        assert_eq!(keep_end_mark(&words, &[], 1, 1, ""), (1, String::new()));
        // No mark on the deleted word.
        assert_eq!(keep_end_mark(&words, &[], 2, 2, ""), (2, String::new()));
        // Not a deletion.
        assert_eq!(
            keep_end_mark(&words, &[], 1, 1, "fine."),
            (1, "fine.".into())
        );
        // Nothing before it.
        assert_eq!(keep_end_mark(&words, &[], 0, 0, ""), (0, String::new()));
    }

    #[test]
    fn pruning_keeps_the_newest_copies() {
        let dir = tempfile::tempdir().unwrap();
        let now = std::time::SystemTime::now();
        for i in 0..5u64 {
            let path = dir.path().join(format!("{i}.wav"));
            let file = std::fs::File::create(&path).unwrap();
            file.set_modified(now - std::time::Duration::from_secs(100 - i))
                .unwrap();
        }
        prune(dir.path(), 2);
        let mut left: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        left.sort();
        assert_eq!(left, ["3.wav", "4.wav"]);
    }

    fn timed(words: &[(&str, i64, i64)]) -> Vec<TimedWord> {
        words
            .iter()
            .map(|(t, s, e)| TimedWord {
                text: t.to_string(),
                start_ms: *s,
                end_ms: *e,
                precision: Precision::Aligned,
            })
            .collect()
    }

    fn fix(first: usize, last: usize, meant: &str) -> WordFix {
        WordFix {
            first,
            last,
            heard: String::new(),
            meant: meant.into(),
            at: 0,
        }
    }

    #[test]
    fn a_fix_replaces_its_range_with_one_word_spanning_it() {
        let words = timed(&[
            ("open", 0, 300),
            ("quad", 300, 600),
            ("coat", 600, 900),
            ("now", 900, 1200),
        ]);
        let mut f = fix(1, 2, "Claude Code");
        f.heard = "quad coat".into();
        let shown = apply_fixes(&words, &[f]);
        let texts: Vec<&str> = shown.iter().map(|w| w.text.as_str()).collect();
        assert_eq!(texts, vec!["open", "Claude Code", "now"]);
        assert_eq!((shown[1].start_ms, shown[1].end_ms), (300.0, 900.0));
        assert_eq!(
            (shown[1].heard_first, shown[1].heard_last),
            (Some(1), Some(2))
        );
        assert_eq!(shown[1].fixed_from.as_deref(), Some("quad coat"));
        assert_eq!(
            shown[2].heard_first,
            Some(3),
            "later words keep their original index"
        );
    }

    #[test]
    fn deleting_a_word_is_a_fix_to_nothing() {
        let words = timed(&[("so", 0, 100), ("um", 100, 200), ("yes", 200, 300)]);
        let texts: Vec<String> = apply_fixes(&words, &[fix(1, 1, "")])
            .into_iter()
            .map(|w| w.text)
            .collect();
        assert_eq!(texts, vec!["so", "yes"]);
    }

    #[test]
    fn cleaned_words_point_back_at_the_heard_words() {
        let heard = apply_fixes(
            &timed(&[
                ("okay", 0, 300),
                ("um", 300, 500),
                ("open", 500, 800),
                ("it", 800, 1000),
            ]),
            &[],
        );
        let cleaned = line_up_cleaned("Okay, open it.", &heard, (0, 1100));
        let first: Vec<Option<u32>> = cleaned.iter().map(|w| w.heard_first).collect();
        assert_eq!(first, vec![Some(0), Some(2), Some(3)]);
        assert_eq!(cleaned[1].start_ms, 500.0);
    }

    #[test]
    fn loudness_is_scaled_to_the_loudest_slice() {
        let mut pcm = vec![0.0f32; 1600];
        pcm.extend(vec![0.25f32; 1600]);
        let bars = loudness(&pcm, 2);
        assert_eq!(bars, vec![0.0, 1.0]);
        assert_eq!(loudness(&[], 3), vec![0.0; 3]);
    }

    #[test]
    fn rules_are_made_from_bare_words() {
        assert_eq!(bare("Quad coat,"), "Quad coat");
        assert_eq!(bare("  \"Claude   Code.\" "), "Claude Code");
    }
}

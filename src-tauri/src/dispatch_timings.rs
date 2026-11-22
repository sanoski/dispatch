//! Dispatch: word timings for every saved dictation (HANDOFF.md, "Word
//! timings with Whisper"). The review window needs them to tie each word to
//! its spot on the waveform.
//!
//! After a dictation is saved, and for any older ones at startup, a
//! background job reads the recording, runs Parakeet on it for its word
//! times, and lines those up with the saved text (`dispatch_word_align`).
//! Parakeet runs in its own worker process, the same way Handy runs the main
//! speech model, so a GPU fault there can't take Dispatch down. It unloads
//! after a couple of idle minutes. Without Parakeet downloaded, the times are
//! estimated from the text and the loud part of the clip.
//!
//! Each recording gets a small JSON file in `dispatch\word-timings\` under
//! the app data folder, named after its WAV. A file is redone when the saved
//! text changes, and removed when its dictation is deleted. Nothing in
//! Handy's own history or recordings is changed.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tauri_specta::Event;
use transcribe_cpp::{Backend, RunOptions, TimestampKind};

use crate::dispatch_word_align::{self as align, Precision, SourceWord, TimedWord};
use crate::engine_supervisor::{DeviceSelector, EngineSupervisor, LoadSpec};
use crate::managers::history::HistoryUpdatePayload;
use crate::managers::model::ModelManager;

/// Bumped when the file format or the alignment changes, so old files are
/// redone.
const FORMAT_VERSION: u32 = 1;
/// Wait this long after startup before catching up on old dictations, so
/// Dispatch's own startup isn't slowed.
const STARTUP_DELAY: Duration = Duration::from_secs(20);
/// Unload Parakeet after this long with nothing to time.
const IDLE_UNLOAD: Duration = Duration::from_secs(120);
const SAMPLE_RATE: u32 = 16_000;

/// One dictation's word timings, as saved.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct WordTimings {
    pub version: u32,
    /// The recording, as named in history.
    pub file_name: String,
    /// The text the words belong to (history's transcript). When it changes,
    /// the timings are redone.
    pub text: String,
    /// What the times came from: the timing model's file name, or "estimate".
    pub source: String,
    pub duration_ms: i64,
    /// Unix seconds.
    pub created_at: i64,
    pub words: Vec<TimedWord>,
}

/// Starts the job: catches up on old dictations shortly after startup, then
/// times each new one as history saves it.
pub fn start(app: &AppHandle) {
    let (tx, rx) = mpsc::channel::<()>();
    if TRIGGER.set(tx.clone()).is_err() {
        return; // already running
    }
    let worker_app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("dispatch-word-timings".into())
        .spawn(move || run(worker_app, rx));
    if let Err(e) = spawned {
        warn!("Word timings: couldn't start the background job: {e}");
        return;
    }
    HistoryUpdatePayload::listen_any(app, move |event| {
        if matches!(
            event.payload,
            HistoryUpdatePayload::Added { .. }
                | HistoryUpdatePayload::Updated { .. }
                | HistoryUpdatePayload::Deleted { .. }
        ) {
            let _ = tx.send(());
        }
    });
    let delayed = TRIGGER.get().cloned();
    std::thread::spawn(move || {
        std::thread::sleep(STARTUP_DELAY);
        if let Some(tx) = delayed {
            let _ = tx.send(());
        }
    });
}

static TRIGGER: OnceLock<mpsc::Sender<()>> = OnceLock::new();

/// The saved timings for a recording, if any.
pub fn load(app: &AppHandle, file_name: &str) -> Option<WordTimings> {
    let path = timings_dir(app)?.join(timings_file_name(file_name));
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn run(app: AppHandle, rx: mpsc::Receiver<()>) {
    let mut timer = Timer::new();
    loop {
        match rx.recv_timeout(IDLE_UNLOAD) {
            Ok(()) => {
                // Several saves in a row: one pass covers them all.
                while rx.try_recv().is_ok() {}
                if let Err(e) = sync(&app, &mut timer) {
                    warn!("Word timings: {e}");
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => timer.unload(),
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        }
    }
}

/// Times every dictation whose timings are missing, out of date, or for
/// different text, and removes timings whose dictation is gone.
fn sync(app: &AppHandle, timer: &mut Timer) -> Result<(), String> {
    let dir = timings_dir(app).ok_or("no app data folder")?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("can't create {}: {e}", dir.display()))?;
    let recordings = crate::portable::app_data_dir(app)
        .map_err(|e| e.to_string())?
        .join("recordings");
    let entries = dictations(app)?;

    let started = Instant::now();
    let (mut done, mut words, mut aligned) = (0usize, 0usize, 0usize);
    let mut keep = HashSet::new();
    for (file_name, text) in &entries {
        let target = dir.join(timings_file_name(file_name));
        keep.insert(timings_file_name(file_name));
        if up_to_date(&target, text) {
            continue;
        }
        let wav = recordings.join(file_name);
        if !wav.is_file() {
            continue;
        }
        match time_one(app, timer, file_name, text, &wav) {
            Ok(timings) => {
                words += timings.words.len();
                aligned += timings
                    .words
                    .iter()
                    .filter(|w| w.precision != Precision::Estimated)
                    .count();
                write_json(&target, &timings)?;
                done += 1;
            }
            Err(e) => warn!("Word timings: {file_name}: {e}"),
        }
    }
    remove_orphans(&dir, &keep);
    if done > 0 {
        info!(
            "Word timings: timed {done} dictation(s), {aligned} of {words} words from {} ({} estimated), in {} ms",
            timer.source_name(),
            words - aligned,
            started.elapsed().as_millis()
        );
    }
    Ok(())
}

fn time_one(
    app: &AppHandle,
    timer: &mut Timer,
    file_name: &str,
    text: &str,
    wav: &Path,
) -> Result<WordTimings, String> {
    let pcm = crate::audio_toolkit::audio::read_wav_samples(wav).map_err(|e| e.to_string())?;
    let duration_ms = pcm.len() as i64 * 1000 / SAMPLE_RATE as i64;
    let span = align::speech_span(&pcm, SAMPLE_RATE);
    let (words, source) = match timer.words(app, pcm) {
        Ok(Some((source_words, source))) => (align::align(text, &source_words, span), source),
        Ok(None) => (align::estimate(text, span), "estimate".to_string()),
        Err(e) => {
            warn!("Word timings: Parakeet failed on {file_name}, estimating instead: {e}");
            (align::estimate(text, span), "estimate".to_string())
        }
    };
    Ok(WordTimings {
        version: FORMAT_VERSION,
        file_name: file_name.to_string(),
        text: text.to_string(),
        source,
        duration_ms,
        created_at: chrono::Utc::now().timestamp(),
        words,
    })
}

/// Parakeet in its own worker, loaded on first use.
struct Timer {
    engine: Option<EngineSupervisor>,
    loaded: Option<PathBuf>,
    /// Set once no Parakeet model was found, until the next pass.
    missing: bool,
}

impl Timer {
    fn new() -> Self {
        Self {
            engine: None,
            loaded: None,
            missing: false,
        }
    }

    fn source_name(&self) -> String {
        self.loaded
            .as_deref()
            .and_then(Path::file_name)
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "estimates".to_string())
    }

    /// Parakeet's timed words for `pcm` and the model's file name, or `None`
    /// when no Parakeet model is downloaded.
    fn words(
        &mut self,
        app: &AppHandle,
        pcm: Vec<f32>,
    ) -> Result<Option<(Vec<SourceWord>, String)>, String> {
        let Some(path) = parakeet_path(app) else {
            if !self.missing {
                info!("Word timings: no Parakeet model downloaded, so times are estimated");
                self.missing = true;
            }
            return Ok(None);
        };
        self.missing = false;
        let engine = self.engine.get_or_insert_with(|| {
            EngineSupervisor::new(!crate::utils::is_windows_x64_emulated_on_arm64())
        });
        if self.loaded.as_ref() != Some(&path) {
            let started = Instant::now();
            engine
                .load(LoadSpec {
                    path: path.clone(),
                    backend: Backend::Auto,
                    device: DeviceSelector::Auto,
                })
                .map_err(|e| format!("couldn't load {}: {e}", path.display()))?;
            info!(
                "Word timings: loaded {} in {} ms",
                path.display(),
                started.elapsed().as_millis()
            );
            self.loaded = Some(path.clone());
        }
        let options = RunOptions {
            timestamps: TimestampKind::Word,
            ..Default::default()
        };
        let transcript = engine.transcribe(pcm, options).map_err(|e| e.to_string())?;
        let words = transcript
            .words
            .into_iter()
            .map(|w| SourceWord {
                text: w.text,
                start_ms: w.t0_ms,
                end_ms: w.t1_ms,
            })
            .collect();
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        Ok(Some((words, name)))
    }

    fn unload(&mut self) {
        if let Some(engine) = &self.engine {
            if self.loaded.take().is_some() {
                engine.unload().wait();
                debug!(
                    "Word timings: unloaded Parakeet after {} s idle",
                    IDLE_UNLOAD.as_secs()
                );
            }
        }
    }
}

/// A downloaded Parakeet model, preferring the English one Dispatch tested.
fn parakeet_path(app: &AppHandle) -> Option<PathBuf> {
    let models = app.try_state::<Arc<ModelManager>>()?;
    let mut candidates: Vec<_> = models
        .get_available_models()
        .into_iter()
        .filter(|m| m.is_downloaded && m.filename.to_lowercase().contains("parakeet"))
        .collect();
    candidates.sort_by_key(|m| !m.filename.to_lowercase().contains("unified-en"));
    candidates
        .into_iter()
        .find_map(|m| models.get_model_path(&m.id).ok())
}

/// Every dictation in history (not transcribed files): its recording and text.
fn dictations(app: &AppHandle) -> Result<Vec<(String, String)>, String> {
    let db = crate::portable::app_data_dir(app)
        .map_err(|e| e.to_string())?
        .join("history.db");
    let conn =
        rusqlite::Connection::open_with_flags(&db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| format!("can't open {}: {e}", db.display()))?;
    let mut stmt = conn
        .prepare(
            "SELECT file_name, transcription_text FROM transcription_history \
             WHERE source_name IS NULL ORDER BY id DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<_, _>>().map_err(|e| e.to_string())
}

fn timings_dir(app: &AppHandle) -> Option<PathBuf> {
    Some(
        crate::portable::app_data_dir(app)
            .ok()?
            .join("dispatch")
            .join("word-timings"),
    )
}

/// "handy-123.wav" -> "handy-123.json".
fn timings_file_name(file_name: &str) -> String {
    let stem = Path::new(file_name)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| file_name.to_string());
    format!("{stem}.json")
}

fn up_to_date(path: &Path, text: &str) -> bool {
    std::fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<WordTimings>(&bytes).ok())
        .is_some_and(|t| t.version == FORMAT_VERSION && t.text == text)
}

fn write_json(path: &Path, timings: &WordTimings) -> Result<(), String> {
    let json = serde_json::to_vec_pretty(timings).map_err(|e| e.to_string())?;
    // Write then rename, so a crash never leaves half a file.
    let partial = path.with_extension("json.partial");
    std::fs::write(&partial, json)
        .map_err(|e| format!("can't write {}: {e}", partial.display()))?;
    std::fs::rename(&partial, path).map_err(|e| format!("can't save {}: {e}", path.display()))
}

fn remove_orphans(dir: &Path, keep: &HashSet<String>) {
    let Ok(files) = std::fs::read_dir(dir) else {
        return;
    };
    for file in files.flatten() {
        let name = file.file_name().to_string_lossy().into_owned();
        if name.ends_with(".json") && !keep.contains(&name) {
            match std::fs::remove_file(file.path()) {
                Ok(()) => debug!("Word timings: removed {name}, its dictation is gone"),
                Err(e) => warn!("Word timings: couldn't remove {name}: {e}"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn timings(text: &str) -> WordTimings {
        WordTimings {
            version: FORMAT_VERSION,
            file_name: "handy-1.wav".into(),
            text: text.into(),
            source: "estimate".into(),
            duration_ms: 1000,
            created_at: 0,
            words: align::estimate(text, (0, 1000)),
        }
    }

    #[test]
    fn timings_files_are_named_after_their_recording() {
        assert_eq!(
            timings_file_name("handy-1795313048.wav"),
            "handy-1795313048.json"
        );
    }

    #[test]
    fn timings_are_redone_when_the_text_or_format_changes() {
        let dir =
            std::env::temp_dir().join(format!("dispatch-timings-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("handy-1.json");
        write_json(&path, &timings("hello there")).unwrap();
        assert!(up_to_date(&path, "hello there"));
        assert!(!up_to_date(&path, "hello their"), "new text");
        let mut old = timings("hello there");
        old.version = FORMAT_VERSION - 1;
        write_json(&path, &old).unwrap();
        assert!(!up_to_date(&path, "hello there"), "old format");
        assert!(!up_to_date(&dir.join("missing.json"), "hello there"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn orphaned_timings_are_removed() {
        let dir =
            std::env::temp_dir().join(format!("dispatch-orphans-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("kept.json"), "{}").unwrap();
        std::fs::write(dir.join("gone.json"), "{}").unwrap();
        std::fs::write(dir.join("notes.txt"), "not ours").unwrap();
        remove_orphans(&dir, &HashSet::from(["kept.json".to_string()]));
        assert!(dir.join("kept.json").exists());
        assert!(!dir.join("gone.json").exists());
        assert!(
            dir.join("notes.txt").exists(),
            "only timings files are touched"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

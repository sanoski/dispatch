//! Dispatch: which recordings are kept (HANDOFF.md, "Keeping recordings").
//!
//! By default every recording is kept: each one can be replayed in the
//! review window, and each is training data for the voice fine-tune. People
//! who don't want that can keep only the newest [`NEWEST_KEPT`]. Unlike
//! Handy's "keep latest N", which deletes whole dictations, this removes
//! only old audio. Every transcript stays in History (achievements are
//! counted from them), and starred dictations keep their audio.
//!
//! Nothing is deleted outright: old recordings go to the Recycle Bin, so a
//! mistaken switch can be undone there. The settings page shows what a
//! switch would remove ([`cleanup_preview`]) and asks first.
//!
//! Handy's own clean-up stays off (`recording_retention_period` is Never,
//! set by Dispatch's defaults and by [`set_keep_all`]), so the two never
//! fight over the same files.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use log::{debug, warn};
use serde::Serialize;
use specta::Type;
use tauri::AppHandle;

use crate::settings::{get_settings, write_settings, RecordingRetentionPeriod};

/// Recordings kept when not keeping all of them: enough to see the waveform
/// and replay the latest dictations.
pub const NEWEST_KEPT: usize = 10;

/// One clean-up at a time.
static PRUNING: Mutex<()> = Mutex::new(());

pub fn default_keep_all() -> bool {
    true
}

/// What switching to "keep the newest" would move to the Recycle Bin now.
#[derive(Serialize, Debug, Clone, Default, Type)]
pub struct AudioCleanup {
    pub recordings: u32,
    /// Their total length.
    pub duration_ms: f64,
    pub bytes: f64,
    /// How many are kept: [`NEWEST_KEPT`].
    pub kept: u32,
}

/// Moves old audio to the Recycle Bin after each new dictation, when only
/// the newest are kept.
pub fn start(app: &AppHandle) {
    use crate::managers::history::HistoryUpdatePayload;
    use tauri_specta::Event;
    let handle = app.clone();
    HistoryUpdatePayload::listen_any(app, move |event| {
        if !matches!(event.payload, HistoryUpdatePayload::Added { .. }) {
            return;
        }
        if get_settings(&handle).dispatch_keep_all_audio {
            return;
        }
        let handle = handle.clone();
        std::thread::spawn(move || prune(&handle));
    });
}

/// What [`set_keep_all`] with `false` would move to the Recycle Bin now.
pub fn cleanup_preview(app: &AppHandle) -> Result<AudioCleanup, String> {
    let mut preview = AudioCleanup {
        kept: NEWEST_KEPT as u32,
        ..Default::default()
    };
    for path in old_recording_paths(app)? {
        let Ok(meta) = std::fs::metadata(&path) else {
            continue;
        };
        preview.recordings += 1;
        preview.bytes += meta.len() as f64;
        // 16 kHz, 16-bit mono after a 44-byte header: 32,000 bytes a second.
        preview.duration_ms += meta.len().saturating_sub(44) as f64 / 32.0;
    }
    Ok(preview)
}

/// Changes the setting. Turning "keep all" off moves the old audio to the
/// Recycle Bin now; the page has already shown what that means and asked.
pub fn set_keep_all(app: &AppHandle, keep_all: bool) {
    let mut settings = get_settings(app);
    settings.dispatch_keep_all_audio = keep_all;
    // Handy's clean-up would delete whole dictations; Dispatch's replaces it.
    settings.recording_retention_period = RecordingRetentionPeriod::Never;
    write_settings(app, settings);
    if !keep_all {
        prune(app);
    }
    crate::dispatch_achievements::refresh();
}

/// Moves the audio of every dictation past the newest [`NEWEST_KEPT`] (except
/// starred ones) to the Recycle Bin, and deletes their review-window
/// playback copies (those are only a cache).
fn prune(app: &AppHandle) {
    let _one_at_a_time = PRUNING.lock();
    let paths = match old_recording_paths(app) {
        Ok(paths) => paths,
        Err(e) => {
            warn!("Keeping recordings: couldn't read history: {e}");
            return;
        }
    };
    if paths.is_empty() {
        return;
    }
    let quiet = crate::portable::app_data_dir(app)
        .map(|dir| dir.join("dispatch").join("quiet-playback"))
        .ok();
    match trash::delete_all(&paths) {
        Ok(()) => debug!(
            "Keeping recordings: moved {} old recordings to the Recycle Bin",
            paths.len()
        ),
        Err(e) => {
            warn!("Keeping recordings: couldn't move old recordings to the Recycle Bin: {e}");
            return;
        }
    }
    if let Some(quiet) = quiet {
        for path in &paths {
            if let Some(name) = path.file_name() {
                let _ = std::fs::remove_file(quiet.join(name));
            }
        }
    }
}

/// The old recordings still on disk, past the newest [`NEWEST_KEPT`].
fn old_recording_paths(app: &AppHandle) -> Result<Vec<PathBuf>, String> {
    let data_dir = crate::portable::app_data_dir(app).map_err(|e| e.to_string())?;
    let recordings = data_dir.join("recordings");
    Ok(old_recordings(&data_dir.join("history.db"), NEWEST_KEPT)?
        .into_iter()
        .filter(|name| !name.contains(['/', '\\']) && !name.contains(".."))
        .map(|name| recordings.join(name))
        .filter(|path| path.is_file())
        .collect())
}

/// The recordings of dictations past the newest `keep`, newest first,
/// leaving out starred ones and transcribed files.
fn old_recordings(db: &Path, keep: usize) -> Result<Vec<String>, String> {
    if !db.is_file() {
        return Ok(Vec::new());
    }
    let conn =
        rusqlite::Connection::open_with_flags(db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| e.to_string())?;
    let _ = conn.busy_timeout(std::time::Duration::from_millis(500));
    let mut statement = conn
        .prepare(
            "SELECT file_name, saved FROM transcription_history \
             WHERE source_name IS NULL ORDER BY timestamp DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, bool>(1)?))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows
        .into_iter()
        .skip(keep)
        .filter(|(_, starred)| !starred)
        .map(|(name, _)| name)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_newest_and_the_starred() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("history.db");
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute_batch(
            "CREATE TABLE transcription_history (file_name TEXT, timestamp INTEGER, \
             saved BOOLEAN, source_name TEXT);",
        )
        .unwrap();
        for (i, saved, source) in [
            (1, false, None),
            (2, true, None),              // starred: keeps its audio
            (3, false, Some("call.mp3")), // a transcribed file: never touched
            (4, false, None),
            (5, false, None),
            (6, false, None),
        ] {
            conn.execute(
                "INSERT INTO transcription_history VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![format!("{i}.wav"), i, saved, source],
            )
            .unwrap();
        }
        drop(conn);
        // Newest two (6, 5) stay; of the rest, 2 is starred and 3 is a file.
        assert_eq!(old_recordings(&db, 2).unwrap(), ["4.wav", "1.wav"]);
        assert!(old_recordings(&db, 10).unwrap().is_empty());
        assert!(old_recordings(&dir.path().join("none.db"), 2)
            .unwrap()
            .is_empty());
    }
}

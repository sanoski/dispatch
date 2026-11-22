//! Tauri commands for Dispatch-only settings (see `crate::dispatch`).

use crate::dispatch::POST_PROCESS_TIMEOUT_RANGE_SECS;
use crate::settings::{get_settings, write_settings};
use tauri::AppHandle;

/// The Kokoro voices read aloud can use, such as "bm_george".
#[tauri::command]
#[specta::specta]
pub fn get_read_aloud_voices() -> Vec<String> {
    crate::dispatch_tts::voices()
}

#[tauri::command]
#[specta::specta]
pub fn change_read_aloud_voice_setting(app: AppHandle, voice: String) -> Result<(), String> {
    if !crate::dispatch_tts::is_voice(&voice) {
        return Err(format!("Unknown voice: {voice}"));
    }
    let mut settings = get_settings(&app);
    settings.dispatch_read_aloud_voice = voice;
    write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_read_aloud_speed_setting(app: AppHandle, speed: f32) -> Result<(), String> {
    let range = crate::dispatch_tts::SPEED_RANGE;
    if !speed.is_finite() || !range.contains(&speed) {
        return Err(format!(
            "Speed must be between {} and {}",
            range.start(),
            range.end()
        ));
    }
    let mut settings = get_settings(&app);
    settings.dispatch_read_aloud_speed = speed;
    write_settings(&app, settings);
    Ok(())
}

/// Plays a short sample of a voice at the current speed.
#[tauri::command]
#[specta::specta]
pub fn preview_read_aloud_voice(app: AppHandle, voice: String) -> Result<(), String> {
    if !crate::dispatch_tts::is_voice(&voice) {
        return Err(format!("Unknown voice: {voice}"));
    }
    crate::dispatch_tts::preview(&app, &voice);
    Ok(())
}

/// Reads text pasted into the Read Aloud page, stopping anything being read.
#[tauri::command]
#[specta::specta]
pub fn read_text_aloud(app: AppHandle, text: String) -> Result<(), String> {
    if text.chars().count() > crate::dispatch_tts::MAX_PASTED_CHARS {
        return Err(format!(
            "Read Aloud takes up to {} characters at a time",
            crate::dispatch_tts::MAX_PASTED_CHARS
        ));
    }
    crate::dispatch_tts::read_text(&app, &text);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_read_back_setting(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.dispatch_read_back = enabled;
    write_settings(&app, settings);
    Ok(())
}

/// Whether George's voice files are downloaded, and any download's progress.
#[tauri::command]
#[specta::specta]
pub fn get_voice_files_status(app: AppHandle) -> crate::dispatch_voice_files::VoiceFilesStatus {
    crate::dispatch_voice_files::status(&app)
}

/// Starts downloading George's voice files (about 340 MB) in the background.
#[tauri::command]
#[specta::specta]
pub fn download_voice_files(app: AppHandle) {
    crate::dispatch_voice_files::start_download(&app);
}

#[tauri::command]
#[specta::specta]
pub fn cancel_voice_files_download() {
    crate::dispatch_voice_files::cancel_download();
}

/// The pill's "Set up" button: opens the Read Aloud page.
#[tauri::command]
#[specta::specta]
pub fn open_read_aloud_page() {
    crate::dispatch_reading_pill::open_read_aloud_page();
}

/// The read-aloud pill's Stop button.
#[tauri::command]
#[specta::specta]
pub fn stop_read_aloud() {
    crate::dispatch_tts::stop();
}

/// What the read-aloud pill shows now (the pill's page asks when it loads).
#[tauri::command]
#[specta::specta]
pub fn get_reading_pill_state() -> crate::dispatch_reading_pill::PillState {
    crate::dispatch_reading_pill::current()
}

#[tauri::command]
#[specta::specta]
pub fn change_fix_rules_setting(
    app: AppHandle,
    rules: Vec<crate::dispatch_fixes::FixRule>,
) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.dispatch_fixes = crate::dispatch_fixes::clean_rules(rules)?;
    write_settings(&app, settings);
    crate::dispatch_achievements::refresh(); // Fixer
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_smart_spacing_setting(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.dispatch_smart_spacing = enabled;
    write_settings(&app, settings);
    Ok(())
}

/// What the cleanup setup card in Settings shows (dispatch_cleanup_setup.rs).
#[tauri::command]
#[specta::specta]
pub fn get_cleanup_setup(app: AppHandle) -> crate::dispatch_cleanup_setup::CleanupSetup {
    crate::dispatch_cleanup_setup::status(&app)
}

/// The card's "Check again": asks Ollama now, and switches cleanup back on
/// if it was waiting for Ollama or its model.
#[tauri::command]
#[specta::specta]
pub async fn check_cleanup_setup(app: AppHandle) -> crate::dispatch_cleanup_setup::CleanupSetup {
    crate::dispatch_cleanup_setup::check(&app).await
}

#[tauri::command]
#[specta::specta]
pub fn change_clean_up_dictation_setting(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.dispatch_clean_up_dictation = enabled;
    write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_post_process_timeout_setting(app: AppHandle, seconds: f64) -> Result<(), String> {
    if !seconds.is_finite() || !POST_PROCESS_TIMEOUT_RANGE_SECS.contains(&seconds) {
        return Err(format!(
            "Timeout must be between {} and {} seconds",
            POST_PROCESS_TIMEOUT_RANGE_SECS.start(),
            POST_PROCESS_TIMEOUT_RANGE_SECS.end()
        ));
    }
    let mut settings = get_settings(&app);
    settings.post_process_timeout_secs = seconds;
    write_settings(&app, settings);
    Ok(())
}

/// A dictation to summarize for the review window's list.
#[derive(serde::Deserialize, specta::Type)]
pub struct SummaryRequest {
    pub file_name: String,
    pub heard_text: String,
}

/// The review window's view of one dictation: waveform, timed words, fixes.
#[tauri::command]
#[specta::specta]
pub async fn get_dictation_review(
    app: AppHandle,
    file_name: String,
    heard_text: String,
    cleaned_text: Option<String>,
) -> Result<crate::dispatch_review::DictationReview, String> {
    tokio::task::spawn_blocking(move || {
        crate::dispatch_review::review(&app, &file_name, &heard_text, cleaned_text.as_deref())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// The review window's "Quiet background": a copy of the recording with the
/// background noise quieted, made the first time (about a second).
#[tauri::command]
#[specta::specta]
pub async fn get_quiet_dictation_audio(
    app: AppHandle,
    file_name: String,
) -> Result<crate::dispatch_review::QuietCopy, String> {
    tokio::task::spawn_blocking(move || crate::dispatch_review::quiet_copy(&app, &file_name))
        .await
        .map_err(|e| e.to_string())?
}

/// The review window's "Looks right": marks a dictation checked (its text is
/// what was said), or takes the mark off.
#[tauri::command]
#[specta::specta]
pub async fn set_dictation_checked(
    app: AppHandle,
    file_name: String,
    heard_text: String,
    cleaned_text: Option<String>,
    checked: bool,
) -> Result<crate::dispatch_review::DictationReview, String> {
    tokio::task::spawn_blocking(move || {
        crate::dispatch_review::set_checked(
            &app,
            &file_name,
            &heard_text,
            cleaned_text.as_deref(),
            checked,
        )
    })
    .await
    .map_err(|e| e.to_string())?
}

/// What switching to "keep only the newest recordings" would move to the
/// Recycle Bin now, so the page can ask first.
#[tauri::command]
#[specta::specta]
pub async fn get_audio_cleanup_preview(
    app: AppHandle,
) -> Result<crate::dispatch_audio::AudioCleanup, String> {
    tokio::task::spawn_blocking(move || crate::dispatch_audio::cleanup_preview(&app))
        .await
        .map_err(|e| e.to_string())?
}

/// Keep every recording, or only the newest (older audio goes to the Recycle
/// Bin; transcripts stay).
#[tauri::command]
#[specta::specta]
pub async fn change_keep_all_audio_setting(app: AppHandle, keep_all: bool) -> Result<(), String> {
    tokio::task::spawn_blocking(move || crate::dispatch_audio::set_keep_all(&app, keep_all))
        .await
        .map_err(|e| e.to_string())
}

/// How many dictations History holds in all, for the count by its title.
#[tauri::command]
#[specta::specta]
pub async fn get_dictation_count(app: AppHandle) -> Result<u64, String> {
    tokio::task::spawn_blocking(move || crate::dispatch_review::dictation_count(&app))
        .await
        .map_err(|e| e.to_string())?
}

/// Length, a little waveform and the fix count for each listed dictation.
#[tauri::command]
#[specta::specta]
pub async fn get_dictation_summaries(
    app: AppHandle,
    entries: Vec<SummaryRequest>,
) -> Result<Vec<crate::dispatch_review::DictationSummary>, String> {
    tokio::task::spawn_blocking(move || {
        let entries: Vec<(String, String)> = entries
            .into_iter()
            .map(|e| (e.file_name, e.heard_text))
            .collect();
        crate::dispatch_review::summaries(&app, &entries)
    })
    .await
    .map_err(|e| e.to_string())
}

/// Replaces the heard words `first..=last` with `meant` and returns the
/// updated review. With `add_rule`, also adds it to the Fix-It List.
#[tauri::command]
#[specta::specta]
#[allow(clippy::too_many_arguments)]
pub async fn save_dictation_fix(
    app: AppHandle,
    file_name: String,
    heard_text: String,
    cleaned_text: Option<String>,
    first: u32,
    last: u32,
    meant: String,
    add_rule: bool,
) -> Result<crate::dispatch_review::DictationReview, String> {
    tokio::task::spawn_blocking(move || {
        crate::dispatch_review::save_fix(
            &app,
            &file_name,
            &heard_text,
            cleaned_text.as_deref(),
            first as usize,
            last as usize,
            &meant,
            add_rule,
        )
        .inspect(|_| crate::dispatch_achievements::refresh()) // Course Correction, Fixer
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Everything the Achievements page shows.
#[tauri::command]
#[specta::specta]
pub async fn get_achievements(
    app: AppHandle,
) -> Result<crate::dispatch_achievements::AchievementsStatus, String> {
    tokio::task::spawn_blocking(move || crate::dispatch_achievements::status(&app))
        .await
        .map_err(|e| e.to_string())?
}

/// Marks achievements' unlock pop-ups as shown.
#[tauri::command]
#[specta::specta]
pub async fn mark_achievements_seen(app: AppHandle, ids: Vec<String>) -> Result<(), String> {
    // Off the main thread: it waits for the achievements' lock.
    tokio::task::spawn_blocking(move || crate::dispatch_achievements::mark_seen(&app, &ids))
        .await
        .map_err(|e| e.to_string())?
}

/// Opening History: grows the main window to fit the review window's three
/// columns, if the screen has room. Never shrinks it, and does nothing when
/// it's maximized or already big enough.
#[tauri::command]
#[specta::specta]
pub fn fit_review_window(app: AppHandle) -> Result<(), String> {
    use tauri::Manager;
    const WANT: (f64, f64) = (1280.0, 820.0);
    let Some(window) = app.get_webview_window("main") else {
        return Ok(());
    };
    if window.is_maximized().unwrap_or(false) {
        return Ok(());
    }
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    let size = window
        .inner_size()
        .map_err(|e| e.to_string())?
        .to_logical::<f64>(scale);
    let Some(monitor) = window.current_monitor().map_err(|e| e.to_string())? else {
        return Ok(());
    };
    let screen = monitor
        .work_area()
        .size
        .to_logical::<f64>(monitor.scale_factor());
    let width = WANT.0.min(screen.width - 40.0).max(size.width);
    let height = WANT.1.min(screen.height - 40.0).max(size.height);
    if width <= size.width + 1.0 && height <= size.height + 1.0 {
        return Ok(());
    }
    window
        .set_size(tauri::LogicalSize::new(width, height))
        .map_err(|e| e.to_string())?;
    window.center().map_err(|e| e.to_string())
}

/// The main window's page has loaded: the splash screen can make way for it
/// (dispatch_splash.rs).
#[tauri::command]
#[specta::specta]
pub fn main_window_ready() {
    crate::dispatch_splash::main_window_ready();
}

//! Dispatch: the cleanup setup check (HANDOFF.md, "Cleanup setup check").
//!
//! AI cleanup is on by default and runs on Ollama, which most new users
//! won't have. Without this, every dictation would try Ollama, fail, and
//! raise a "Transcribed Without Cleanup" warning. So on a new install,
//! Dispatch asks Ollama which models it has: if Ollama or the cleanup model
//! is missing, Clean Up Dictation is switched off quietly and Settings shows
//! how to set it up. Once they're found (at the next start, or from the
//! card's "Check again"), cleanup switches itself back on.
//!
//! Only until cleanup has reached its model once (`dispatch_cleanup_verified`).
//! After that, and on every install that predates this check (defaults
//! version 10), nothing here changes a setting: if Ollama is down, the
//! warning is the right answer.

use std::sync::Mutex;
use std::time::Duration;

use log::{debug, info};
use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, Emitter};

use crate::settings::{get_settings, write_settings, AppSettings};

/// Sent with a [`CleanupSetup`] whenever a check finishes.
pub const EVENT: &str = "dispatch-cleanup-setup";

/// Ollama answers its model list in milliseconds when it's running.
const TIMEOUT: Duration = Duration::from_secs(2);
/// Ollama can start after Dispatch when Windows signs in, so a failed check
/// at startup is tried again after these waits.
const RETRY_WAITS: [Duration; 3] = [
    Duration::from_secs(5),
    Duration::from_secs(15),
    Duration::from_secs(40),
];

/// What's missing for cleanup.
#[derive(Serialize, Type, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CleanupProblem {
    /// Ollama didn't answer: not installed, or not running.
    NoOllama,
    /// Ollama is running, but the cleanup model isn't downloaded.
    NoModel,
}

/// What the Settings card shows.
#[derive(Serialize, Type, Clone, Debug, PartialEq)]
pub struct CleanupSetup {
    /// Show the setup card.
    pub show: bool,
    pub problem: Option<CleanupProblem>,
    /// The model to download, such as "qwen3.5:4b".
    pub model: String,
}

/// The last check's result: `None` before any, `Some(None)` when cleanup is
/// ready, `Some(Some(problem))` otherwise.
static LAST: Mutex<Option<Option<CleanupProblem>>> = Mutex::new(None);

/// At startup: checks a new install's cleanup in the background, trying
/// again for about a minute in case Ollama starts after Dispatch.
pub fn start(app: &AppHandle) {
    let settings = get_settings(app);
    if settings.dispatch_cleanup_verified
        || crate::dispatch::local_cleanup_target(&settings).is_none()
    {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        for wait in std::iter::once(Duration::ZERO).chain(RETRY_WAITS) {
            tokio::time::sleep(wait).await;
            if check(&app).await.problem.is_none() {
                return;
            }
        }
    });
}

/// Checks now and applies the result: the card's "Check again".
pub async fn check(app: &AppHandle) -> CleanupSetup {
    let settings = get_settings(app);
    let Some((root, model)) = crate::dispatch::local_cleanup_target(&settings) else {
        return status(app);
    };
    let problem = probe(&root, &model).await;
    debug!("Cleanup setup check: {problem:?}");
    if let Ok(mut last) = LAST.lock() {
        *last = Some(problem);
    }
    let mut settings = get_settings(app);
    if apply(&mut settings, problem) {
        write_settings(app, settings);
    }
    let status = status(app);
    let _ = app.emit(EVENT, &status);
    status
}

/// What the card shows now, from the last check.
pub fn status(app: &AppHandle) -> CleanupSetup {
    let settings = get_settings(app);
    let target = crate::dispatch::local_cleanup_target(&settings);
    let problem = LAST.lock().ok().and_then(|last| last.flatten());
    CleanupSetup {
        show: !settings.dispatch_cleanup_verified && target.is_some() && problem.is_some(),
        problem,
        model: target
            .map(|(_, model)| model)
            .unwrap_or_else(|| crate::dispatch::DEFAULT_CLEANUP_MODEL.to_string()),
    }
}

/// Updates the settings for a check's result. True if anything changed.
fn apply(settings: &mut AppSettings, problem: Option<CleanupProblem>) -> bool {
    match problem {
        None => {
            let changed = !settings.dispatch_cleanup_verified || settings.dispatch_cleanup_paused;
            settings.dispatch_cleanup_verified = true;
            if settings.dispatch_cleanup_paused {
                info!("Cleanup setup: Ollama and its model found, cleanup back on");
                settings.dispatch_cleanup_paused = false;
                settings.dispatch_clean_up_dictation = true;
            }
            changed
        }
        Some(problem) => {
            if settings.dispatch_cleanup_verified || !settings.dispatch_clean_up_dictation {
                return false;
            }
            info!("Cleanup setup: {problem:?}, cleanup off until it's set up");
            settings.dispatch_clean_up_dictation = false;
            settings.dispatch_cleanup_paused = true;
            true
        }
    }
}

/// Asks Ollama at `root` for its models: `None` when `model` is there.
async fn probe(root: &str, model: &str) -> Option<CleanupProblem> {
    let response = reqwest::Client::new()
        .get(format!("{root}/api/tags"))
        .timeout(TIMEOUT)
        .send()
        .await
        .and_then(|response| response.error_for_status());
    let Ok(response) = response else {
        return Some(CleanupProblem::NoOllama);
    };
    let Ok(body) = response.json::<serde_json::Value>().await else {
        return Some(CleanupProblem::NoOllama);
    };
    let names: Vec<&str> = body["models"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| m["name"].as_str().or_else(|| m["model"].as_str()))
        .collect();
    if has_model(&names, model) {
        None
    } else {
        Some(CleanupProblem::NoModel)
    }
}

/// Whether Ollama's model list has `model`. A name without a tag means
/// `:latest`, as in Ollama.
fn has_model(names: &[&str], model: &str) -> bool {
    let want = model.trim().to_lowercase();
    let want = if want.contains(':') {
        want
    } else {
        format!("{want}:latest")
    };
    names.iter().any(|name| {
        let name = name.trim().to_lowercase();
        name == want || (!name.contains(':') && format!("{name}:latest") == want)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::get_default_settings;

    fn new_install() -> AppSettings {
        let mut settings = get_default_settings();
        crate::dispatch::apply_dispatch_defaults(&mut settings);
        settings
    }

    #[test]
    fn a_new_install_without_ollama_pauses_cleanup_then_resumes() {
        let mut settings = new_install();
        assert!(settings.dispatch_clean_up_dictation);

        assert!(apply(&mut settings, Some(CleanupProblem::NoOllama)));
        assert!(!settings.dispatch_clean_up_dictation);
        assert!(settings.dispatch_cleanup_paused);
        // Checking again changes nothing more.
        assert!(!apply(&mut settings, Some(CleanupProblem::NoModel)));

        assert!(apply(&mut settings, None));
        assert!(settings.dispatch_clean_up_dictation);
        assert!(!settings.dispatch_cleanup_paused);
        assert!(settings.dispatch_cleanup_verified);
    }

    #[test]
    fn never_switches_off_cleanup_that_has_worked() {
        let mut settings = new_install();
        settings.dispatch_cleanup_verified = true;
        assert!(!apply(&mut settings, Some(CleanupProblem::NoOllama)));
        assert!(settings.dispatch_clean_up_dictation);
    }

    #[test]
    fn leaves_cleanup_off_if_the_user_turned_it_off() {
        let mut settings = new_install();
        settings.dispatch_clean_up_dictation = false;
        assert!(!apply(&mut settings, Some(CleanupProblem::NoOllama)));
        assert!(!settings.dispatch_cleanup_paused);
        // Found later: verified, but still off, as the user chose.
        apply(&mut settings, None);
        assert!(!settings.dispatch_clean_up_dictation);
    }

    #[test]
    fn matches_model_names_like_ollama() {
        let names = ["qwen3.5:4b", "gemma3:4b", "llama3:latest"];
        assert!(has_model(&names, "qwen3.5:4b"));
        assert!(has_model(&names, "QWEN3.5:4B"));
        assert!(has_model(&names, "llama3"));
        assert!(!has_model(&names, "qwen3.5:9b"));
        assert!(!has_model(&names, "qwen3.5"));
        assert!(!has_model(&[], "qwen3.5:4b"));
    }
}

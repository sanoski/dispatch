//! Dispatch: the splash screen (ARTWORK.md, "Splash screen").
//!
//! When Dispatch is opened by hand, a small borderless window with Joshua's
//! art shows while the app loads, then the main window takes its place. It
//! never shows when Dispatch starts hidden with Windows, or when the main
//! window is brought back later (the tray, a second launch): only on the
//! startup path that would otherwise show the main window straight away.
//!
//! The main window is made hidden and loads its page behind the splash. The
//! page calls the `main_window_ready` command once it's drawn; the splash
//! stays at least [`SHORTEST`] so it doesn't flicker, and never longer than
//! [`LONGEST`] even if that call never comes.
//!
//! The page is `src/dispatch-splash/`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use log::{debug, warn};
use tauri::{AppHandle, Manager, WebviewWindowBuilder};

pub const LABEL: &str = "dispatch_splash";

/// The art is 16:9; logical pixels.
const WIDTH: f64 = 560.0;
const HEIGHT: f64 = 315.0;
/// Long enough to see, short enough not to be in the way.
const SHORTEST: Duration = Duration::from_millis(1200);
/// If the main window never says it's ready, show it anyway.
const LONGEST: Duration = Duration::from_secs(8);
const POLL: Duration = Duration::from_millis(40);

static MAIN_READY: AtomicBool = AtomicBool::new(false);

/// The main window's page has loaded (the `main_window_ready` command).
pub fn main_window_ready() {
    MAIN_READY.store(true, Ordering::SeqCst);
}

/// Shows the splash, then `show_main` (Handy's own way of showing the main
/// window) once the main window is ready. Without a splash window, for any
/// reason, the main window shows straight away.
pub fn show_main_window(app: &AppHandle, show_main: fn(&AppHandle)) {
    if !open(app) {
        show_main(app);
        return;
    }
    let app = app.clone();
    let started = Instant::now();
    std::thread::spawn(move || {
        while !done_waiting(started.elapsed(), MAIN_READY.load(Ordering::SeqCst)) {
            std::thread::sleep(POLL);
        }
        let handle = app.clone();
        let swapped = app.run_on_main_thread(move || {
            show_main(&handle);
            close(&handle);
        });
        if let Err(e) = swapped {
            warn!("Splash: couldn't switch to the main window: {e}");
        }
    });
}

/// Whether the splash has been up long enough.
fn done_waiting(elapsed: Duration, main_ready: bool) -> bool {
    elapsed >= LONGEST || (main_ready && elapsed >= SHORTEST)
}

/// Makes and shows the splash window, centered. False if it couldn't.
fn open(app: &AppHandle) -> bool {
    let mut builder = WebviewWindowBuilder::new(
        app,
        LABEL,
        tauri::WebviewUrl::App("src/dispatch-splash/index.html".into()),
    )
    .title("Dispatch")
    .inner_size(WIDTH, HEIGHT)
    .center()
    .resizable(false)
    .maximizable(false)
    .minimizable(false)
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .skip_taskbar(true)
    .visible(true);
    if let Some(data_dir) = crate::portable::data_dir() {
        builder = builder.data_directory(data_dir.join("webview"));
    }
    match builder.build() {
        Ok(_) => {
            debug!("Splash shown");
            true
        }
        Err(e) => {
            warn!("Splash: couldn't create its window: {e}");
            false
        }
    }
}

/// Removes the splash window for good. `destroy`, not `close`: Handy turns
/// every close into a hide (lib.rs, `on_window_event`).
fn close(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(LABEL) {
        if let Err(e) = window.destroy() {
            warn!("Splash: couldn't close its window: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waits_for_the_main_window_but_not_forever() {
        // Ready early: still shows for the shortest time.
        assert!(!done_waiting(Duration::from_millis(300), true));
        assert!(done_waiting(SHORTEST, true));
        // Not ready: keeps waiting, up to the longest time.
        assert!(!done_waiting(Duration::from_secs(3), false));
        assert!(done_waiting(LONGEST, false));
    }
}

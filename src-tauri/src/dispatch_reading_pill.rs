//! Dispatch: the read-aloud pill, a small sign at the bottom of the screen
//! while George reads, with a Stop button (HANDOFF.md, "Read aloud").
//!
//! It's its own always-on-top window (`dispatch_reading`), made hidden at
//! startup like Handy's recording overlay, so Handy's overlay code stays
//! untouched. It never takes focus: the text being read stays selected in
//! the app it came from. It sits above where Handy's recording pill goes,
//! so dictating while George reads doesn't stack the two.
//!
//! `dispatch_tts` calls [`show`] and [`hide`]; the page
//! (`src/dispatch-reading/`) follows [`EVENT`] and asks for the current
//! state when it loads. The Read Aloud page in the main window follows the
//! same event for its Play and Stop button.

use std::sync::{Mutex, OnceLock};

use log::{debug, warn};
use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, Emitter, Manager, WebviewWindowBuilder};

pub const LABEL: &str = "dispatch_reading";
/// The event the page listens for, with a [`PillState`].
pub const EVENT: &str = "dispatch-reading-pill";

/// Window size in logical pixels. The pill itself is centered inside, with
/// room around it for its shadow.
const WIDTH: f64 = 440.0;
const HEIGHT: f64 = 76.0;
/// Gap between the window's bottom edge and the screen's, clear of Handy's
/// recording pill (40 px up, 50 px tall).
const BOTTOM_GAP: f64 = 96.0;

/// What the pill shows.
#[derive(Serialize, Clone, Debug, PartialEq, Type)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum PillState {
    Hidden,
    /// Kokoro is loading (the first read after starting Dispatch).
    Warming {
        voice: String,
    },
    /// Speaking the selection.
    Reading {
        voice: String,
    },
    /// George is answering an empty selection.
    NothingSelected,
    /// George's voice files aren't downloaded yet; the pill offers to.
    NeedsVoice,
}

/// Asks the main window to switch to a sidebar page (App.tsx listens).
pub const OPEN_PAGE_EVENT: &str = "dispatch-open-page";

static APP: OnceLock<AppHandle> = OnceLock::new();
static CURRENT: Mutex<PillState> = Mutex::new(PillState::Hidden);

/// Makes the pill's window, hidden. Called once at startup.
pub fn create(app: &AppHandle) {
    if APP.set(app.clone()).is_err() {
        return;
    }
    let mut builder = WebviewWindowBuilder::new(
        app,
        LABEL,
        tauri::WebviewUrl::App("src/dispatch-reading/index.html".into()),
    )
    .title("Reading aloud")
    .inner_size(WIDTH, HEIGHT)
    .resizable(false)
    .shadow(false)
    .maximizable(false)
    .minimizable(false)
    .closable(false)
    .accept_first_mouse(true)
    .decorations(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .transparent(true)
    .focusable(false)
    .focused(false)
    .visible(false);
    if let Some(data_dir) = crate::portable::data_dir() {
        builder = builder.data_directory(data_dir.join("webview"));
    }
    match builder.build() {
        Ok(_) => debug!("Read-aloud pill window created (hidden)"),
        Err(e) => warn!("Read-aloud pill: couldn't create its window: {e}"),
    }
}

/// The pill's state now, for the page when it loads.
pub fn current() -> PillState {
    CURRENT
        .lock()
        .map(|s| s.clone())
        .unwrap_or(PillState::Hidden)
}

/// Shows the pill in `state`. Placed on the screen with the mouse when it
/// appears; a change of state while it's up leaves it where it is.
pub fn show(state: PillState) {
    let Some(app) = APP.get() else {
        return;
    };
    let was_hidden = match CURRENT.lock() {
        Ok(mut current) => {
            let was_hidden = *current == PillState::Hidden;
            *current = state.clone();
            was_hidden
        }
        Err(_) => return,
    };
    // To every window: the Read Aloud page follows it too.
    let _ = app.emit(EVENT, &state);
    if was_hidden {
        place_and_show(app);
    }
}

/// Shows the pill in `state` for `duration`, then hides it unless something
/// else has shown since.
pub fn show_for(state: PillState, duration: std::time::Duration) {
    show(state.clone());
    std::thread::spawn(move || {
        std::thread::sleep(duration);
        if current() == state {
            hide();
        }
    });
}

/// The pill's "Set up" button: brings the main window up on the Read Aloud
/// page, where the voice can be downloaded.
pub fn open_read_aloud_page() {
    let Some(app) = APP.get() else {
        return;
    };
    hide();
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
    let _ = app.emit(OPEN_PAGE_EVENT, "readaloud");
}

/// Hides the pill.
pub fn hide() {
    let Some(app) = APP.get() else {
        return;
    };
    match CURRENT.lock() {
        Ok(mut current) if *current != PillState::Hidden => *current = PillState::Hidden,
        _ => return,
    }
    let _ = app.emit(EVENT, PillState::Hidden);
    let Some(window) = app.get_webview_window(LABEL) else {
        return;
    };
    // Hidden the same native way it's shown. Tauri's hide() did nothing
    // here: Tauri never saw the native show, thought the window was already
    // hidden, and left an empty, invisible window over the bottom of the
    // screen that swallowed clicks (found 2026-11-22: VS Code's prompt box
    // sometimes ignored clicks). Queued on the main thread after any show.
    #[cfg(target_os = "windows")]
    {
        let target = window.clone();
        let _ = window.run_on_main_thread(move || {
            use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE};
            if let Ok(hwnd) = target.hwnd() {
                unsafe {
                    let _ = ShowWindow(hwnd, SW_HIDE);
                }
            }
        });
    }
    #[cfg(not(target_os = "windows"))]
    let _ = window.hide();
}

/// Bottom center of the screen with the mouse on it, shown without taking
/// focus.
fn place_and_show(app: &AppHandle) {
    let Some(window) = app.get_webview_window(LABEL) else {
        return;
    };
    let Some((x, y, width, height)) = bounds(app) else {
        let _ = window.show();
        return;
    };
    #[cfg(target_os = "windows")]
    {
        let target = window.clone();
        let _ = window.run_on_main_thread(move || {
            use windows::Win32::UI::WindowsAndMessaging::{
                SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE, SWP_SHOWWINDOW,
            };
            if let Ok(hwnd) = target.hwnd() {
                // One native call: placed, sized, topmost and shown, without
                // activating it (Tauri's show() would take focus).
                unsafe {
                    let _ = SetWindowPos(
                        hwnd,
                        Some(HWND_TOPMOST),
                        x,
                        y,
                        width,
                        height,
                        SWP_NOACTIVATE | SWP_SHOWWINDOW,
                    );
                }
            }
        });
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
        let _ = window.set_size(tauri::PhysicalSize::new(width as u32, height as u32));
        let _ = window.show();
    }
}

/// The window's rectangle in physical pixels on the monitor with the mouse
/// (or the main monitor).
fn bounds(app: &AppHandle) -> Option<(i32, i32, i32, i32)> {
    let cursor = app.cursor_position().ok();
    let monitors = app.available_monitors().ok()?;
    let monitor = cursor
        .and_then(|c| {
            monitors.iter().find(|m| {
                let (p, s) = (m.position(), m.size());
                c.x >= p.x as f64
                    && c.x < p.x as f64 + s.width as f64
                    && c.y >= p.y as f64
                    && c.y < p.y as f64 + s.height as f64
            })
        })
        .cloned()
        .or_else(|| app.primary_monitor().ok().flatten())?;
    let scale = monitor.scale_factor();
    // WebView2 zooms the page with Windows' text size, so the window grows
    // with it to keep the pill whole.
    let content = scale * text_scale();
    let width = (WIDTH * content).round() as i32;
    let height = (HEIGHT * content).round() as i32;
    let (p, s) = (monitor.position(), monitor.size());
    let x = p.x + (s.width as i32 - width) / 2;
    let y = p.y + s.height as i32 - height - (BOTTOM_GAP * scale).round() as i32;
    Some((x, y, width, height))
}

/// Windows' accessibility text size (Settings > Accessibility > Text size).
#[cfg(target_os = "windows")]
fn text_scale() -> f64 {
    winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
        .open_subkey(r"Software\Microsoft\Accessibility")
        .and_then(|key| key.get_value::<u32, _>("TextScaleFactor"))
        .map(|percent| (percent as f64 / 100.0).clamp(1.0, 2.25))
        .unwrap_or(1.0)
}

#[cfg(not(target_os = "windows"))]
fn text_scale() -> f64 {
    1.0
}

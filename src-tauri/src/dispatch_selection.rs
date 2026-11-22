//! Dispatch: the text selected in whatever app has focus, for read aloud
//! (see `dispatch_tts` and HANDOFF.md, "Read aloud").
//!
//! First it asks Windows UI Automation for the selection, which doesn't touch
//! the clipboard (the same system smart spacing uses). Apps that don't report
//! their text that way get a Ctrl+C instead: the selection is read from the
//! clipboard, and the text that was on the clipboard before is put back.

use tauri::AppHandle;

/// The most text read from one selection, in UTF-16 units (UI Automation's).
#[cfg_attr(not(windows), allow(dead_code))]
const MAX_CHARS: i32 = 100_000;

/// The selected text, trimmed, or `None` when nothing is selected or the app
/// won't say.
pub fn selected_text(app: &AppHandle) -> Option<String> {
    #[cfg(windows)]
    {
        if let Some(text) = windows_impl::from_ui_automation() {
            log::debug!("Read aloud: got the selection from UI Automation");
            return Some(text);
        }
        let text = windows_impl::by_copying(app);
        if text.is_some() {
            log::debug!("Read aloud: got the selection by copying it");
        }
        text
    }
    #[cfg(not(windows))]
    {
        let _ = app;
        None
    }
}

fn non_empty(text: String) -> Option<String> {
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

#[cfg(windows)]
mod windows_impl {
    use super::{non_empty, MAX_CHARS};
    use std::time::{Duration, Instant};
    use tauri::AppHandle;
    use tauri_plugin_clipboard_manager::ClipboardExt;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
        COINIT_MULTITHREADED,
    };
    use windows::Win32::System::DataExchange::GetClipboardSequenceNumber;
    use windows::Win32::UI::Accessibility::{
        CUIAutomation, IUIAutomation, IUIAutomationTextPattern, UIA_TextPatternId,
    };
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS,
        KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
    };

    /// How long an app gets to answer UI Automation. Some apps never do.
    const UIA_TIMEOUT: Duration = Duration::from_millis(500);
    /// How long to wait for the hotkey's modifier keys to be let go, so the
    /// Ctrl+C doesn't arrive as Ctrl+Alt+C.
    const MODIFIER_RELEASE_TIMEOUT: Duration = Duration::from_millis(1000);
    /// How long an app gets to put the selection on the clipboard.
    const COPY_TIMEOUT: Duration = Duration::from_millis(500);
    const VK_C: VIRTUAL_KEY = VIRTUAL_KEY(0x43);

    pub fn from_ui_automation() -> Option<String> {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            // SAFETY: COM is initialized on this thread for the duration of
            // the lookup and uninitialized after; the UIA objects are dropped
            // inside `lookup`, before that.
            let text = unsafe {
                let initialized = CoInitializeEx(None, COINIT_MULTITHREADED).is_ok();
                let text = lookup();
                if initialized {
                    CoUninitialize();
                }
                text
            };
            let _ = tx.send(text);
        });
        rx.recv_timeout(UIA_TIMEOUT).ok().flatten()
    }

    unsafe fn lookup() -> Option<String> {
        let automation: IUIAutomation =
            CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok()?;
        let focused = automation.GetFocusedElement().ok()?;
        let pattern: IUIAutomationTextPattern =
            focused.GetCurrentPatternAs(UIA_TextPatternId).ok()?;
        let selection = pattern.GetSelection().ok()?;
        let mut text = String::new();
        for i in 0..selection.Length().ok()? {
            let range = selection.GetElement(i).ok()?;
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(&range.GetText(MAX_CHARS).ok()?.to_string());
        }
        non_empty(text)
    }

    pub fn by_copying(app: &AppHandle) -> Option<String> {
        wait_for_modifiers_up();
        let clipboard = app.clipboard();
        let saved = clipboard.read_text().ok();
        // SAFETY: plain Win32 calls with no pointers held past the call.
        let before = unsafe { GetClipboardSequenceNumber() };
        send_ctrl_c();

        let deadline = Instant::now() + COPY_TIMEOUT;
        let mut copied = false;
        while Instant::now() < deadline {
            if unsafe { GetClipboardSequenceNumber() } != before {
                copied = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(15));
        }
        if !copied {
            // Nothing was selected, or the app ignores Ctrl+C.
            return None;
        }
        // Some apps write the clipboard in several steps.
        std::thread::sleep(Duration::from_millis(40));
        let text = clipboard.read_text().ok();
        // Put the old text back. If the clipboard held something other than
        // text (an image), it can't be restored, so the copy stays.
        if let Some(saved) = saved {
            if let Err(e) = clipboard.write_text(saved) {
                log::warn!("Read aloud: couldn't restore the clipboard: {e}");
            }
        }
        text.and_then(non_empty)
    }

    fn wait_for_modifiers_up() {
        let deadline = Instant::now() + MODIFIER_RELEASE_TIMEOUT;
        let held = || {
            [VK_CONTROL, VK_MENU, VK_SHIFT, VK_LWIN, VK_RWIN]
                .iter()
                // SAFETY: reads key state; no pointers involved.
                .any(|vk| unsafe { GetAsyncKeyState(vk.0 as i32) } as u16 & 0x8000 != 0)
        };
        while held() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(15));
        }
    }

    fn key(vk: VIRTUAL_KEY, up: bool) -> INPUT {
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: if up {
                        KEYEVENTF_KEYUP
                    } else {
                        KEYBD_EVENT_FLAGS(0)
                    },
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }

    fn send_ctrl_c() {
        let inputs = [
            key(VK_CONTROL, false),
            key(VK_C, false),
            key(VK_C, true),
            key(VK_CONTROL, true),
        ];
        // SAFETY: `inputs` outlives the call and the size matches `INPUT`.
        let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
        if sent as usize != inputs.len() {
            log::warn!("Read aloud: Windows accepted only {sent} of the Ctrl+C key events");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::non_empty;

    #[test]
    fn blank_selections_count_as_nothing() {
        assert_eq!(non_empty("  \n\t ".into()), None);
        assert_eq!(
            non_empty("  Hello there.\n".into()).as_deref(),
            Some("Hello there.")
        );
    }
}

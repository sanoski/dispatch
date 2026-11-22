//! Dispatch: smart spacing between dictations.
//!
//! Dictating, stopping, then dictating again in the same spot pasted the new
//! text right against the old ("dictation?I'm starting"). Before pasting,
//! this asks Windows UI Automation (the accessibility interface screen
//! readers use) for the character just before the cursor, and puts a space in
//! front of the new text when it would otherwise butt against a word.
//!
//! When the app can't say (no accessibility support, or it doesn't answer
//! within `LOOKUP_TIMEOUT`), the text is pasted unchanged, as before.

use log::debug;
use std::time::Duration;

/// How long to wait for the focused app to report its text. A hung app must
/// not hold up the paste.
const LOOKUP_TIMEOUT: Duration = Duration::from_millis(250);

/// The cursor is at the start of the text box (nothing before it).
const START_OF_FIELD: char = '\n';

/// After these, the next word follows with no space: "(word", "“word".
const OPENERS: &[char] = &['(', '[', '{', '<', '“', '‘', '/'];

/// These attach to the word before them: "word.", "word)".
const ATTACHES_LEFT: &[char] = &[
    '.', ',', '!', '?', ';', ':', ')', ']', '}', '>', '”', '’', '%', '\'',
];

/// `text`, with a space in front when the cursor sits right after a word.
pub fn with_leading_space(text: String, enabled: bool) -> String {
    if !enabled || text.is_empty() {
        return text;
    }
    if needs_leading_space(char_before_caret(), &text) {
        format!(" {text}")
    } else {
        text
    }
}

fn needs_leading_space(before: Option<char>, text: &str) -> bool {
    let (Some(before), Some(first)) = (before, text.chars().next()) else {
        return false;
    };
    !before.is_whitespace()
        && !first.is_whitespace()
        && !OPENERS.contains(&before)
        && !ATTACHES_LEFT.contains(&first)
}

#[cfg(windows)]
fn char_before_caret() -> Option<char> {
    let (tx, rx) = std::sync::mpsc::channel();
    // Its own thread, so a slow or hung app can be abandoned at the timeout.
    std::thread::spawn(move || {
        let _ = tx.send(uia::char_before_caret());
    });
    let result = rx.recv_timeout(LOOKUP_TIMEOUT).ok().flatten();
    debug!(
        "Smart spacing: character before cursor {}",
        match result {
            None => "unknown",
            Some(START_OF_FIELD) => "none (start of field)",
            Some(c) if c.is_whitespace() => "is whitespace",
            Some(_) => "is text",
        }
    );
    result
}

#[cfg(not(windows))]
fn char_before_caret() -> Option<char> {
    None
}

#[cfg(windows)]
mod uia {
    use super::START_OF_FIELD;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
        COINIT_MULTITHREADED,
    };
    use windows::Win32::UI::Accessibility::{
        CUIAutomation, IUIAutomation, IUIAutomationTextPattern, TextPatternRangeEndpoint_End,
        TextPatternRangeEndpoint_Start, TextUnit_Character, UIA_TextPatternId,
    };

    pub fn char_before_caret() -> Option<char> {
        // SAFETY: COM is initialized on this thread for the duration of the
        // lookup and uninitialized after; the UIA objects are dropped first.
        unsafe {
            let initialized = CoInitializeEx(None, COINIT_MULTITHREADED).is_ok();
            let result = lookup();
            if initialized {
                CoUninitialize();
            }
            result
        }
    }

    unsafe fn lookup() -> Option<char> {
        let automation: IUIAutomation =
            CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok()?;
        let focused = automation.GetFocusedElement().ok()?;
        let text: IUIAutomationTextPattern = focused.GetCurrentPatternAs(UIA_TextPatternId).ok()?;
        let selection = text.GetSelection().ok()?;
        if selection.Length().ok()? < 1 {
            return None;
        }
        let range = selection.GetElement(0).ok()?;
        // Collapse to the start of the selection (the cursor, when nothing is
        // selected), then reach back one character.
        range
            .MoveEndpointByRange(
                TextPatternRangeEndpoint_End,
                &range,
                TextPatternRangeEndpoint_Start,
            )
            .ok()?;
        let moved = range
            .MoveEndpointByUnit(TextPatternRangeEndpoint_Start, TextUnit_Character, -1)
            .ok()?;
        if moved == 0 {
            return Some(START_OF_FIELD);
        }
        range.GetText(-1).ok()?.to_string().chars().last()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_a_space_after_a_word_or_sentence() {
        assert!(needs_leading_space(Some('n'), "I'm starting"));
        assert!(needs_leading_space(Some('?'), "I'm starting"));
        assert!(needs_leading_space(Some('.'), "Next sentence."));
        assert!(needs_leading_space(Some('"'), "and then"));
    }

    #[test]
    fn leaves_text_alone_when_no_space_is_needed() {
        assert!(!needs_leading_space(Some(' '), "word"));
        assert!(!needs_leading_space(Some('\n'), "word"));
        assert!(!needs_leading_space(Some(START_OF_FIELD), "word"));
        assert!(!needs_leading_space(Some('('), "word"));
        assert!(!needs_leading_space(Some('“'), "word"));
        assert!(!needs_leading_space(Some('d'), ", and more"));
        assert!(!needs_leading_space(Some('d'), "."));
        assert!(!needs_leading_space(Some('d'), " already spaced"));
    }

    #[test]
    fn unknown_cursor_means_no_change() {
        assert!(!needs_leading_space(None, "word"));
        assert_eq!(with_leading_space(String::new(), true), "");
        assert_eq!(with_leading_space("word".into(), false), "word");
    }
}

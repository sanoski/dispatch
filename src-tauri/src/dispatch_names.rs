//! Dispatch: makes sure AI cleanup never drops a name (HANDOFF.md, "AI
//! cleanup details").
//!
//! The small local cleanup model has twice swapped one name for another
//! ("the same API key Ellis uses" came back as "...Parakeet uses"), despite
//! a prompt rule against it. So every cleanup is checked here, in code: each
//! name in what the speech model heard must still be in the cleaned text. If
//! one is missing, Dispatch pastes what was heard instead (the Fix-It List
//! has already run on it).
//!
//! A name is a word with a capital letter that doesn't start a sentence, or
//! any of the user's custom words and Fix-It replacements, wherever they
//! appear. The check is loose on purpose: the name only has to appear
//! somewhere in the cleaned text, ignoring case, spaces and punctuation, so
//! "Eleven Labs" cleaned to "ElevenLabs" passes. New names the model adds
//! (fixing "quad coat" to "Claude Code") are fine; only dropped ones count.

/// Words that are capitalized but aren't names.
const NOT_NAMES: &[&str] = &["I", "I'm", "I've", "I'll", "I'd", "OK"];

/// Whether `cleaned` still has every name in `heard`. `known` are names
/// counted wherever they appear (custom words, Fix-It replacements).
pub fn keeps_names(heard: &str, cleaned: &str, known: &[String]) -> bool {
    let cleaned = squash(cleaned);
    names(heard, known)
        .iter()
        .all(|name| cleaned.contains(name))
}

/// The names in `text`, squashed (lowercase letters and digits only).
fn names(text: &str, known: &[String]) -> Vec<String> {
    let mut found = Vec::new();
    let mut sentence_start = true;
    for raw in text.split_whitespace() {
        let word = raw.trim_matches(|c: char| !c.is_alphanumeric());
        let word = word
            .strip_suffix("'s")
            .or_else(|| word.strip_suffix("\u{2019}s"))
            .unwrap_or(word);
        if !sentence_start && word.chars().any(char::is_uppercase) && !NOT_NAMES.contains(&word) {
            found.push(squash(word));
        }
        if !word.is_empty() {
            sentence_start = raw.ends_with(['.', '?', '!', ':']);
        }
    }
    let words = format!(" {} ", spaced(text));
    for name in known {
        let name_words = spaced(name);
        if !name_words.is_empty() && words.contains(&format!(" {name_words} ")) {
            found.push(squash(name));
        }
    }
    found.retain(|name| !name.is_empty());
    found
}

/// Lowercase letters and digits only: "Ellis's" and "ElevenLabs" style.
fn squash(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Lowercase words separated by single spaces, punctuation dropped.
fn spaced(text: &str) -> String {
    text.split(|c: char| !c.is_alphanumeric() && c != '\'')
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn known() -> Vec<String> {
        ["Claude Code", "Parakeet", "Ollama", "ellis"]
            .map(String::from)
            .to_vec()
    }

    #[test]
    fn catches_the_real_swaps() {
        // Both from 2026-11-22: Whisper heard right, cleanup swapped names.
        assert!(!keeps_names(
            "Just use the same API key Ellis uses.",
            "Just use the same API key Parakeet uses.",
            &known(),
        ));
        assert!(!keeps_names(
            "Also a strange occurrence just happened. For some reason, Dispatch insisted on swapping Ellis for Parakeet.",
            "Also, a strange occurrence just happened. For some reason, Dispatch insisted on swapping Parakeet for Claude Code.",
            &known(),
        ));
    }

    #[test]
    fn ordinary_cleanups_pass() {
        assert!(keeps_names(
            "um so I told Ellis, uh, to wait for the Parakeet pass",
            "So I told Ellis to wait for the Parakeet pass.",
            &known(),
        ));
        // Joined up, possessive, different case: still the same names.
        assert!(keeps_names(
            "I have credits on Eleven Labs and Ellis's key",
            "I have credits on ElevenLabs and ellis's key.",
            &known(),
        ));
        // Capitalized only because it starts a sentence.
        assert!(keeps_names(
            "So that works. Then we ship it.",
            "That works, then we ship it.",
            &known(),
        ));
        // A name the model adds is fine.
        assert!(keeps_names(
            "open up quad coat",
            "Open up Claude Code.",
            &known(),
        ));
    }

    #[test]
    fn known_names_count_anywhere() {
        // "ellis" (from the Fix-It List) is lowercase and starts the text,
        // but it's still a name.
        assert!(!keeps_names(
            "ellis reads it back",
            "Parakeet reads it back.",
            &known(),
        ));
        assert!(keeps_names("ollama is up", "Ollama is up.", &known()));
    }
}

//! Dispatch's fix-it list: exact "heard this, meant that" replacements.
//!
//! Some mishearings are the same every time ("quad code" for "Claude Code").
//! Fuzzy custom words can't catch them (the words are too different), and the
//! cleanup model only catches them when cleanup is on. These rules run on
//! every transcript, before custom words, filler removal and cleanup, and
//! also fill `${mishearings}` in the cleanup prompt so the model knows them.
//!
//! Matching ignores case and runs of spaces, and only matches whole words:
//! "quad code" fixes "Quad  code" but not "quadcode" or "squad codes".

use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};
use specta::Type;

/// One fix-it rule.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Type)]
pub struct FixRule {
    /// What the speech model writes, e.g. "quad code".
    pub heard: String,
    /// What was meant, e.g. "Claude Code".
    pub meant: String,
}

/// Longest allowed `heard` or `meant`, in characters.
pub const MAX_FIX_LEN: usize = 100;

/// Normalizes and validates rules from the settings UI: trims, collapses
/// spaces, drops rules with an empty side, and keeps the first rule for each
/// `heard` (ignoring case).
pub fn clean_rules(rules: Vec<FixRule>) -> Result<Vec<FixRule>, String> {
    let mut cleaned: Vec<FixRule> = Vec::new();
    for rule in rules {
        let heard = collapse_spaces(&rule.heard);
        let meant = collapse_spaces(&rule.meant);
        if heard.is_empty() || meant.is_empty() {
            continue;
        }
        if heard.chars().count() > MAX_FIX_LEN || meant.chars().count() > MAX_FIX_LEN {
            return Err(format!("Fixes are limited to {MAX_FIX_LEN} characters"));
        }
        if cleaned.iter().any(|r| r.heard.eq_ignore_ascii_case(&heard)) {
            continue;
        }
        cleaned.push(FixRule { heard, meant });
    }
    Ok(cleaned)
}

/// Applies every rule to `text`. Longer `heard` phrases go first, so
/// "quad code" wins over a shorter rule for "quad".
pub fn apply(text: &str, rules: &[FixRule]) -> String {
    let mut ordered: Vec<&FixRule> = rules.iter().collect();
    ordered.sort_by_key(|r| std::cmp::Reverse(r.heard.chars().count()));
    let mut text = text.to_string();
    for rule in ordered {
        if let Some(pattern) = pattern_for(&rule.heard) {
            text = pattern
                .replace_all(&text, regex::NoExpand(&rule.meant))
                .into_owned();
        }
    }
    text
}

/// [`apply`], and tells the achievements which rules fixed something
/// (Quad Code No More counts them).
pub fn apply_and_note(text: &str, rules: &[FixRule]) -> String {
    let hits: Vec<String> = rules
        .iter()
        .filter(|r| pattern_for(&r.heard).is_some_and(|p| p.is_match(text)))
        .map(|r| r.heard.clone())
        .collect();
    if !hits.is_empty() {
        crate::dispatch_achievements::note(crate::dispatch_achievements::Moment::FixesApplied(
            hits,
        ));
    }
    apply(text, rules)
}

/// The rules as the cleanup prompt lists them:
/// `"quad code" means "Claude Code"; "Eric Heat" means "Parakeet"`.
pub fn mishearings_for_prompt(rules: &[FixRule]) -> String {
    if rules.is_empty() {
        return "(none)".to_string();
    }
    rules
        .iter()
        .map(|r| format!("\"{}\" means \"{}\"", r.heard, r.meant))
        .collect::<Vec<_>>()
        .join("; ")
}

fn collapse_spaces(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Case-insensitive, whole-word pattern for `heard`. Word boundaries are only
/// required where the phrase starts or ends with a letter or digit, so a rule
/// like "c sharp" or "node.js" still works.
fn pattern_for(heard: &str) -> Option<Regex> {
    let words: Vec<String> = heard.split_whitespace().map(regex::escape).collect();
    if words.is_empty() {
        return None;
    }
    let edge = |c: Option<char>| match c {
        Some(c) if c.is_alphanumeric() => r"\b",
        _ => "",
    };
    let pattern = format!(
        "{}{}{}",
        edge(heard.chars().next()),
        words.join(r"\s+"),
        edge(heard.chars().last())
    );
    RegexBuilder::new(&pattern)
        .case_insensitive(true)
        .build()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(heard: &str, meant: &str) -> FixRule {
        FixRule {
            heard: heard.into(),
            meant: meant.into(),
        }
    }

    fn example_rules() -> Vec<FixRule> {
        vec![
            rule("quad code", "Claude Code"),
            rule("Eric Heat", "Parakeet"),
        ]
    }

    #[test]
    fn fixes_known_mishearings_in_any_case() {
        let rules = example_rules();
        assert_eq!(
            apply("open up quad code and switch to Eric heat", &rules),
            "open up Claude Code and switch to Parakeet"
        );
        assert_eq!(
            apply("Quad  Code is great.", &rules),
            "Claude Code is great."
        );
    }

    #[test]
    fn only_matches_whole_words() {
        let rules = example_rules();
        assert_eq!(apply("quadcode", &rules), "quadcode");
        assert_eq!(apply("squad codes", &rules), "squad codes");
    }

    #[test]
    fn longer_phrases_win() {
        let rules = vec![rule("quad", "Quad"), rule("quad code", "Claude Code")];
        assert_eq!(apply("the quad code", &rules), "the Claude Code");
    }

    #[test]
    fn replacement_text_is_literal() {
        let rules = vec![rule("price", "$5 ${x}")];
        assert_eq!(apply("the price", &rules), "the $5 ${x}");
    }

    #[test]
    fn punctuation_edges_still_match() {
        let rules = vec![rule("node js", "Node.js"), rule("c#", "C#")];
        assert_eq!(apply("run node js", &rules), "run Node.js");
        assert_eq!(apply("write c# code", &rules), "write C# code");
    }

    #[test]
    fn rules_are_cleaned_and_deduplicated() {
        let cleaned = clean_rules(vec![
            rule("  quad   code ", " Claude Code "),
            rule("Quad Code", "something else"),
            rule("", "empty heard"),
            rule("empty meant", "  "),
        ])
        .unwrap();
        assert_eq!(cleaned, vec![rule("quad code", "Claude Code")]);
        assert!(clean_rules(vec![rule(&"x".repeat(MAX_FIX_LEN + 1), "y")]).is_err());
    }

    #[test]
    fn prompt_lists_the_rules() {
        assert_eq!(
            mishearings_for_prompt(&example_rules()),
            r#""quad code" means "Claude Code"; "Eric Heat" means "Parakeet""#
        );
        assert_eq!(mishearings_for_prompt(&[]), "(none)");
    }
}

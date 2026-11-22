//! Dispatch: what George says when the read-aloud hotkey is pressed with
//! nothing selected. A hidden Easter egg (HANDOFF.md, "Read aloud"): the
//! first press gets a plain "Nothing is selected.", and pressing again and
//! again within a short time makes him more and more exasperated, until he
//! goes on a tea break. The first line of a run depends on what just
//! happened and on the time of day.
//!
//! Lines are plain text for Kokoro. `[pause]` inside a line becomes a longer
//! silence. Edit freely; the tests only check the shape of the lists.

use chrono::{Datelike, NaiveDate, NaiveDateTime, Timelike, Weekday};
use std::time::{Duration, Instant};

/// Marks a longer pause inside a line.
pub const PAUSE: &str = "[pause]";

/// Presses closer together than this count as one run and raise the level.
const RUN_WINDOW: Duration = Duration::from_secs(30);
/// How long George's tea break lasts after the last level.
const BREAK_LENGTH: Duration = Duration::from_secs(45);
/// A press this soon after a finished read gets an "that was all" line.
const JUST_READ: Duration = Duration::from_secs(15);

/// One list per level, mildest first. The last level sends him on a break.
const LEVELS: &[&[&str]] = &[
    &[
        "Nothing is selected.",
        "There's nothing selected.",
        "Select some text first, and I'll read it to you.",
    ],
    &[
        "Still nothing selected.",
        "I'm afraid there's still nothing there.",
        "Highlight something first, then press the keys.",
    ],
    &[
        "I can't read what isn't there.",
        "That's three times now. Nothing is selected.",
        "The signal's clear, but there's nothing on the line.",
    ],
    &[
        "Have you tried selecting something? It really does help.",
        "I'm starting to think you just like the sound of my voice.",
        "Other voices wouldn't put up with this, you know.",
    ],
    &[
        "This is becoming a bit of a habit.",
        "I'll read anything. A shopping list. A recipe. Anything at all.",
        "Even the cows in Vermont know how to select text.",
    ],
    &[
        "Shall I read you the silence, then? [pause] Moving, wasn't it?",
        "Right. I'll just wait here, then. [pause] Still waiting.",
        "Honestly, I'm a voice, not a mind reader.",
    ],
    &[
        "That's it. I'm putting the kettle on. Select something while I'm gone.",
        "I'm going on my break. Don't touch anything.",
        "You've worn me out. I'm off for a cup of tea.",
    ],
];

const ON_BREAK: &[&str] = &[
    "I said I'm on break.",
    "The kettle's still boiling.",
    "Shh. Tea time.",
];

const BACK: &[&str] = &[
    "Oh, you're back. Still nothing selected, I see.",
    "Right, I'm back. Did you select anything? No. Of course not.",
];

const AFTER_READ: &[&str] = &[
    "That was all of it. There's nothing else selected.",
    "That's the end. Select some more if you'd like another go.",
];

const LATE_NIGHT: &[&str] = &[
    "It's the middle of the night, and there's nothing selected.",
    "Shouldn't you be asleep? There's nothing selected.",
];

const MORNING: &[&str] = &[
    "Good morning. Nothing's selected yet.",
    "Morning. Select something and I'll read it.",
];

const FRIDAY: &[&str] = &["It's Friday afternoon. Nothing is selected, and honestly, fair enough."];

/// What to say, and whether this press sent George on his break (the hidden
/// "Pushing His Buttons" achievement, once achievements exist).
#[derive(Debug, PartialEq, Eq)]
pub struct Line {
    pub text: &'static str,
    pub wore_him_out: bool,
}

/// George's mood between presses. Lives for the session only.
#[derive(Default)]
pub struct Banter {
    level: usize,
    last_press: Option<Instant>,
    last_line: Option<&'static str>,
    last_read_end: Option<Instant>,
    break_until: Option<Instant>,
    last_day: Option<NaiveDate>,
}

impl Banter {
    /// A real read finished: the next empty press starts calm again.
    pub fn read_finished(&mut self, now: Instant) {
        self.last_read_end = Some(now);
        self.last_press = None;
        self.level = 0;
    }

    /// The hotkey was pressed with nothing selected. `clock` is local time;
    /// `seed` picks among a level's lines.
    pub fn nothing_selected(&mut self, now: Instant, clock: NaiveDateTime, seed: u64) -> Line {
        let first_today = self.last_day != Some(clock.date());
        self.last_day = Some(clock.date());

        if let Some(until) = self.break_until {
            self.last_press = Some(now);
            if now < until {
                return self.say(ON_BREAK, seed, false);
            }
            self.break_until = None;
            self.level = 0;
            return self.say(BACK, seed, false);
        }

        let in_a_run = self
            .last_press
            .is_some_and(|t| now.saturating_duration_since(t) <= RUN_WINDOW);
        self.level = if in_a_run {
            (self.level + 1).min(LEVELS.len() - 1)
        } else {
            0
        };
        self.last_press = Some(now);

        let lines = if self.level == 0 {
            self.opening(now, clock, first_today)
        } else {
            LEVELS[self.level]
        };
        let last_level = self.level == LEVELS.len() - 1;
        if last_level {
            self.break_until = Some(now + BREAK_LENGTH);
        }
        self.say(lines, seed, last_level)
    }

    fn opening(
        &self,
        now: Instant,
        clock: NaiveDateTime,
        first_today: bool,
    ) -> &'static [&'static str] {
        if self
            .last_read_end
            .is_some_and(|t| now.saturating_duration_since(t) <= JUST_READ)
        {
            return AFTER_READ;
        }
        let hour = clock.hour();
        if hour < 5 {
            LATE_NIGHT
        } else if first_today && hour < 11 {
            MORNING
        } else if clock.weekday() == Weekday::Fri && hour >= 15 {
            FRIDAY
        } else {
            LEVELS[0]
        }
    }

    fn say(&mut self, lines: &'static [&'static str], seed: u64, wore_him_out: bool) -> Line {
        let fresh: Vec<&'static str> = lines
            .iter()
            .copied()
            .filter(|line| Some(*line) != self.last_line)
            .collect();
        let pool = if fresh.is_empty() {
            lines.to_vec()
        } else {
            fresh
        };
        let text = pool[(seed % pool.len() as u64) as usize];
        self.last_line = Some(text);
        Line { text, wore_him_out }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    /// A Tuesday at 1 p.m., so no time-of-day line applies.
    fn afternoon() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 11, 24)
            .unwrap()
            .and_hms_opt(13, 0, 0)
            .unwrap()
    }

    fn in_list(list: &[&str], text: &str) -> bool {
        list.contains(&text)
    }

    #[test]
    fn quick_presses_escalate_to_a_tea_break_and_back() {
        let mut george = Banter::default();
        let start = Instant::now();
        let clock = afternoon();
        // First press of the day after 11 a.m. on a Tuesday: the plain level.
        george.last_day = Some(clock.date());
        for (level, lines) in LEVELS.iter().enumerate() {
            let now = start + Duration::from_secs(5 * level as u64);
            let line = george.nothing_selected(now, clock, level as u64);
            assert!(in_list(lines, line.text), "level {level}: {}", line.text);
            assert_eq!(line.wore_him_out, level == LEVELS.len() - 1);
        }
        let on_break = start + Duration::from_secs(40);
        assert!(in_list(
            ON_BREAK,
            george.nothing_selected(on_break, clock, 0).text
        ));
        let after = on_break + BREAK_LENGTH + Duration::from_secs(1);
        assert!(in_list(BACK, george.nothing_selected(after, clock, 0).text));
        // And calm again after that.
        let later = after + RUN_WINDOW + Duration::from_secs(1);
        assert!(in_list(
            LEVELS[0],
            george.nothing_selected(later, clock, 0).text
        ));
    }

    #[test]
    fn a_pause_between_presses_starts_over() {
        let mut george = Banter::default();
        let start = Instant::now();
        george.last_day = Some(afternoon().date());
        george.nothing_selected(start, afternoon(), 0);
        let second = george.nothing_selected(start + Duration::from_secs(3), afternoon(), 0);
        assert!(in_list(LEVELS[1], second.text));
        let much_later = start + Duration::from_secs(3) + RUN_WINDOW + Duration::from_secs(1);
        assert!(in_list(
            LEVELS[0],
            george.nothing_selected(much_later, afternoon(), 0).text
        ));
    }

    #[test]
    fn a_real_read_calms_him_down() {
        let mut george = Banter::default();
        let start = Instant::now();
        george.last_day = Some(afternoon().date());
        george.nothing_selected(start, afternoon(), 0);
        george.nothing_selected(start + Duration::from_secs(2), afternoon(), 0);
        george.read_finished(start + Duration::from_secs(4));
        let line = george.nothing_selected(start + Duration::from_secs(6), afternoon(), 0);
        assert!(in_list(AFTER_READ, line.text));
    }

    #[test]
    fn time_of_day_shapes_the_first_line() {
        let date = NaiveDate::from_ymd_opt(2026, 11, 24).unwrap();
        let mut george = Banter::default();
        let night = date.and_hms_opt(2, 30, 0).unwrap();
        assert!(in_list(
            LATE_NIGHT,
            george.nothing_selected(Instant::now(), night, 0).text
        ));

        let mut george = Banter::default();
        let morning = date.and_hms_opt(8, 0, 0).unwrap();
        assert!(in_list(
            MORNING,
            george.nothing_selected(Instant::now(), morning, 0).text
        ));

        let mut george = Banter::default();
        let friday = NaiveDate::from_ymd_opt(2026, 11, 27)
            .unwrap()
            .and_hms_opt(16, 0, 0)
            .unwrap();
        assert!(in_list(
            FRIDAY,
            george.nothing_selected(Instant::now(), friday, 0).text
        ));
    }

    #[test]
    fn the_same_line_never_comes_twice_in_a_row() {
        let mut george = Banter::default();
        let mut now = Instant::now();
        george.last_day = Some(afternoon().date());
        let mut last = "";
        for _ in 0..20 {
            // Far apart, so every press is level 0 with the same three lines.
            now += RUN_WINDOW + Duration::from_secs(1);
            let line = george.nothing_selected(now, afternoon(), 0).text;
            assert_ne!(line, last);
            last = line;
        }
    }

    #[test]
    fn every_list_has_lines_and_pauses_are_well_formed() {
        let all = LEVELS
            .iter()
            .copied()
            .chain([ON_BREAK, BACK, AFTER_READ, LATE_NIGHT, MORNING, FRIDAY]);
        let mut count = 0;
        for list in all {
            assert!(!list.is_empty());
            for line in list {
                count += 1;
                assert!(!line.trim().is_empty());
                for part in line.split(PAUSE) {
                    assert!(!part.trim().is_empty(), "empty side of a pause in {line}");
                }
            }
        }
        assert_eq!(count, 33);
    }
}

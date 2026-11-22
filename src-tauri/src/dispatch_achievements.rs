//! Dispatch: achievements (HANDOFF.md, "Feature: Achievements").
//!
//! Milestones the user reaches and odd things they stumble into, shown on the
//! Achievements page. Most are worked out from the saved dictations
//! (`history.db`, the recordings and the review window's corrections), so
//! they unlock on the date they were really earned, even for dictations made
//! before this existed. A few come from moments elsewhere in Dispatch: George
//! reading aloud (`dispatch_tts`), George going on his tea break
//! (`dispatch_banter`), and the Fix-It List fixing "quad code"
//! (`dispatch_fixes`). Those call [`note`].
//!
//! Unlocks are kept in `dispatch\achievements.json` under the app data
//! folder, so deleting a dictation never takes an achievement away. Each one
//! is marked seen once its pop-up has shown. Nothing in Handy's own files is
//! changed; history is only read.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use chrono::{Local, NaiveDate, NaiveDateTime, TimeZone, Timelike};
use log::{info, warn};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Emitter};
use tauri_specta::Event;

use crate::managers::history::HistoryUpdatePayload;

/// The event the frontend listens for: something new was unlocked.
pub const CHANGED_EVENT: &str = "dispatch-achievements-changed";

const STORE_VERSION: u32 = 1;
/// Catch up shortly after startup, once Dispatch has settled.
const STARTUP_DELAY: Duration = Duration::from_secs(5);

/// Voice data tiers, in minutes of recorded audio (rough guides from typical
/// Whisper fine-tunes, see HANDOFF.md).
const TIER_MINUTES: [(&str, f64); 4] = [
    ("tier_1", 30.0),
    ("tier_2", 120.0),
    ("tier_3", 300.0),
    ("tier_4", 600.0),
];
const MARATHON_MS: f64 = 180_000.0;
const BEDTIME_STORY: Duration = Duration::from_secs(5 * 60);
const QUAD_CODE_FIXES: u64 = 10;
/// Speed Talker: over this many words a minute, on a dictation long enough
/// that it isn't a fluke.
const SPEED_WPM: f64 = 200.0;
const SPEED_MIN_MS: f64 = 10_000.0;
const SPEED_MIN_WORDS: usize = 30;
/// Pace is averaged over this many days.
const PACE_DAYS: i64 = 14;

/// Where an achievement sits on the page.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum AchievementSection {
    Milestones,
    Habits,
    Features,
    Hidden,
    VoiceData,
}

struct Def {
    id: &'static str,
    section: AchievementSection,
}

/// Every achievement, in page order. Titles and descriptions live in the
/// frontend's translations, under `dispatch.achievements.items.<id>`.
const DEFS: &[Def] = &[
    def("first_words", AchievementSection::Milestones),
    def("getting_rolling", AchievementSection::Milestones),
    def("thousand_club", AchievementSection::Milestones),
    def("wordsmith", AchievementSection::Milestones),
    def("novelist", AchievementSection::Milestones),
    def("marathon", AchievementSection::Milestones),
    def("on_a_roll", AchievementSection::Habits),
    def("regular", AchievementSection::Habits),
    def("proofreader", AchievementSection::Features),
    def("fixer", AchievementSection::Features),
    def("course_correction", AchievementSection::Features),
    def("all_aboard", AchievementSection::Hidden),
    def("pizza_in_the_oven", AchievementSection::Hidden),
    def("might_could", AchievementSection::Hidden),
    def("night_shift", AchievementSection::Hidden),
    def("quad_code_no_more", AchievementSection::Hidden),
    def("speed_talker", AchievementSection::Hidden),
    def("pushing_his_buttons", AchievementSection::Hidden),
    def("bedtime_story", AchievementSection::Hidden),
    def("tier_1", AchievementSection::VoiceData),
    def("tier_2", AchievementSection::VoiceData),
    def("tier_3", AchievementSection::VoiceData),
    def("tier_4", AchievementSection::VoiceData),
];

const fn def(id: &'static str, section: AchievementSection) -> Def {
    Def { id, section }
}

/// Something that happened elsewhere in Dispatch that may unlock an
/// achievement.
#[derive(Debug, Clone)]
pub enum Moment {
    /// George finished (or was stopped partway through) reading the user's
    /// selected text, after playing for this long.
    ReadAloud(Duration),
    /// George was pestered all the way to his tea break.
    GeorgeOnBreak,
    /// The Fix-It List fixed these phrases (their `heard` side) in a transcript.
    FixesApplied(Vec<String>),
}

/// One achievement as the page shows it.
#[derive(Serialize, Debug, Clone, Type)]
pub struct Achievement {
    pub id: String,
    pub section: AchievementSection,
    /// Unix seconds, when it was earned.
    pub unlocked_at: Option<i64>,
    /// Its unlock pop-up has been shown.
    pub seen: bool,
    /// Progress toward it, for the ones with a count (never for hidden ones).
    pub have: Option<f64>,
    pub need: Option<f64>,
}

/// Everything the Achievements page shows.
#[derive(Serialize, Debug, Clone, Type)]
pub struct AchievementsStatus {
    pub achievements: Vec<Achievement>,
    /// Recorded audio across every dictation, in milliseconds.
    pub recorded_ms: f64,
    /// Audio of the dictations checked in the review window (marked "Looks
    /// right" or with a word fixed): the best training data, since their text
    /// is known to be right.
    pub checked_ms: f64,
    /// Whether every recording is kept. With only the newest few kept, the
    /// voice data meter and the training tiers don't apply, and the page hides
    /// them (`dispatch_audio`).
    pub keeps_all_audio: bool,
    /// Recorded audio per day over the last couple of weeks (or since the
    /// first dictation, if that's sooner). None before the first dictation.
    pub per_day_ms: Option<f64>,
    pub dictations: u32,
    pub words: u32,
    pub days_used: u32,
}

/// Tells the achievements about a moment. Cheap and never blocks: the work
/// happens on the achievements thread.
pub fn note(moment: Moment) {
    if let Some(tx) = SENDER.get() {
        let _ = tx.lock().map(|tx| tx.send(Message::Moment(moment)));
    }
}

/// Asks for the achievements to be worked out again, for example after the
/// Fix-It List or a correction changed.
pub fn refresh() {
    if let Some(tx) = SENDER.get() {
        let _ = tx.lock().map(|tx| tx.send(Message::Refresh));
    }
}

/// Starts the achievements thread: catches up shortly after startup, then
/// again whenever history changes or a moment is noted.
pub fn start(app: &AppHandle) {
    let (tx, rx) = mpsc::channel::<Message>();
    if SENDER.set(Mutex::new(tx.clone())).is_err() {
        return; // already running
    }
    let worker_app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("dispatch-achievements".into())
        .spawn(move || run(worker_app, rx));
    if let Err(e) = spawned {
        warn!("Achievements: couldn't start: {e}");
        return;
    }
    let history_tx = Mutex::new(tx.clone());
    HistoryUpdatePayload::listen_any(app, move |event| {
        if matches!(
            event.payload,
            HistoryUpdatePayload::Added { .. } | HistoryUpdatePayload::Updated { .. }
        ) {
            let _ = history_tx.lock().map(|tx| tx.send(Message::Refresh));
        }
    });
    std::thread::spawn(move || {
        std::thread::sleep(STARTUP_DELAY);
        let _ = tx.send(Message::Refresh);
    });
}

/// The page's data, working out anything new first.
pub fn status(app: &AppHandle) -> Result<AchievementsStatus, String> {
    let (status, new) = {
        let mut state = state().lock().map_err(|_| "achievements are unavailable")?;
        state.evaluate(app)?
    };
    if new {
        let _ = app.emit(CHANGED_EVENT, ());
    }
    Ok(AchievementsStatus {
        keeps_all_audio: crate::settings::get_settings(app).dispatch_keep_all_audio,
        ..status
    })
}

/// Marks achievements' unlock pop-ups as shown.
pub fn mark_seen(app: &AppHandle, ids: &[String]) -> Result<(), String> {
    let mut state = state().lock().map_err(|_| "achievements are unavailable")?;
    state.load(app);
    let mut changed = false;
    for id in ids {
        if let Some(unlock) = state.store.unlocked.get_mut(id) {
            changed |= !unlock.seen;
            unlock.seen = true;
        }
    }
    if changed {
        state.save(app)?;
    }
    Ok(())
}

enum Message {
    Refresh,
    Moment(Moment),
}

static SENDER: OnceLock<Mutex<mpsc::Sender<Message>>> = OnceLock::new();

fn state() -> &'static Mutex<State> {
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(State::default()))
}

fn run(app: AppHandle, rx: mpsc::Receiver<Message>) {
    while let Ok(first) = rx.recv() {
        // Several messages in a row: take them all, then work things out once.
        let mut moments = Vec::new();
        for message in std::iter::once(first).chain(rx.try_iter()) {
            if let Message::Moment(moment) = message {
                moments.push(moment);
            }
        }
        let Ok(mut state) = state().lock() else {
            return;
        };
        state.load(&app);
        let now = Local::now().timestamp();
        let mut new = false;
        for moment in &moments {
            new |= state.store.apply(moment, now);
        }
        match state.evaluate(&app) {
            Ok((_, found)) => new |= found,
            Err(e) => warn!("Achievements: {e}"),
        }
        if new {
            if let Err(e) = state.save(&app) {
                warn!("Achievements: couldn't save: {e}");
            }
        }
        // Tell the window only after letting go of the state.
        drop(state);
        if new {
            let _ = app.emit(CHANGED_EVENT, ());
        }
    }
}

/// What's saved in `achievements.json`.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
struct Store {
    version: u32,
    unlocked: BTreeMap<String, Unlock>,
    /// Running counts for moments, such as "fix:quad code".
    counts: BTreeMap<String, u64>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct Unlock {
    /// Unix seconds.
    at: i64,
    seen: bool,
}

impl Store {
    /// Unlocks `id` at `at` unless it already is. True if it's new.
    fn unlock(&mut self, id: &str, at: i64) -> bool {
        if self.unlocked.contains_key(id) {
            return false;
        }
        info!("Achievements: unlocked {id}");
        self.unlocked
            .insert(id.to_string(), Unlock { at, seen: false });
        true
    }

    /// Records a moment. True if it unlocked something.
    fn apply(&mut self, moment: &Moment, now: i64) -> bool {
        match moment {
            Moment::ReadAloud(played) => {
                let mut new = self.unlock("proofreader", now);
                if *played >= BEDTIME_STORY {
                    new |= self.unlock("bedtime_story", now);
                }
                new
            }
            Moment::GeorgeOnBreak => self.unlock("pushing_his_buttons", now),
            Moment::FixesApplied(heard) => {
                for phrase in heard {
                    *self
                        .counts
                        .entry(format!("fix:{}", phrase.to_lowercase()))
                        .or_default() += 1;
                }
                if self.counts.get("fix:quad code").copied().unwrap_or(0) >= QUAD_CODE_FIXES {
                    self.unlock("quad_code_no_more", now)
                } else {
                    false
                }
            }
        }
    }
}

#[derive(Default)]
struct State {
    loaded: bool,
    store: Store,
    /// Recording lengths in milliseconds, by file name (reading a WAV's
    /// header each time adds up).
    lengths: HashMap<String, f64>,
}

impl State {
    fn load(&mut self, app: &AppHandle) {
        if self.loaded {
            return;
        }
        self.loaded = true;
        let Some(path) = store_path(app) else {
            return;
        };
        match std::fs::read(&path) {
            Ok(bytes) => match serde_json::from_slice(&bytes) {
                Ok(store) => self.store = store,
                Err(e) => warn!("Achievements: {} is unreadable: {e}", path.display()),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => warn!("Achievements: couldn't read {}: {e}", path.display()),
        }
    }

    fn save(&mut self, app: &AppHandle) -> Result<(), String> {
        let path = store_path(app).ok_or("no app data folder")?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        self.store.version = STORE_VERSION;
        let json = serde_json::to_vec_pretty(&self.store).map_err(|e| e.to_string())?;
        let partial = path.with_extension("json.partial");
        std::fs::write(&partial, json).map_err(|e| e.to_string())?;
        std::fs::rename(&partial, &path).map_err(|e| e.to_string())
    }

    /// Works out everything from the saved dictations. The bool is true when
    /// something new was unlocked (and saved).
    fn evaluate(&mut self, app: &AppHandle) -> Result<(AchievementsStatus, bool), String> {
        self.load(app);
        let data_dir = crate::portable::app_data_dir(app).map_err(|e| e.to_string())?;
        let rows = read_history(&data_dir.join("history.db"))?;
        let recordings = data_dir.join("recordings");
        let dictations: Vec<Dictation> = rows
            .into_iter()
            .map(|row| {
                let length_ms = *self
                    .lengths
                    .entry(row.file_name.clone())
                    .or_insert_with(|| recording_length_ms(&recordings.join(&row.file_name)));
                Dictation {
                    file_name: row.file_name,
                    at: row.timestamp,
                    heard: row.heard,
                    shown: row.shown,
                    length_ms,
                }
            })
            .collect();
        let checked = read_corrections(&data_dir.join("dispatch").join("corrections"));
        let settings = crate::settings::get_settings(app);
        let own_fix_rule = settings.dispatch_fixes.iter().any(|rule| {
            !crate::dispatch::SEED_FIXES
                .iter()
                .any(|(heard, _)| heard.eq_ignore_ascii_case(&rule.heard))
        });

        let now = Local::now().timestamp();
        let facts = work_out(&dictations, &checked, now);
        let mut new = false;
        for (id, at) in &facts.unlocks {
            new |= self.store.unlock(id, *at);
        }
        if own_fix_rule {
            new |= self.store.unlock("fixer", now);
        }
        if new {
            self.save(app)?;
        }
        Ok((self.status(&facts), new))
    }

    fn status(&self, facts: &Facts) -> AchievementsStatus {
        let achievements = DEFS
            .iter()
            .map(|def| {
                let unlock = self.store.unlocked.get(def.id);
                let (have, need) = match (def.section, facts.progress.get(def.id)) {
                    (AchievementSection::Hidden, _) | (_, None) => (None, None),
                    (_, Some(&(have, need))) => (Some(have), Some(need)),
                };
                Achievement {
                    id: def.id.to_string(),
                    section: def.section,
                    unlocked_at: unlock.map(|u| u.at),
                    seen: unlock.is_some_and(|u| u.seen),
                    have,
                    need,
                }
            })
            .collect();
        AchievementsStatus {
            achievements,
            recorded_ms: facts.recorded_ms,
            checked_ms: facts.checked_ms,
            per_day_ms: facts.per_day_ms,
            dictations: facts.dictations,
            words: facts.words,
            days_used: facts.days_used,
            keeps_all_audio: true,
        }
    }
}

/// One saved dictation, as the achievements see it.
#[derive(Debug, Clone)]
struct Dictation {
    file_name: String,
    /// Unix seconds.
    at: i64,
    /// What the speech model wrote (after the Fix-It List).
    heard: String,
    /// What was pasted: the cleaned text when there is one.
    shown: String,
    length_ms: f64,
}

/// What [`work_out`] found.
#[derive(Debug, Default)]
struct Facts {
    /// Earned achievements and when each was earned.
    unlocks: Vec<(&'static str, i64)>,
    /// (have, need) for the ones with a count.
    progress: HashMap<&'static str, (f64, f64)>,
    recorded_ms: f64,
    checked_ms: f64,
    per_day_ms: Option<f64>,
    dictations: u32,
    words: u32,
    days_used: u32,
}

/// Everything that can be told from the dictations alone. `checked` holds
/// the recordings checked in the review window, each with when its first word
/// was fixed (None if it was marked "Looks right" with nothing to fix).
fn work_out(dictations: &[Dictation], checked: &HashMap<String, Option<i64>>, now: i64) -> Facts {
    let mut sorted: Vec<&Dictation> = dictations.iter().collect();
    sorted.sort_by_key(|d| d.at);

    let mut facts = Facts {
        dictations: sorted.len() as u32,
        ..Default::default()
    };
    let earn = |facts: &mut Facts, id: &'static str, at: i64| {
        if !facts.unlocks.iter().any(|(known, _)| *known == id) {
            facts.unlocks.push((id, at));
        }
    };

    let mut words = 0usize;
    let mut recorded_ms = 0.0;
    let mut first_on_day: BTreeMap<NaiveDate, i64> = BTreeMap::new();
    for (i, d) in sorted.iter().enumerate() {
        let count = i + 1;
        if count == 1 {
            earn(&mut facts, "first_words", d.at);
        }
        if count == 100 {
            earn(&mut facts, "getting_rolling", d.at);
        }
        if count == 1_000 {
            earn(&mut facts, "thousand_club", d.at);
        }

        let before = words;
        words += word_count(&d.shown);
        if before < 10_000 && words >= 10_000 {
            earn(&mut facts, "wordsmith", d.at);
        }
        if before < 50_000 && words >= 50_000 {
            earn(&mut facts, "novelist", d.at);
        }

        let recorded_before = recorded_ms;
        recorded_ms += d.length_ms;
        for (id, minutes) in TIER_MINUTES {
            let need = minutes * 60_000.0;
            if recorded_before < need && recorded_ms >= need {
                earn(&mut facts, id, d.at);
            }
        }

        if d.length_ms > MARATHON_MS {
            earn(&mut facts, "marathon", d.at);
        }
        let local = local_time(d.at);
        first_on_day.entry(local.date()).or_insert(d.at);
        if (2..4).contains(&local.hour()) {
            earn(&mut facts, "night_shift", d.at);
        }
        let heard = phrase_text(&d.heard);
        let shown = phrase_text(&d.shown);
        if heard.contains(" all aboard ") || shown.contains(" all aboard ") {
            earn(&mut facts, "all_aboard", d.at);
        }
        if heard.contains(" pizza in the oven ") {
            earn(&mut facts, "pizza_in_the_oven", d.at);
        }
        if shown.contains(" might could ") {
            earn(&mut facts, "might_could", d.at);
        }
        let heard_words = word_count(&d.heard);
        if d.length_ms >= SPEED_MIN_MS
            && heard_words >= SPEED_MIN_WORDS
            && heard_words as f64 / (d.length_ms / 60_000.0) > SPEED_WPM
        {
            earn(&mut facts, "speed_talker", d.at);
        }
        if checked.contains_key(&d.file_name) {
            facts.checked_ms += d.length_ms;
        }
    }
    // Course Correction dates from the first fix, whichever dictation it was.
    if let Some(first_fix) = sorted
        .iter()
        .filter_map(|d| checked.get(&d.file_name).copied().flatten())
        .min()
    {
        earn(&mut facts, "course_correction", first_fix);
    }

    // Days in a row.
    let today = local_time(now).date();
    let mut run = 0i64;
    let mut previous: Option<NaiveDate> = None;
    for (&day, &at) in &first_on_day {
        run = match previous {
            Some(p) if (day - p).num_days() == 1 => run + 1,
            _ => 1,
        };
        previous = Some(day);
        if run == 7 {
            earn(&mut facts, "on_a_roll", at);
        }
        if run == 30 {
            earn(&mut facts, "regular", at);
        }
    }
    // The streak still going: it counts until a whole day is missed.
    let current = match previous {
        Some(last) if (today - last).num_days() <= 1 => run,
        _ => 0,
    } as f64;

    facts.words = words as u32;
    facts.recorded_ms = recorded_ms;
    facts.days_used = first_on_day.len() as u32;
    let count = sorted.len() as f64;
    for (id, have, need) in [
        ("getting_rolling", count, 100.0),
        ("thousand_club", count, 1_000.0),
        ("wordsmith", words as f64, 10_000.0),
        ("novelist", words as f64, 50_000.0),
        ("on_a_roll", current, 7.0),
        ("regular", current, 30.0),
    ] {
        facts.progress.insert(id, (have.min(need), need));
    }

    // Pace: recorded audio per day over the last two weeks, counting only
    // the days since the first dictation.
    if let Some((&first_day, _)) = first_on_day.iter().next() {
        let window_start = (today - chrono::Duration::days(PACE_DAYS - 1)).max(first_day);
        let days = ((today - window_start).num_days() + 1).max(1) as f64;
        let recent_ms: f64 = sorted
            .iter()
            .filter(|d| local_time(d.at).date() >= window_start)
            .map(|d| d.length_ms)
            .sum();
        facts.per_day_ms = Some(recent_ms / days);
    }
    facts
}

/// Lowercase words separated by single spaces, with a space at each end, so
/// `contains(" might could ")` only matches whole words.
fn phrase_text(text: &str) -> String {
    let cleaned: String = text
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '\'' {
                c.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect();
    format!(
        " {} ",
        cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
    )
}

fn word_count(text: &str) -> usize {
    text.split_whitespace()
        .filter(|w| w.chars().any(char::is_alphanumeric))
        .count()
}

fn local_time(at: i64) -> NaiveDateTime {
    Local
        .timestamp_opt(at, 0)
        .single()
        .map(|t| t.naive_local())
        .unwrap_or_default()
}

struct Row {
    file_name: String,
    timestamp: i64,
    heard: String,
    shown: String,
}

/// The dictations in history (not transcribed files), read-only.
fn read_history(db: &Path) -> Result<Vec<Row>, String> {
    if !db.is_file() {
        return Ok(Vec::new());
    }
    let conn =
        rusqlite::Connection::open_with_flags(db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| format!("can't open history: {e}"))?;
    // Don't wait long behind a write; the next change tries again.
    let _ = conn.busy_timeout(Duration::from_millis(250));
    let mut statement = conn
        .prepare(
            "SELECT file_name, timestamp, transcription_text, post_processed_text \
             FROM transcription_history WHERE source_name IS NULL",
        )
        .map_err(|e| format!("can't read history: {e}"))?;
    let rows = statement
        .query_map([], |row| {
            let heard: String = row.get(2)?;
            let cleaned: Option<String> = row.get(3)?;
            let shown = cleaned
                .filter(|c| !c.trim().is_empty())
                .unwrap_or_else(|| heard.clone());
            Ok(Row {
                file_name: row.get(0)?,
                timestamp: row.get(1)?,
                heard,
                shown,
            })
        })
        .map_err(|e| format!("can't read history: {e}"))?;
    Ok(rows.filter_map(Result::ok).collect())
}

/// The recordings checked in the review window, from `dispatch\corrections\`
/// (`dispatch_review`), each with when its first word was fixed: None if it
/// was marked "Looks right" with nothing fixed.
fn read_corrections(dir: &Path) -> HashMap<String, Option<i64>> {
    #[derive(Deserialize)]
    struct Corrections {
        file_name: String,
        fixes: Vec<Fix>,
        #[serde(default)]
        checked_at: Option<i64>,
    }
    #[derive(Deserialize)]
    struct Fix {
        at: i64,
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return HashMap::new();
    };
    let mut seen = HashSet::new();
    entries
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| std::fs::read(e.path()).ok())
        .filter_map(|bytes| serde_json::from_slice::<Corrections>(&bytes).ok())
        .filter_map(|c| {
            let first = c.fixes.iter().map(|f| f.at).min();
            if first.is_none() && c.checked_at.is_none() {
                return None;
            }
            seen.insert(c.file_name.clone())
                .then_some((c.file_name, first))
        })
        .collect()
}

/// A recording's length from its WAV header, or 0 if it can't be read.
fn recording_length_ms(path: &Path) -> f64 {
    match hound::WavReader::open(path) {
        Ok(reader) => {
            let spec = reader.spec();
            reader.duration() as f64 * 1000.0 / spec.sample_rate.max(1) as f64
        }
        Err(_) => 0.0,
    }
}

fn store_path(app: &AppHandle) -> Option<PathBuf> {
    Some(
        crate::portable::app_data_dir(app)
            .ok()?
            .join("dispatch")
            .join("achievements.json"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Unix seconds for a local date and time.
    fn at(y: i32, m: u32, d: u32, h: u32, min: u32) -> i64 {
        Local
            .with_ymd_and_hms(y, m, d, h, min, 0)
            .single()
            .unwrap()
            .timestamp()
    }

    fn dictation(at: i64, text: &str, length_ms: f64) -> Dictation {
        Dictation {
            file_name: format!("dispatch-{at}.wav"),
            at,
            heard: text.to_string(),
            shown: text.to_string(),
            length_ms,
        }
    }

    fn unlocked(facts: &Facts, id: &str) -> Option<i64> {
        facts
            .unlocks
            .iter()
            .find(|(known, _)| *known == id)
            .map(|(_, at)| *at)
    }

    #[test]
    fn every_achievement_has_a_unique_id() {
        let ids: HashSet<_> = DEFS.iter().map(|d| d.id).collect();
        assert_eq!(ids.len(), DEFS.len());
        assert_eq!(DEFS.len(), 23);
        assert_eq!(
            DEFS.iter()
                .filter(|d| d.section == AchievementSection::Hidden)
                .count(),
            8
        );
    }

    #[test]
    fn milestones_unlock_on_the_dictation_that_earned_them() {
        let start = at(2026, 11, 21, 9, 0);
        let dictations: Vec<_> = (0..120)
            .map(|i| dictation(start + i * 60, "testing one two", 3_000.0))
            .collect();
        let facts = work_out(&dictations, &HashMap::new(), start + 7_200);
        assert_eq!(unlocked(&facts, "first_words"), Some(start));
        assert_eq!(unlocked(&facts, "getting_rolling"), Some(start + 99 * 60));
        assert_eq!(unlocked(&facts, "thousand_club"), None);
        assert_eq!(facts.progress["thousand_club"], (120.0, 1_000.0));
        assert_eq!(facts.words, 360);
        assert_eq!(facts.dictations, 120);
    }

    #[test]
    fn hidden_phrases_match_whole_words_only() {
        let t = at(2026, 11, 21, 12, 0);
        let facts = work_out(
            &[
                dictation(t, "All aboard! Next stop, Ashland.", 4_000.0),
                dictation(t + 1, "I'm going to throw a pizza in the oven.", 4_000.0),
                dictation(t + 2, "We mightcould do it.", 4_000.0),
            ],
            &HashMap::new(),
            t + 10,
        );
        assert_eq!(unlocked(&facts, "all_aboard"), Some(t));
        assert_eq!(unlocked(&facts, "pizza_in_the_oven"), Some(t + 1));
        assert_eq!(unlocked(&facts, "might_could"), None);
    }

    #[test]
    fn might_could_needs_to_survive_cleanup() {
        let t = at(2026, 11, 21, 12, 0);
        let mut d = dictation(t, "Might could build that in stages", 4_000.0);
        d.shown = "Could build that in stages.".into();
        assert_eq!(
            unlocked(&work_out(&[d], &HashMap::new(), t), "might_could"),
            None
        );
    }

    #[test]
    fn night_shift_is_two_to_four_in_the_morning() {
        let early = at(2026, 11, 22, 1, 59);
        let night = at(2026, 11, 22, 2, 30);
        let facts = work_out(
            &[
                dictation(early, "hi", 1_000.0),
                dictation(night, "hi", 1_000.0),
            ],
            &HashMap::new(),
            night,
        );
        assert_eq!(unlocked(&facts, "night_shift"), Some(night));
    }

    #[test]
    fn streaks_count_days_in_a_row() {
        let days: Vec<_> = [1, 2, 3, 5, 6, 7, 8, 9, 10, 11]
            .iter()
            .map(|&d| dictation(at(2026, 12, d, 10, 0), "hello", 1_000.0))
            .collect();
        let facts = work_out(&days, &HashMap::new(), at(2026, 12, 12, 9, 0));
        // 5th to 11th is the first seven in a row.
        assert_eq!(unlocked(&facts, "on_a_roll"), Some(at(2026, 12, 11, 10, 0)));
        // Nothing yet on the 12th, but the streak isn't broken until the 13th.
        assert_eq!(facts.progress["on_a_roll"], (7.0, 7.0));
        assert_eq!(facts.progress["regular"], (7.0, 30.0));
        let later = work_out(&days, &HashMap::new(), at(2026, 12, 13, 9, 0));
        assert_eq!(later.progress["regular"], (0.0, 30.0));
        assert_eq!(later.days_used, 10);
    }

    #[test]
    fn voice_tiers_and_pace_come_from_recorded_audio() {
        let t = at(2026, 11, 21, 9, 0);
        let dictations: Vec<_> = (0..40)
            .map(|i| dictation(t + i * 60, "words", 60_000.0))
            .collect();
        let facts = work_out(&dictations, &HashMap::new(), t + 3_600);
        assert_eq!(unlocked(&facts, "tier_1"), Some(t + 29 * 60));
        assert_eq!(unlocked(&facts, "tier_2"), None);
        assert_eq!(facts.recorded_ms, 2_400_000.0);
        // All on day one: 40 minutes a day.
        assert_eq!(facts.per_day_ms, Some(2_400_000.0));
    }

    #[test]
    fn speed_talker_needs_a_real_dictation() {
        let t = at(2026, 11, 21, 9, 0);
        let fast = vec!["word"; 40].join(" "); // 40 words in 10 s = 240 wpm
        let short = vec!["word"; 12].join(" "); // 12 words in 2 s, too short
        let facts = work_out(
            &[
                dictation(t, &short, 2_000.0),
                dictation(t + 1, &fast, 10_000.0),
            ],
            &HashMap::new(),
            t + 5,
        );
        assert_eq!(unlocked(&facts, "speed_talker"), Some(t + 1));
    }

    #[test]
    fn course_correction_dates_from_the_first_fix() {
        let t = at(2026, 11, 21, 9, 0);
        let a = dictation(t, "one", 5_000.0);
        let b = dictation(t + 60, "two", 7_000.0);
        let c = dictation(t + 120, "three", 4_000.0);
        let checked = HashMap::from([
            (b.file_name.clone(), Some(t + 500)),
            (a.file_name.clone(), Some(t + 900)),
            // Marked "Looks right": checked audio, but not a fix.
            (c.file_name.clone(), None),
        ]);
        let facts = work_out(&[a, b, c], &checked, t + 1_000);
        assert_eq!(unlocked(&facts, "course_correction"), Some(t + 500));
        assert_eq!(facts.checked_ms, 16_000.0);
    }

    #[test]
    fn looks_right_alone_isnt_a_course_correction() {
        let t = at(2026, 11, 21, 9, 0);
        let a = dictation(t, "one", 5_000.0);
        let checked = HashMap::from([(a.file_name.clone(), None)]);
        let facts = work_out(&[a], &checked, t + 1_000);
        assert_eq!(unlocked(&facts, "course_correction"), None);
        assert_eq!(facts.checked_ms, 5_000.0);
    }

    #[test]
    fn moments_unlock_once() {
        let mut store = Store::default();
        assert!(store.apply(&Moment::ReadAloud(Duration::from_secs(20)), 100));
        assert!(!store.unlocked.contains_key("bedtime_story"));
        assert!(store.apply(&Moment::ReadAloud(Duration::from_secs(301)), 200));
        assert_eq!(store.unlocked["proofreader"].at, 100);
        assert_eq!(store.unlocked["bedtime_story"].at, 200);
        assert!(store.apply(&Moment::GeorgeOnBreak, 300));
        assert!(!store.apply(&Moment::GeorgeOnBreak, 400));
    }

    #[test]
    fn quad_code_needs_ten_fixes() {
        let mut store = Store::default();
        let fix = Moment::FixesApplied(vec!["Quad Code".into()]);
        for i in 0..9 {
            assert!(!store.apply(&fix, i));
        }
        assert!(store.apply(&fix, 9));
        assert_eq!(store.counts["fix:quad code"], 10);
    }
}

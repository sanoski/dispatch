//! Dispatch: read the selected text aloud with Kokoro (HANDOFF.md, "Read
//! aloud").
//!
//! Select text in any app and press the read-aloud hotkey (default
//! Ctrl+Alt+R): Dispatch speaks it. Press the hotkey again to stop. Kokoro
//! runs inside Dispatch on the CPU through `loqui-kokoro` (Rust and ONNX
//! Runtime, no Python, no GPL code). The text is spoken a sentence or two at
//! a time, so the first words start while the rest is still being made.
//!
//! The model files are downloaded into Dispatch's own data folder
//! (`dispatch\kokoro\`, see `dispatch_voice_files`). A debug build without
//! them falls back to the repo's `vendor/kokoro/`.

use std::collections::HashMap;
use std::num::NonZero;
use std::ops::RangeInclusive;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use log::{debug, error, info, warn};
use loqui_kokoro::{Blend, Device, Kokoro, KokoroModel, SAMPLE_RATE};
use rodio::cpal::traits::HostTrait;
use tauri::AppHandle;

use crate::actions::ShortcutAction;
use crate::dispatch_reading_pill::PillState;
use crate::settings::{get_settings, ShortcutBinding};

/// The read-aloud hotkey's id in `AppSettings::bindings`.
pub const BINDING_ID: &str = "dispatch_read_aloud";
const DEFAULT_SHORTCUT: &str = "ctrl+alt+r";

/// Joshua's pick from the voice samples (2026-11-21): a British man.
pub const DEFAULT_VOICE: &str = "bm_george";
pub const SPEED_RANGE: RangeInclusive<f32> = 0.5..=2.0;

/// The dev copy in the repo's `vendor/kokoro/` (kokoro-onnx's release file,
/// a different export of the same model).
#[cfg(debug_assertions)]
const DEV_MODEL_FILE: &str = "kokoro-v1.0.onnx";
/// How long the pill says the voice needs downloading.
const NEEDS_VOICE_FOR: Duration = Duration::from_secs(8);

/// Longest piece of text sent to Kokoro at once. Shorter pieces start
/// sooner; Kokoro itself handles up to 510 phonemes (roughly 400 characters).
const MAX_CHUNK_CHARS: usize = 300;
/// Silence between pieces. Kokoro trims each piece's edges, so without it
/// sentences run into each other.
const PAUSE_BETWEEN_CHUNKS: Duration = Duration::from_millis(180);
/// Silence for a `[pause]` mark (see `dispatch_banter`).
const LONG_PAUSE: Duration = Duration::from_millis(1400);

pub fn default_voice() -> String {
    DEFAULT_VOICE.to_string()
}

pub fn default_speed() -> f32 {
    1.0
}

/// The voices Dispatch offers: Kokoro's American and British English ones.
pub fn voices() -> Vec<String> {
    loqui_kokoro::ENGLISH_VOICES
        .iter()
        .map(|v| v.to_string())
        .collect()
}

pub fn is_voice(voice: &str) -> bool {
    loqui_kokoro::ENGLISH_VOICES.contains(&voice)
}

/// Adds the read-aloud hotkey to the default bindings. Existing settings get
/// it too: Handy adds any default binding a stored settings file is missing.
pub fn add_default_binding(bindings: &mut HashMap<String, ShortcutBinding>) {
    bindings.insert(
        BINDING_ID.to_string(),
        ShortcutBinding {
            id: BINDING_ID.to_string(),
            name: "Read Aloud".to_string(),
            description: "Reads the selected text aloud. Press again to stop.".to_string(),
            default_binding: DEFAULT_SHORTCUT.to_string(),
            current_binding: DEFAULT_SHORTCUT.to_string(),
        },
    );
}

/// The hotkey: on press, read the selection, or stop if already reading.
pub struct ReadAloudAction;

impl ShortcutAction for ReadAloudAction {
    fn start(&self, app: &AppHandle, _binding_id: &str, _shortcut_str: &str) {
        toggle(app);
    }

    fn stop(&self, _app: &AppHandle, _binding_id: &str, _shortcut_str: &str) {}
}

/// Reads the selection aloud, or stops reading if Dispatch already is.
pub fn toggle(app: &AppHandle) {
    let reader = reader();
    if reader.busy.swap(true, Ordering::SeqCst) {
        stop();
        return;
    }
    let app = app.clone();
    let generation = reader.generation.load(Ordering::SeqCst);
    std::thread::spawn(
        move || match crate::dispatch_selection::selected_text(&app) {
            Some(text) => speak(&app, &text, None, generation, Kind::Selection),
            None => {
                let text = nothing_selected_line();
                speak(&app, &text, None, generation, Kind::Banter);
            }
        },
    );
}

/// George's answer to an empty selection (`dispatch_banter`).
fn nothing_selected_line() -> String {
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64)
        .unwrap_or(0);
    let Ok(mut banter) = reader().banter.lock() else {
        return "Nothing is selected.".to_string();
    };
    let line = banter.nothing_selected(Instant::now(), chrono::Local::now().naive_local(), seed);
    if line.wore_him_out {
        info!("Read aloud: nothing selected, and George went on a tea break");
        crate::dispatch_achievements::note(crate::dispatch_achievements::Moment::GeorgeOnBreak);
    } else {
        info!("Read aloud: nothing selected");
    }
    line.text.to_string()
}

/// Longest text the Read Aloud page sends at once (about 30 pages).
pub const MAX_PASTED_CHARS: usize = 100_000;

/// Reads `text` (from the Read Aloud page, or a dictation read back),
/// stopping anything already being read. Like a selection: the pill shows
/// and the hotkey stops it.
pub fn read_text(app: &AppHandle, text: &str) {
    let text = text.trim();
    if text.is_empty() {
        return;
    }
    stop();
    let reader = reader();
    reader.busy.store(true, Ordering::SeqCst);
    let generation = reader.generation.load(Ordering::SeqCst);
    speak(app, text, None, generation, Kind::Selection);
}

/// Stage 3: with "Read back each dictation" on, reads every dictation once
/// it's saved, just before it's pasted (history saves it first). Transcribed
/// files are left alone.
pub fn start_read_back(app: &AppHandle) {
    use crate::managers::history::HistoryUpdatePayload;
    use tauri_specta::Event;
    let handle = app.clone();
    HistoryUpdatePayload::listen_any(app, move |event| {
        let HistoryUpdatePayload::Added { entry } = event.payload else {
            return;
        };
        if entry.source_name.is_some() || !get_settings(&handle).dispatch_read_back {
            return;
        }
        let text = entry
            .post_processed_text
            .filter(|t| !t.trim().is_empty())
            .unwrap_or(entry.transcription_text);
        debug!("Read aloud: reading back a dictation");
        read_text(&handle, &text);
    });

    // A new recording stops George, so his voice never ends up in it.
    // Handy's overlay announces each recording ("show-overlay"); with the
    // overlay turned off there's no announcement, and Handy's "mute while
    // recording" setting is what keeps him out.
    tauri::Listener::listen_any(app, "show-overlay", |event| {
        let state = event.payload().trim_matches('"');
        if matches!(state, "recording" | "streaming") && reader().busy.load(Ordering::SeqCst) {
            debug!("Read aloud: stopped for a new recording");
            stop();
        }
    });
}

/// Plays a short sample of `voice`, stopping anything being read.
pub fn preview(app: &AppHandle, voice: &str) {
    stop();
    let reader = reader();
    reader.busy.store(true, Ordering::SeqCst);
    let name = voice_name(voice);
    let text = format!("Hi, I'm {name}. This is how I'll read your text back to you.");
    let generation = reader.generation.load(Ordering::SeqCst);
    speak(
        app,
        &text,
        Some(voice.to_string()),
        generation,
        Kind::Sample,
    );
}

/// Stops reading, mid-sentence.
pub fn stop() {
    let reader = reader();
    reader.generation.fetch_add(1, Ordering::SeqCst);
    crate::dispatch_reading_pill::hide();
    if let Some(player) = reader.player.lock().ok().and_then(|mut p| p.take()) {
        player.stop();
    }
    reader.busy.store(false, Ordering::SeqCst);
}

/// "bm_george" -> "George".
fn voice_name(voice: &str) -> String {
    let name = voice.split_once('_').map_or(voice, |(_, name)| name);
    let mut chars = name.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

/// What a job is reading.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    /// The user's selected text: it calms George down, never has `[pause]`
    /// marks honored, and shows the reading pill.
    Selection,
    /// One of George's "nothing selected" lines.
    Banter,
    /// A voice sample from the settings page (no pill).
    Sample,
}

/// Queues `text` for the speech thread.
fn speak(app: &AppHandle, text: &str, voice: Option<String>, generation: u64, kind: Kind) {
    let reader = reader();
    let Some((model_path, voices_dir)) = model_files(app) else {
        warn!("Read aloud: George's voice files aren't downloaded yet");
        reader.busy.store(false, Ordering::SeqCst);
        crate::dispatch_reading_pill::show_for(PillState::NeedsVoice, NEEDS_VOICE_FOR);
        return;
    };
    let settings = get_settings(app);
    let job = Job {
        text: text.to_string(),
        voice: voice.unwrap_or(settings.dispatch_read_aloud_voice),
        speed: settings.dispatch_read_aloud_speed,
        output_device: settings.selected_output_device,
        model_path,
        voices_dir,
        generation,
        kind,
    };
    if reader.jobs.send(job).is_err() {
        error!("Read aloud: the speech thread has stopped");
        reader.busy.store(false, Ordering::SeqCst);
    }
}

/// Kokoro's model file and voices folder: the downloaded copy, or in a
/// debug build the repo's `vendor/kokoro/`.
fn model_files(app: &AppHandle) -> Option<(PathBuf, PathBuf)> {
    crate::dispatch_voice_files::installed_paths(app).or_else(dev_copy)
}

#[cfg(debug_assertions)]
fn dev_copy() -> Option<(PathBuf, PathBuf)> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("vendor")
        .join("kokoro");
    let (model, voices) = (dir.join(DEV_MODEL_FILE), dir.join("voices"));
    (model.is_file() && voices.is_dir()).then_some((model, voices))
}

#[cfg(not(debug_assertions))]
fn dev_copy() -> Option<(PathBuf, PathBuf)> {
    None
}

/// A debug build has the repo's copy to fall back on.
pub fn dev_copy_available() -> bool {
    dev_copy().is_some()
}

struct Job {
    text: String,
    voice: String,
    speed: f32,
    output_device: Option<String>,
    model_path: PathBuf,
    voices_dir: PathBuf,
    /// Matches `Reader::generation` until `stop` is called.
    generation: u64,
    kind: Kind,
}

struct Reader {
    jobs: mpsc::Sender<Job>,
    /// Bumped by `stop`; a job only keeps going while its number is current.
    generation: AtomicU64,
    /// Reading, or about to: the next hotkey press stops instead.
    busy: AtomicBool,
    /// What's playing, so `stop` can cut it off at once.
    player: Mutex<Option<Arc<rodio::Player>>>,
    /// George's mood when nothing is selected.
    banter: Mutex<crate::dispatch_banter::Banter>,
}

fn reader() -> &'static Reader {
    static READER: OnceLock<Reader> = OnceLock::new();
    READER.get_or_init(|| {
        let (jobs, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("dispatch-read-aloud".into())
            .spawn(move || run(rx))
            .expect("failed to start the read-aloud thread");
        Reader {
            jobs,
            generation: AtomicU64::new(0),
            busy: AtomicBool::new(false),
            player: Mutex::new(None),
            banter: Mutex::new(Default::default()),
        }
    })
}

/// The speech thread. Kokoro loads on the first job and stays loaded.
fn run(rx: mpsc::Receiver<Job>) {
    let mut engine: Option<(PathBuf, Kokoro)> = None;
    for job in rx {
        let reader = reader();
        if job.generation != reader.generation.load(Ordering::SeqCst) {
            continue; // stopped before it started
        }
        let started = Instant::now();
        if job.kind == Kind::Banter {
            crate::dispatch_reading_pill::show(PillState::NothingSelected);
        }
        let result = play(&mut engine, &job);
        if job.kind != Kind::Sample {
            crate::dispatch_reading_pill::hide();
        }
        match result {
            Ok(()) if job.kind == Kind::Selection => {
                if let Ok(mut banter) = reader.banter.lock() {
                    banter.read_finished(Instant::now());
                }
                crate::dispatch_achievements::note(
                    crate::dispatch_achievements::Moment::ReadAloud(started.elapsed()),
                );
            }
            Ok(()) => {}
            Err(e) => error!("Read aloud failed: {e}"),
        }
        if job.generation == reader.generation.load(Ordering::SeqCst) {
            reader.busy.store(false, Ordering::SeqCst);
        }
    }
}

fn play(engine: &mut Option<(PathBuf, Kokoro)>, job: &Job) -> Result<(), String> {
    let reader = reader();
    let current = || job.generation == reader.generation.load(Ordering::SeqCst);

    if engine.as_ref().map(|(path, _)| path) != Some(&job.model_path) {
        if job.kind == Kind::Selection {
            crate::dispatch_reading_pill::show(PillState::Warming {
                voice: voice_name(&job.voice),
            });
        }
        let started = Instant::now();
        let model = KokoroModel::load(&job.model_path, Device::Cpu, None)
            .map_err(|e| format!("couldn't load Kokoro: {e}"))?;
        let kokoro = Kokoro::new(
            model,
            job.voices_dir.clone(),
            loqui_g2p::OovFallback::Neural,
        );
        info!(
            "Read aloud: Kokoro loaded from {} in {} ms",
            job.model_path.display(),
            started.elapsed().as_millis()
        );
        *engine = Some((job.model_path.clone(), kokoro));
    }
    let Some((_, kokoro)) = engine.as_ref() else {
        return Err("Kokoro isn't loaded".into());
    };

    let voice: Blend = job
        .voice
        .parse()
        .or_else(|_| DEFAULT_VOICE.parse())
        .map_err(|e| format!("bad voice {}: {e}", job.voice))?;
    let speed = job.speed.clamp(*SPEED_RANGE.start(), *SPEED_RANGE.end());

    let mut sink = open_output(job.output_device.as_deref())?;
    sink.log_on_drop(false);
    let player = Arc::new(rodio::Player::connect_new(sink.mixer()));
    if let Ok(mut slot) = reader.player.lock() {
        *slot = Some(player.clone());
    }

    let one_channel = NonZero::new(1u16).expect("1 is not zero");
    let rate = NonZero::new(SAMPLE_RATE).expect("Kokoro's sample rate is not zero");
    let silence =
        |length: Duration| vec![0.0f32; (SAMPLE_RATE as f32 * length.as_secs_f32()) as usize];
    let pause = silence(PAUSE_BETWEEN_CHUNKS);

    let started = Instant::now();
    let mut shown_reading = false;
    for (i, piece) in pieces(&job.text, job.kind != Kind::Selection)
        .into_iter()
        .enumerate()
    {
        if !current() {
            break;
        }
        let Some(chunk) = piece else {
            player.append(rodio::buffer::SamplesBuffer::new(
                one_channel,
                rate,
                silence(LONG_PAUSE),
            ));
            continue;
        };
        let audio = kokoro
            .speak(&chunk, &voice, speed)
            .map_err(|e| format!("Kokoro couldn't speak \"{chunk}\": {e}"))?;
        if i == 0 {
            debug!(
                "Read aloud: first words ready in {} ms",
                started.elapsed().as_millis()
            );
        }
        if !current() {
            break;
        }
        player.append(rodio::buffer::SamplesBuffer::new(one_channel, rate, audio));
        if job.kind == Kind::Selection && !shown_reading {
            shown_reading = true;
            crate::dispatch_reading_pill::show(PillState::Reading {
                voice: voice_name(&job.voice),
            });
        }
        player.append(rodio::buffer::SamplesBuffer::new(
            one_channel,
            rate,
            pause.clone(),
        ));
    }
    while current() && !player.empty() {
        std::thread::sleep(Duration::from_millis(40));
    }
    player.stop();
    if let Ok(mut slot) = reader.player.lock() {
        if slot.as_ref().is_some_and(|p| Arc::ptr_eq(p, &player)) {
            *slot = None;
        }
    }
    Ok(())
}

/// The speakers chosen under Settings > Sound > Output Device, like Handy's
/// feedback sounds, or the default device.
fn open_output(name: Option<&str>) -> Result<rodio::MixerDeviceSink, String> {
    use rodio::DeviceSinkBuilder;
    let named = name
        .filter(|n| !n.eq_ignore_ascii_case("default"))
        .and_then(|name| {
            let found = crate::audio_toolkit::get_cpal_host()
                .output_devices()
                .ok()?
                .find(|d| {
                    crate::audio_toolkit::audio::output_device_name(d).as_deref() == Some(name)
                });
            if found.is_none() {
                warn!("Read aloud: output device '{name}' not found, using the default");
            }
            found
        });
    let builder = match named {
        Some(device) => DeviceSinkBuilder::from_device(device),
        None => DeviceSinkBuilder::from_default_device(),
    }
    .map_err(|e| format!("couldn't open the speakers: {e}"))?;
    builder
        .open_stream()
        .map_err(|e| format!("couldn't open the speakers: {e}"))
}

/// What to play, in order: `Some(chunk)` to speak, `None` for a long pause.
/// With `honor_pauses`, each `[pause]` mark (George's lines) becomes one.
fn pieces(text: &str, honor_pauses: bool) -> Vec<Option<String>> {
    if !honor_pauses {
        return chunks(text).into_iter().map(Some).collect();
    }
    let mut out = Vec::new();
    for (i, part) in text.split(crate::dispatch_banter::PAUSE).enumerate() {
        if i > 0 {
            out.push(None);
        }
        out.extend(chunks(part).into_iter().map(Some));
    }
    out
}

/// Splits text into the pieces spoken one after another: the first sentence
/// alone (so speech starts quickly), then sentences packed up to
/// `MAX_CHUNK_CHARS`. Line breaks end a sentence, and a sentence longer than
/// the limit is cut at a comma or space.
pub fn chunks(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for sentence in sentences(text).into_iter().flat_map(split_long) {
        // The first chunk stays one sentence; later ones fill up.
        let fits = out.len() >= 2
            && out
                .last()
                .is_some_and(|last| last.len() + 1 + sentence.len() <= MAX_CHUNK_CHARS);
        match out.last_mut() {
            Some(last) if fits => {
                last.push(' ');
                last.push_str(&sentence);
            }
            _ => out.push(sentence),
        }
    }
    out
}

fn sentences(text: &str) -> Vec<String> {
    const CLOSERS: [char; 6] = ['"', '\'', ')', ']', '”', '’'];
    let mut out = Vec::new();
    for line in text.lines() {
        let mut current = String::new();
        let mut chars = line.chars().peekable();
        while let Some(c) = chars.next() {
            current.push(c);
            if matches!(c, '.' | '!' | '?') {
                while let Some(&next) = chars.peek() {
                    if !CLOSERS.contains(&next) {
                        break;
                    }
                    current.push(next);
                    chars.next();
                }
                if chars.peek().is_none_or(|n| n.is_whitespace()) {
                    push_trimmed(&mut out, &current);
                    current.clear();
                }
            }
        }
        push_trimmed(&mut out, &current);
    }
    out
}

fn push_trimmed(out: &mut Vec<String>, piece: &str) {
    let piece = piece.split_whitespace().collect::<Vec<_>>().join(" ");
    if !piece.is_empty() {
        out.push(piece);
    }
}

fn split_long(sentence: String) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = sentence.as_str();
    while rest.len() > MAX_CHUNK_CHARS {
        // The last comma, semicolon or space that fits, else a hard cut.
        let fits = |i: usize| i <= MAX_CHUNK_CHARS;
        let cut = rest
            .char_indices()
            .take_while(|(i, _)| fits(*i))
            .filter(|(_, c)| matches!(c, ',' | ';'))
            .map(|(i, c)| i + c.len_utf8())
            .last()
            .or_else(|| {
                rest.char_indices()
                    .take_while(|(i, _)| fits(*i))
                    .filter(|(_, c)| *c == ' ')
                    .map(|(i, _)| i)
                    .last()
            })
            .filter(|&i| i > 0)
            .unwrap_or_else(|| {
                rest.char_indices()
                    .map(|(i, _)| i)
                    .take_while(|&i| fits(i))
                    .last()
                    .filter(|&i| i > 0)
                    .unwrap_or(rest.len())
            });
        let (head, tail) = rest.split_at(cut);
        push_trimmed(&mut out, head);
        rest = tail;
    }
    push_trimmed(&mut out, rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_sentence_is_spoken_alone_then_sentences_are_packed() {
        let text = "Hello there. This is the second one! And a third? Yes.";
        assert_eq!(
            chunks(text),
            vec!["Hello there.", "This is the second one! And a third? Yes."]
        );
    }

    #[test]
    fn line_breaks_end_sentences_and_spaces_are_tidied() {
        assert_eq!(
            chunks("A heading\n\n   some   text\nwithout periods"),
            vec!["A heading", "some text without periods"]
        );
    }

    #[test]
    fn closing_quotes_stay_with_their_sentence() {
        assert_eq!(
            sentences("He said \"stop.\" Then he left. Version 1.5 shipped."),
            vec!["He said \"stop.\"", "Then he left.", "Version 1.5 shipped."]
        );
    }

    #[test]
    fn long_sentences_are_cut_at_commas_or_spaces() {
        let long = format!(
            "{}, {}",
            "word ".repeat(40).trim(),
            "more ".repeat(40).trim()
        );
        let pieces = chunks(&long);
        assert!(pieces.len() >= 2);
        assert!(pieces.iter().all(|p| p.len() <= MAX_CHUNK_CHARS));
        assert!(pieces[0].ends_with(','));
        let unbroken = "x".repeat(MAX_CHUNK_CHARS * 2 + 5);
        let pieces = chunks(&unbroken);
        assert_eq!(pieces.concat(), unbroken);
        assert!(pieces.iter().all(|p| p.len() <= MAX_CHUNK_CHARS));
    }

    #[test]
    fn non_ascii_text_is_cut_on_character_boundaries() {
        let text = "é".repeat(MAX_CHUNK_CHARS);
        assert_eq!(chunks(&text).concat(), text);
    }

    #[test]
    fn pause_marks_become_silences_only_in_georges_lines() {
        let line = "Shall I read you the silence, then? [pause] Moving, wasn't it?";
        assert_eq!(
            pieces(line, true),
            vec![
                Some("Shall I read you the silence, then?".to_string()),
                None,
                Some("Moving, wasn't it?".to_string()),
            ]
        );
        assert!(pieces(line, false).iter().all(Option::is_some));
    }

    #[test]
    fn nothing_to_say_gives_no_chunks() {
        assert!(chunks("  \n \t").is_empty());
    }

    #[test]
    fn voice_names_read_naturally() {
        assert_eq!(voice_name("bm_george"), "George");
        assert_eq!(voice_name("af_heart"), "Heart");
        assert_eq!(voice_name("plain"), "Plain");
    }

    #[test]
    fn default_voice_is_one_kokoro_offers() {
        assert!(is_voice(DEFAULT_VOICE));
        assert!(!is_voice("zf_xiaobei"), "only English voices are offered");
    }

    #[test]
    fn the_hotkey_is_added_to_the_defaults() {
        let settings = crate::settings::get_default_settings();
        assert_eq!(
            settings.bindings[BINDING_ID].current_binding,
            DEFAULT_SHORTCUT
        );
        assert_eq!(settings.dispatch_read_aloud_voice, DEFAULT_VOICE);
        assert_eq!(settings.dispatch_read_aloud_speed, 1.0);
    }
}

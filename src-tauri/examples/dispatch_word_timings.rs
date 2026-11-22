//! Dispatch: try the word-timing pass on real dictations before it runs in
//! the app (HANDOFF.md, "Word timings with Whisper"). For the latest
//! dictations in a history database, runs Parakeet on the saved audio for
//! its word times, lines them up with the saved (Whisper) text, and prints
//! every word with its time and how it got it.
//!
//!     cargo build --no-default-features --example dispatch_word_timings
//!     target/debug/examples/dispatch_word_timings.exe --db <history.db> \
//!         --recordings <recordings dir> --model <parakeet .gguf> [--limit 10] [--quiet]
//!
//! Run with `target/debug` on PATH so the speech engine's DLLs are found.

#[allow(dead_code)] // the tool doesn't use every helper
#[path = "../src/dispatch_word_align.rs"]
mod align;

use std::path::{Path, PathBuf};
use std::time::Instant;

use align::{Precision, SourceWord};
use transcribe_cpp::{Model, RunOptions, TimestampKind};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut db = None;
    let mut recordings = None;
    let mut model_path = None;
    let mut limit = 10usize;
    let mut quiet = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--db" => db = args.next().map(PathBuf::from),
            "--recordings" => recordings = args.next().map(PathBuf::from),
            "--model" => model_path = args.next().map(PathBuf::from),
            "--limit" => limit = args.next().ok_or("--limit needs a number")?.parse()?,
            "--quiet" => quiet = true,
            other => return Err(format!("unknown argument {other}").into()),
        }
    }
    let (db, recordings, model_path) = (
        db.ok_or("--db is required")?,
        recordings.ok_or("--recordings is required")?,
        model_path.ok_or("--model is required")?,
    );

    let conn =
        rusqlite::Connection::open_with_flags(&db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut stmt = conn.prepare(
        "SELECT file_name, transcription_text FROM transcription_history \
         WHERE source_name IS NULL ORDER BY id DESC LIMIT ?1",
    )?;
    let entries: Vec<(String, String)> = stmt
        .query_map([limit as i64], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;

    transcribe_cpp::init_backends_default()?;
    let started = Instant::now();
    let model = Model::load(&model_path)?;
    println!(
        "loaded {} on {} in {} ms, finest timestamps: {:?}",
        model.arch(),
        model.backend(),
        started.elapsed().as_millis(),
        model.capabilities()?.max_timestamp_kind
    );
    let mut session = model.session()?;
    let options = RunOptions {
        timestamps: TimestampKind::Word,
        ..Default::default()
    };

    let (mut aligned, mut estimated, mut total_ms) = (0usize, 0usize, 0u128);
    for (file, text) in &entries {
        let pcm = load_wav(&recordings.join(file))?;
        let t = Instant::now();
        let transcript = session.run(&pcm, &options)?;
        let ms = t.elapsed().as_millis();
        total_ms += ms;
        let source: Vec<SourceWord> = transcript
            .words
            .iter()
            .map(|w| SourceWord {
                text: w.text.clone(),
                start_ms: w.t0_ms,
                end_ms: w.t1_ms,
            })
            .collect();
        let span = align::speech_span(&pcm, 16_000);
        let words = align::align(text, &source, span);
        let a = words
            .iter()
            .filter(|w| w.precision == Precision::Aligned)
            .count();
        aligned += a;
        estimated += words.len() - a;
        println!(
            "\n=== {file} ({:.1} s, Parakeet {ms} ms, {} timed words, {a}/{} aligned)",
            pcm.len() as f64 / 16_000.0,
            source.len(),
            words.len()
        );
        println!("  Whisper:  {text}");
        println!("  Parakeet: {}", transcript.text.trim());
        if !quiet {
            for w in &words {
                let mark = if w.precision == Precision::Aligned {
                    ' '
                } else {
                    '~'
                };
                println!(
                    "   {mark} {:>6.2}-{:<6.2} {}",
                    w.start_ms as f64 / 1000.0,
                    w.end_ms as f64 / 1000.0,
                    w.text
                );
            }
        }
    }
    let n = (aligned + estimated).max(1);
    println!(
        "\n{} dictations: {aligned} of {} words aligned ({:.0}%), {estimated} estimated; Parakeet {} ms in all",
        entries.len(),
        aligned + estimated,
        aligned as f64 * 100.0 / n as f64,
        total_ms
    );
    Ok(())
}

/// 16 kHz mono WAV (what Dispatch records) as f32 samples.
fn load_wav(path: &Path) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
    let mut reader = hound::WavReader::open(path)?;
    let spec = reader.spec();
    if spec.sample_rate != 16_000 || spec.channels != 1 {
        return Err(format!("{}: needs 16 kHz mono", path.display()).into());
    }
    Ok(match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().collect::<Result<_, _>>()?,
        hound::SampleFormat::Int => {
            let scale = (1u32 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.map(|s| s as f32 / scale))
                .collect::<Result<_, _>>()?
        }
    })
}

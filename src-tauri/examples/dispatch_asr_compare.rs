//! Dispatch: run the same recordings through several speech models and print
//! the transcripts side by side, for picking the model that best handles one
//! speaker's voice (HANDOFF.md, "Speech model comparison").
//!
//!     cargo run --release --example dispatch_asr_compare -- \
//!         --clips ../test-clips --vocab "Claude,Claude Code,Dispatch" \
//!         path/to/parakeet.gguf path/to/whisper.gguf ...
//!
//! `--json results.json` also saves every transcript and timing for analysis.
//!
//! Whisper models run twice, without and with the vocabulary list (Dispatch
//! passes custom words to models that accept vocabulary). Other models run
//! once. Uses the same engine as the app (transcribe-cpp, GPU when present).
//! Run it with target/<profile> on PATH so the engine's DLLs are found.

use std::path::{Path, PathBuf};
use std::time::Instant;
use transcribe_cpp::{Model, RunOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut clips_dir = PathBuf::from("test-clips");
    let mut vocabulary: Vec<String> = Vec::new();
    let mut models: Vec<PathBuf> = Vec::new();
    let mut json_out: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--json" => json_out = Some(args.next().ok_or("--json needs a file")?.into()),
            "--clips" => clips_dir = args.next().ok_or("--clips needs a folder")?.into(),
            "--vocab" => {
                vocabulary = args
                    .next()
                    .ok_or("--vocab needs a comma-separated list")?
                    .split(',')
                    .map(|w| w.trim().to_string())
                    .filter(|w| !w.is_empty())
                    .collect()
            }
            _ => models.push(arg.into()),
        }
    }
    if models.is_empty() {
        return Err("pass at least one model .gguf path".into());
    }

    let mut clips: Vec<PathBuf> = std::fs::read_dir(&clips_dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("wav")))
        .collect();
    clips.sort();
    let audio: Vec<(String, Vec<f32>)> = clips
        .iter()
        .map(|p| Ok((name(p), load_wav(p)?)))
        .collect::<Result<_, Box<dyn std::error::Error>>>()?;

    transcribe_cpp::init_backends_default()?;

    // results[clip][column] = (label, text, ms)
    let mut results: Vec<Vec<(String, String, u128)>> = vec![Vec::new(); audio.len()];
    for model_path in &models {
        let started = Instant::now();
        let model = Model::load(model_path)?;
        let whisper = model.arch().to_lowercase().contains("whisper");
        println!(
            "loaded {} ({} on {}) in {} ms, finest timestamps: {:?}",
            name(model_path),
            model.arch(),
            model.backend(),
            started.elapsed().as_millis(),
            model.capabilities()?.max_timestamp_kind
        );
        let mut session = model.session()?;
        let mut variants = vec![("plain", Vec::new())];
        if whisper && !vocabulary.is_empty() {
            variants.push(("+vocab", vocabulary.clone()));
        }
        for (variant, vocab) in &variants {
            let label = if variants.len() > 1 {
                format!("{} {variant}", name(model_path))
            } else {
                name(model_path)
            };
            let options = RunOptions {
                language: whisper.then(|| "en".to_string()),
                vocabulary: vocab.clone(),
                ..Default::default()
            };
            // Warm-up run so the first clip's time isn't inflated.
            session.run(&audio[0].1, &options)?;
            for (i, (_, pcm)) in audio.iter().enumerate() {
                let t = Instant::now();
                let text = session.run(pcm, &options)?.text.trim().to_string();
                results[i].push((label.clone(), text, t.elapsed().as_millis()));
            }
        }
    }

    if let Some(path) = json_out {
        let clips: Vec<serde_json::Value> = audio
            .iter()
            .zip(&results)
            .map(|((clip, pcm), columns)| {
                serde_json::json!({
                    "clip": clip,
                    "seconds": pcm.len() as f64 / 16_000.0,
                    "results": columns.iter().map(|(label, text, ms)| {
                        serde_json::json!({ "model": label, "text": text, "ms": ms })
                    }).collect::<Vec<_>>(),
                })
            })
            .collect();
        std::fs::write(&path, serde_json::to_string_pretty(&clips)?)?;
        println!("saved {}", path.display());
        return Ok(());
    }

    for ((clip, pcm), columns) in audio.iter().zip(&results) {
        println!("\n=== {clip} ({:.1} s) ===", pcm.len() as f64 / 16_000.0);
        for (label, text, ms) in columns {
            println!("[{label}, {ms} ms]\n  {text}");
        }
    }
    Ok(())
}

fn name(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
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

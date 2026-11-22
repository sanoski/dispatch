//! Dispatch test tool: quiets the background in a few dictations and writes
//! before and after copies to listen to, both turned up the way the review
//! window plays them (HANDOFF.md, "Quiet playback"). Also checks the cleaned
//! audio lines up with the recording.
//!
//!     cargo run --release --example dispatch_denoise_check -- <out dir> <recording.wav>...

#[path = "../src/dispatch_denoise.rs"]
mod denoise;

use std::path::{Path, PathBuf};
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("usage: dispatch_denoise_check <out dir> <recording.wav>...");
        std::process::exit(2);
    }
    let out = PathBuf::from(&args[0]);
    std::fs::create_dir_all(&out).expect("out dir");
    for path in &args[1..] {
        let pcm = read(Path::new(path));
        let started = Instant::now();
        let cleaned = denoise::clean(&pcm);
        let took = started.elapsed();
        let name = Path::new(path).file_stem().unwrap().to_string_lossy();
        // The review window's boost: peaks to 0.7, at most 20x.
        let peak = pcm.iter().fold(0.0f32, |p, s| p.max(s.abs()));
        let boost = if peak > 0.0 {
            (0.7 / peak).clamp(1.0, 20.0)
        } else {
            1.0
        };
        write(&out.join(format!("{name} 1 as recorded.wav")), &pcm, boost);
        write(&out.join(format!("{name} 2 cleaned.wav")), &cleaned, boost);
        println!(
            "{name}: {:.1} s of audio cleaned in {} ms, lag {} samples, floor {:.1} -> {:.1} dBFS",
            pcm.len() as f32 / 16_000.0,
            took.as_millis(),
            lag(&pcm, &cleaned),
            floor_db(&pcm),
            floor_db(&cleaned),
        );
    }
}

fn read(path: &Path) -> Vec<f32> {
    let mut reader = hound::WavReader::open(path).expect("wav");
    let spec = reader.spec();
    assert_eq!(spec.sample_rate, 16_000, "expected 16 kHz");
    match spec.sample_format {
        hound::SampleFormat::Int => reader
            .samples::<i16>()
            .step_by(spec.channels as usize)
            .map(|s| s.unwrap() as f32 / 32768.0)
            .collect(),
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .step_by(spec.channels as usize)
            .map(|s| s.unwrap())
            .collect(),
    }
}

fn write(path: &Path, pcm: &[f32], gain: f32) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec).expect("create");
    for s in pcm {
        let v = (s * gain).clamp(-1.0, 1.0);
        writer.write_sample((v * 32767.0) as i16).unwrap();
    }
    writer.finalize().unwrap();
}

/// Where `b` best matches `a`, in samples (0 means lined up).
fn lag(a: &[f32], b: &[f32]) -> i32 {
    (-200i32..=200)
        .max_by(|x, y| {
            let score = |l: i32| -> f32 {
                (0..a.len())
                    .filter_map(|i| {
                        let j = i as i32 + l;
                        (j >= 0 && (j as usize) < b.len()).then(|| a[i] * b[j as usize])
                    })
                    .sum()
            };
            score(*x).total_cmp(&score(*y))
        })
        .unwrap()
}

/// The quietest tenth of 20 ms frames, in dBFS: the background.
fn floor_db(pcm: &[f32]) -> f32 {
    let mut rms: Vec<f32> = pcm
        .chunks(320)
        .map(|c| (c.iter().map(|s| s * s).sum::<f32>() / c.len() as f32).sqrt())
        .collect();
    rms.sort_by(f32::total_cmp);
    20.0 * rms[rms.len() / 10].max(1e-9).log10()
}

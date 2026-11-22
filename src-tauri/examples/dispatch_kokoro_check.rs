//! Dispatch test tool: checks a Kokoro model file works with read aloud, and
//! compares two model files on the same sentence (speed, and how different
//! the audio is). Used to confirm the Hugging Face download matches the
//! model Dispatch was built with (HANDOFF.md, "Read aloud").
//!
//! cargo run --example dispatch_kokoro_check -- <voices dir> <out dir> <model.onnx>...

use std::path::PathBuf;
use std::time::Instant;

use loqui_kokoro::{Blend, Device, Kokoro, KokoroModel, SAMPLE_RATE};

const TEXT: &str = "Hey, just wanted to let you know the build is done. \
    I'll send the release notes tonight once I've checked the last two fixes.";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 3 {
        eprintln!("usage: dispatch_kokoro_check <voices dir> <out dir> <model.onnx>...");
        std::process::exit(2);
    }
    let voices = PathBuf::from(&args[0]);
    let out = PathBuf::from(&args[1]);
    let voice: Blend = "bm_george".parse().expect("voice");
    let mut takes: Vec<Vec<f32>> = Vec::new();
    for (i, model_path) in args[2..].iter().enumerate() {
        let started = Instant::now();
        let model = KokoroModel::load(model_path.as_ref(), Device::Cpu, None).expect("load");
        let kokoro = Kokoro::new(model, voices.clone(), loqui_g2p::OovFallback::Neural);
        let loaded = started.elapsed();
        // Warm up once, then time a second run.
        kokoro.speak(TEXT, &voice, 1.0).expect("speak");
        let started = Instant::now();
        let audio = kokoro.speak(TEXT, &voice, 1.0).expect("speak");
        let took = started.elapsed();
        let seconds = audio.len() as f32 / SAMPLE_RATE as f32;
        println!(
            "{model_path}: loaded in {} ms, {seconds:.1} s of speech in {} ms ({:.1}x real time)",
            loaded.as_millis(),
            took.as_millis(),
            seconds / took.as_secs_f32()
        );
        let wav = out.join(format!("take-{}.wav", i + 1));
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: SAMPLE_RATE,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&wav, spec).expect("wav");
        for s in &audio {
            writer
                .write_sample((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
                .expect("write");
        }
        writer.finalize().expect("finalize");
        takes.push(audio);
    }
    if takes.len() >= 2 {
        let (a, b) = (&takes[0], &takes[1]);
        let n = a.len().min(b.len());
        let rms = |x: &[f32]| (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt();
        let diff: Vec<f32> = (0..n).map(|i| a[i] - b[i]).collect();
        println!(
            "lengths {} vs {} samples; difference {:.4} of the signal's level",
            a.len(),
            b.len(),
            rms(&diff) / rms(&a[..n]).max(1e-9)
        );
    }
}

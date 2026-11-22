//! Dispatch: downloads George's voice (Kokoro's model and the English voice
//! packs) into Dispatch's own data folder, so an installed Dispatch can read
//! aloud on its own (HANDOFF.md, "Read aloud").
//!
//! The files come from Hugging Face's `onnx-community/Kokoro-82M-v1.0-ONNX`,
//! pinned to one revision, and each is checked against its SHA-256 before
//! it's used. That copy is the layout `loqui-kokoro` reads: `model.onnx` and
//! one `voices/<id>.bin` per voice. About 340 MB, once.
//!
//! Files land in `dispatch\kokoro.partial\` first, a `.partial` file at a
//! time (an interrupted download picks up where it stopped), and the folder
//! becomes `dispatch\kokoro\` only when every file has checked out. Nothing
//! is put in Handy's `models` folder.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use log::{info, warn};
use serde::Serialize;
use sha2::{Digest, Sha256};
use specta::Type;
use tauri::{AppHandle, Emitter};

/// The event the Read Aloud page follows, with a [`VoiceFilesStatus`].
pub const EVENT: &str = "dispatch-voice-files";

const REPO: &str = "onnx-community/Kokoro-82M-v1.0-ONNX";
/// The revision the hashes below belong to (2025, Apache-2.0).
const REVISION: &str = "1939ad2a8e416c0acfeecc08a694d14ef25f2231";
pub const MODEL_FILE: &str = "model.onnx";
const MODEL: (&str, &str, u64) = (
    "onnx/model.onnx",
    "8fbea51ea711f2af382e88c833d9e288c6dc82ce5e98421ea61c058ce21a34cb",
    325_532_232,
);
const VOICE_SIZE: u64 = 522_240;
/// The voices Dispatch offers (`loqui_kokoro::ENGLISH_VOICES`) and their
/// SHA-256 at [`REVISION`].
const VOICES: &[(&str, &str)] = &[
    (
        "af_alloy",
        "c4a6b876047fd7fb472edf4ebd63cfac7c3b958a7cae7c106e8f038ca6308c45",
    ),
    (
        "af_aoede",
        "4a004c33430762e2461eedb2013fad808ef4ab3121f5300f554476caf58d8361",
    ),
    (
        "af_bella",
        "f69d836209b78eb8c66e75e3cda491e26ea838a3674257e9d4e5703cbaf55c8b",
    ),
    (
        "af_heart",
        "d583ccff3cdca2f7fae535cb998ac07e9fcb90f09737b9a41fa2734ec44a8f0b",
    ),
    (
        "af_jessica",
        "a240a5e3c15b43563d6e923bdca8ef5613a23471d9b77653694012435df23bd8",
    ),
    (
        "af_kore",
        "9be5221b6a941c04b561959b8ff0b06e809444dcc4ab7e75a7b23606f691819e",
    ),
    (
        "af_nicole",
        "cd2191ab31b914ed7b318416b0e4440fdf392ddad9106a060819aa600a64f59a",
    ),
    (
        "af_nova",
        "18778272caa0d0eebaea251c35fd635f038434f9eee5e691d02a174bd328414f",
    ),
    (
        "af_river",
        "00a2bcf82b1d86e8f19902ede58c65ccf6c0e43b44b7d74fad54e5d8933c9c30",
    ),
    (
        "af_sarah",
        "4409fbc125afabacc615d94db5398d847006a737b0247d6892b7a9a0007a2f0a",
    ),
    (
        "af_sky",
        "4435255c9744f3f31659e0d714ab7689bf65d9e77ec1cce060f083912614f0b9",
    ),
    (
        "am_adam",
        "162b035ed91cfc48b6046982184c645f72edcdd1b82843347f605d7bf7b15716",
    ),
    (
        "am_echo",
        "3968b92c3c4cd1c4416dbded36c13eaa388a90d5788d02a13e4d781f5f8cf3c3",
    ),
    (
        "am_eric",
        "e8b5be17edd1e3636901ce7598baafe2dc8dd8ff707a0c23bf9e461add7e2832",
    ),
    (
        "am_fenrir",
        "c27989f741f7ee34d273a39d8a595cc0837d35f5ced9a29b7cc162614616df43",
    ),
    (
        "am_liam",
        "52403be32fd047c6a44517cb0bcd6b134f2a18baa73e70ef41651e0eab921ade",
    ),
    (
        "am_michael",
        "1d1f21dd8da39c30705cd4c75d039d265e9bc4a2a93ed09bc9e1b1225eb95ba1",
    ),
    (
        "am_onyx",
        "da5d135b424164916d75a68ffb4c2abce3d7d5ccc82dd1ee6cf447ce286145e6",
    ),
    (
        "am_puck",
        "fcf73c989033e9233e0b98713eca600c8c74dcc1614b37009d5450ff4a2274a0",
    ),
    (
        "am_santa",
        "61150cf726ab6c5ed7a99f90a304f91f5a72c00c592e89ec94e5df11c319227a",
    ),
    (
        "bf_alice",
        "08afa6ba24da61ea5e8efa139e5aadc938d83f0a6da5a900adaf763ac1da5573",
    ),
    (
        "bf_emma",
        "669fe0647f9dd04fcab92f1439a40eeb4c8b4ab1f82e4996fe3d918ce4a63b73",
    ),
    (
        "bf_isabella",
        "3754352c4aaa46d17f27654ab7518d65b62ad6163a0f55a5f4330c2da2c4e94f",
    ),
    (
        "bf_lily",
        "5e0ee32ebe64a467124976b14e69590746f1c4ce41a12b587a50c862edfea335",
    ),
    (
        "bm_daniel",
        "6b3194bbceffb746733cbc22c8f593dd44e401a71d53895a2dca891bc595a1e8",
    ),
    (
        "bm_fable",
        "f889083196807b4adb15e9204252165f503b8d33d3982e681c52443c49d798f1",
    ),
    (
        "bm_george",
        "c4b235a4c1f2cd3b939fed08b899ce9385638b763f7b73a59616c4fc9bd6c9bc",
    ),
    (
        "bm_lewis",
        "b8f671cef828c30e66fdf0b0756a76bba58f6bb3398cbbf27058642acbcedb97",
    ),
];

/// How often progress is sent to the page.
const PROGRESS_EVERY: Duration = Duration::from_millis(150);

/// Where the voice files stand, for the Read Aloud page.
#[derive(Serialize, Clone, Debug, Type)]
pub struct VoiceFilesStatus {
    /// Downloaded and checked, in Dispatch's data folder.
    pub installed: bool,
    /// A debug build reading the repo's `vendor/kokoro/` copy instead.
    pub using_dev_copy: bool,
    pub downloading: bool,
    pub downloaded_bytes: f64,
    pub total_bytes: f64,
    /// Why the last download stopped, if it failed.
    pub error: Option<String>,
}

struct Progress {
    downloaded: u64,
    error: Option<String>,
}

static DOWNLOADING: AtomicBool = AtomicBool::new(false);
static CANCEL: AtomicBool = AtomicBool::new(false);
static PROGRESS: Mutex<Progress> = Mutex::new(Progress {
    downloaded: 0,
    error: None,
});

/// One file to fetch: its path in the repo, where it goes, its hash and size.
struct Remote {
    path: String,
    local: String,
    sha256: &'static str,
    size: u64,
}

fn remotes() -> Vec<Remote> {
    let mut list = vec![Remote {
        path: MODEL.0.to_string(),
        local: MODEL_FILE.to_string(),
        sha256: MODEL.1,
        size: MODEL.2,
    }];
    list.extend(VOICES.iter().map(|(id, sha256)| Remote {
        path: format!("voices/{id}.bin"),
        local: format!("voices/{id}.bin"),
        sha256,
        size: VOICE_SIZE,
    }));
    list
}

fn total_bytes() -> u64 {
    MODEL.2 + VOICE_SIZE * VOICES.len() as u64
}

/// `%APPDATA%\com.ironfrostlabs.dispatch\dispatch\kokoro`.
pub fn install_dir(app: &AppHandle) -> Option<PathBuf> {
    Some(
        crate::portable::app_data_dir(app)
            .ok()?
            .join("dispatch")
            .join("kokoro"),
    )
}

/// The downloaded copy, if it's all there: (model file, voices folder).
pub fn installed_paths(app: &AppHandle) -> Option<(PathBuf, PathBuf)> {
    let dir = install_dir(app)?;
    is_complete(&dir).then(|| (dir.join(MODEL_FILE), dir.join("voices")))
}

/// Every file present at its expected size. (Hashes were checked when they
/// were downloaded; checking 325 MB on every read would be slow.)
fn is_complete(dir: &Path) -> bool {
    remotes().iter().all(|r| {
        std::fs::metadata(dir.join(&r.local))
            .map(|m| m.len() == r.size)
            .unwrap_or(false)
    })
}

pub fn status(app: &AppHandle) -> VoiceFilesStatus {
    let installed = installed_paths(app).is_some();
    let (downloaded, error) = PROGRESS
        .lock()
        .map(|p| (p.downloaded, p.error.clone()))
        .unwrap_or((0, None));
    VoiceFilesStatus {
        installed,
        using_dev_copy: !installed && crate::dispatch_tts::dev_copy_available(),
        downloading: DOWNLOADING.load(Ordering::SeqCst),
        downloaded_bytes: downloaded as f64,
        total_bytes: total_bytes() as f64,
        error,
    }
}

/// Starts downloading in the background, unless it already is (or the files
/// are already there). Progress goes out as [`EVENT`].
pub fn start_download(app: &AppHandle) {
    if installed_paths(app).is_some() || DOWNLOADING.swap(true, Ordering::SeqCst) {
        return;
    }
    CANCEL.store(false, Ordering::SeqCst);
    if let Ok(mut p) = PROGRESS.lock() {
        p.error = None;
    }
    send(app);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let result = download_all(&app).await;
        DOWNLOADING.store(false, Ordering::SeqCst);
        match result {
            Ok(()) => info!("Voice files: downloaded and checked"),
            Err(_) if CANCEL.load(Ordering::SeqCst) => info!("Voice files: download cancelled"),
            Err(e) => {
                warn!("Voice files: download failed: {e}");
                if let Ok(mut p) = PROGRESS.lock() {
                    p.error = Some(e);
                }
            }
        }
        send(&app);
    });
}

/// Stops a download. What's downloaded so far is kept for next time.
pub fn cancel_download() {
    CANCEL.store(true, Ordering::SeqCst);
}

fn send(app: &AppHandle) {
    let _ = app.emit(EVENT, status(app));
}

async fn download_all(app: &AppHandle) -> Result<(), String> {
    let dir = install_dir(app).ok_or("no app data folder")?;
    let staging = dir.with_file_name("kokoro.partial");
    std::fs::create_dir_all(staging.join("voices"))
        .map_err(|e| format!("can't create {}: {e}", staging.display()))?;
    let client = reqwest::Client::builder()
        .user_agent("Dispatch")
        .connect_timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;

    let mut done: u64 = 0;
    let mut last_sent = Instant::now() - PROGRESS_EVERY;
    for remote in remotes() {
        let target = staging.join(&remote.local);
        let already = std::fs::metadata(&target)
            .map(|m| m.len() == remote.size)
            .unwrap_or(false)
            && sha256_of(&target).as_deref() == Some(remote.sha256);
        if !already {
            download_one(&client, &remote, &target, |bytes| {
                if let Ok(mut p) = PROGRESS.lock() {
                    p.downloaded = done + bytes;
                }
                if last_sent.elapsed() >= PROGRESS_EVERY {
                    last_sent = Instant::now();
                    send(app);
                }
            })
            .await?;
        }
        done += remote.size;
        if let Ok(mut p) = PROGRESS.lock() {
            p.downloaded = done;
        }
    }

    // Everything checked out: swap the finished folder into place.
    if dir.exists() {
        std::fs::remove_dir_all(&dir)
            .map_err(|e| format!("can't replace {}: {e}", dir.display()))?;
    }
    std::fs::rename(&staging, &dir)
        .map_err(|e| format!("can't move the voice files into place: {e}"))
}

/// Downloads one file to `target`, resuming its `.partial` if there is one,
/// and checks its hash. `progress` gets this file's bytes so far.
async fn download_one(
    client: &reqwest::Client,
    remote: &Remote,
    target: &Path,
    mut progress: impl FnMut(u64),
) -> Result<(), String> {
    let partial = PathBuf::from(format!("{}.partial", target.display()));
    let mut have = std::fs::metadata(&partial).map(|m| m.len()).unwrap_or(0);
    if have >= remote.size {
        let _ = std::fs::remove_file(&partial);
        have = 0;
    }
    let url = format!(
        "https://huggingface.co/{REPO}/resolve/{REVISION}/{}",
        remote.path
    );
    let mut request = client.get(&url);
    if have > 0 {
        request = request.header(reqwest::header::RANGE, format!("bytes={have}-"));
    }
    let response = request
        .send()
        .await
        .map_err(|e| format!("couldn't reach Hugging Face: {e}"))?;
    let resumed = response.status() == reqwest::StatusCode::PARTIAL_CONTENT;
    if !response.status().is_success() {
        return Err(format!(
            "Hugging Face answered {} for {}",
            response.status(),
            remote.path
        ));
    }

    let mut hasher = Sha256::new();
    let mut file = if resumed {
        hash_file_into(&partial, &mut hasher).map_err(|e| e.to_string())?;
        std::fs::OpenOptions::new()
            .append(true)
            .open(&partial)
            .map_err(|e| e.to_string())?
    } else {
        have = 0;
        std::fs::File::create(&partial).map_err(|e| e.to_string())?
    };
    progress(have);

    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        if CANCEL.load(Ordering::SeqCst) {
            return Err("cancelled".into());
        }
        let chunk = chunk.map_err(|e| format!("the download was interrupted: {e}"))?;
        file.write_all(&chunk)
            .map_err(|e| format!("couldn't save the voice files: {e}"))?;
        hasher.update(&chunk);
        have += chunk.len() as u64;
        progress(have);
    }
    file.flush().map_err(|e| e.to_string())?;
    drop(file);

    if have != remote.size {
        return Err(format!(
            "{} stopped at {have} of {} bytes",
            remote.path, remote.size
        ));
    }
    let digest = hex(&hasher.finalize());
    if digest != remote.sha256 {
        let _ = std::fs::remove_file(&partial);
        return Err(format!(
            "{} didn't match its expected checksum",
            remote.path
        ));
    }
    std::fs::rename(&partial, target).map_err(|e| e.to_string())
}

fn hash_file_into(path: &Path, hasher: &mut Sha256) -> std::io::Result<()> {
    let mut file = std::fs::File::open(path)?;
    let mut buffer = vec![0u8; 1 << 20];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            return Ok(());
        }
        hasher.update(&buffer[..read]);
    }
}

fn sha256_of(path: &Path) -> Option<String> {
    let mut hasher = Sha256::new();
    hash_file_into(path, &mut hasher).ok()?;
    Some(hex(&hasher.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_offered_voice_is_downloaded() {
        let pinned: Vec<&str> = VOICES.iter().map(|(id, _)| *id).collect();
        assert_eq!(pinned, loqui_kokoro::ENGLISH_VOICES);
        assert!(pinned.contains(&crate::dispatch_tts::DEFAULT_VOICE));
    }

    #[test]
    fn about_340_mb_in_all() {
        assert_eq!(total_bytes(), 340_154_952);
        assert_eq!(remotes().len(), 29);
    }

    #[test]
    fn a_folder_with_every_file_counts_as_complete() {
        let dir = std::env::temp_dir().join(format!("dispatch-voice-test-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("voices")).unwrap();
        assert!(!is_complete(&dir));
        for r in remotes() {
            let file = std::fs::File::create(dir.join(&r.local)).unwrap();
            file.set_len(r.size).unwrap();
        }
        assert!(is_complete(&dir));
        std::fs::File::create(dir.join("voices/bm_george.bin"))
            .unwrap()
            .set_len(10)
            .unwrap();
        assert!(!is_complete(&dir));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn hashes_are_lowercase_hex() {
        assert_eq!(
            hex(&Sha256::digest(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}

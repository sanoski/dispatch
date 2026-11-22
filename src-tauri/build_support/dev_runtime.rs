//! Put the runtime files the app loads from beside its executable into the
//! cargo profile dir (`target/debug`, `target/release`), so `tauri dev` and a
//! plain `cargo run` start without hand-copying anything.
//!
//! tauri-build already tries to do this with `bundle.resources`, but it finds
//! the profile dir by walking exactly three parents up from `OUT_DIR`. Cargo's
//! newer build-dir layout (cargo 1.100+) nests `OUT_DIR` one level deeper
//! (`target/debug/build/handy/<hash>/out` instead of
//! `target/debug/build/handy-<hash>/out`), so tauri-build copies into
//! `target/debug/build` and the app crashes at launch: STATUS_DLL_NOT_FOUND for
//! the transcribe-cpp DLLs on Windows, then a panic loading the tray icon from
//! `resources/`. This does nothing when tauri-build already found the right dir.

use std::fs;
use std::path::{Path, PathBuf};

pub fn copy_dev_runtime_files() {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR not set"));
    let Some(profile_dir) = profile_dir(&out_dir) else {
        println!(
            "cargo:warning=Could not find the cargo profile dir above {}; runtime files were not copied beside the executable",
            out_dir.display()
        );
        return;
    };

    // Older cargo layout: tauri-build's own copy already landed in the right place.
    if out_dir.ancestors().nth(3) == Some(profile_dir.as_path()) {
        return;
    }

    let manifest_dir =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set"));

    // Mirrors `bundle.resources` in tauri.conf.json / tauri.windows.conf.json.
    let resources_src = manifest_dir.join("resources");
    let resources_dest = profile_dir.join("resources");
    copy_tree(&resources_src, &resources_dest);
    rerun_if_missing(&resources_dest.join("default_settings.json"));

    // Windows loads the transcribe-cpp DLLs from the exe's own directory.
    // (Linux dev builds find them through the rpath set in build.rs.)
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let libs_src = manifest_dir.join("transcribe-libs");
        copy_tree(&libs_src, &profile_dir);
        rerun_if_missing(&profile_dir.join("transcribe.dll"));
    }
}

/// The profile dir is the parent of the `build` dir that holds `OUT_DIR`, in
/// both the old and new layouts.
fn profile_dir(out_dir: &Path) -> Option<PathBuf> {
    out_dir
        .ancestors()
        .find(|dir| dir.file_name().is_some_and(|name| name == "build"))
        .and_then(Path::parent)
        .map(Path::to_path_buf)
}

/// Re-run the build script when a copied file goes missing (cargo treats a
/// missing `rerun-if-changed` path as changed), so deleting the copies and
/// rebuilding restores them. Copies keep their source's modified time, so an
/// existing copy never triggers a rebuild on its own.
fn rerun_if_missing(path: &Path) {
    println!("cargo:rerun-if-changed={}", path.display());
}

fn copy_tree(src: &Path, dest: &Path) {
    let Ok(entries) = fs::read_dir(src) else {
        // Nothing staged (e.g. a static macOS build has no transcribe-libs).
        return;
    };
    if let Err(e) = fs::create_dir_all(dest) {
        println!("cargo:warning=Could not create {}: {e}", dest.display());
        return;
    }
    for entry in entries.flatten() {
        let from = entry.path();
        let to = dest.join(entry.file_name());
        if from.is_dir() {
            copy_tree(&from, &to);
        } else if let Err(e) = copy_file(&from, &to) {
            // Usually a DLL held open by a running copy of the app. The old copy
            // stays in place, so warn instead of failing the build.
            println!(
                "cargo:warning=Could not copy {} to {}: {e} (is the app still running?)",
                from.display(),
                to.display()
            );
        }
    }
}

fn copy_file(from: &Path, to: &Path) -> std::io::Result<()> {
    let from_meta = fs::metadata(from)?;
    if let Ok(to_meta) = fs::metadata(to) {
        if to_meta.len() == from_meta.len() && to_meta.modified().ok() == from_meta.modified().ok()
        {
            return Ok(()); // already up to date
        }
    }
    fs::copy(from, to)?;
    let modified = from_meta.modified()?;
    fs::OpenOptions::new()
        .write(true)
        .open(to)?
        .set_modified(modified)?;
    Ok(())
}

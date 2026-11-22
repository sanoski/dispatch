//! Dispatch's English wording for the tray (included by build.rs).
//!
//! The frontend layers `src/i18n/dispatch-overrides.en.json` over Handy's
//! English strings at runtime (see `src/i18n/dispatch.ts`). The tray's strings
//! are generated at build time instead, so this applies the same overrides to
//! them: error titles and any `tray` keys.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

pub fn apply_english_tray_overrides(translations: &mut BTreeMap<String, serde_json::Value>) {
    let path = Path::new("../src/i18n/dispatch-overrides.en.json");
    println!("cargo:rerun-if-changed={}", path.display());

    let Ok(content) = fs::read_to_string(path) else {
        println!("cargo:warning=Dispatch: {} not found", path.display());
        return;
    };
    let overrides: serde_json::Value =
        serde_json::from_str(&content).expect("dispatch-overrides.en.json is valid JSON");
    let Some(en) = translations.get_mut("en") else {
        return;
    };

    let titles = overrides
        .get("errors")
        .and_then(|errors| errors.as_object())
        .into_iter()
        .flatten()
        .filter(|(key, _)| key.ends_with("Title"));
    let tray = overrides
        .get("tray")
        .and_then(|tray| tray.as_object())
        .into_iter()
        .flatten();
    for (key, value) in titles.chain(tray) {
        en[key] = value.clone();
    }
}

//! Slide, the personal fork, must never present itself as upstream DeckCraft.
#![cfg(not(target_arch = "wasm32"))]

#[test]
fn fork_app_name_is_slide() {
    assert_eq!(deckcraft_ui_egui::APP_NAME, "Slide");
}

use deckcraft_ui_egui::UPDATE_APP;

#[test]
fn fork_app_identity_is_slide() {
    assert_eq!(UPDATE_APP.id, "slide");
    assert_eq!(UPDATE_APP.name, "Slide");
    assert_eq!(UPDATE_APP.developer, "JeongEEE");
    assert_eq!(UPDATE_APP.bundle_id(), "io.github.jeongeee.slide");
    assert_ne!(UPDATE_APP.id, craft_update::suite::DECKCRAFT.id);
    assert_ne!(UPDATE_APP.bundle_id(), craft_update::suite::DECKCRAFT.bundle_id());
}

#[test]
fn fork_never_checks_upstream_releases() {
    assert_eq!(UPDATE_APP.repo, "JeongEEE/deckcraft");
    assert_ne!(UPDATE_APP.repo, craft_update::suite::DECKCRAFT.repo);
}

#[test]
fn fork_update_folders_are_separate_from_upstream() {
    // craft-update derives its per-user data and cache folders from the display name. The app's
    // own settings folder is checked in the desktop app's `settings_dir` tests.
    let paths = craft_update::paths::Paths::for_app(&UPDATE_APP).expect("per-user folders");
    let folder = if cfg!(any(target_os = "macos", target_os = "windows")) { "Slide" } else { "slide" };
    assert!(paths.data.ends_with(folder), "{}", paths.data.display());
    assert!(paths.cache.ends_with(folder), "{}", paths.cache.display());
}

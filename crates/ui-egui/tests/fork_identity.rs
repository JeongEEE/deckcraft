//! Slide, the personal fork, must never present itself as upstream DeckCraft.
#![cfg(not(target_arch = "wasm32"))]

#[test]
fn fork_app_name_is_slide() {
    assert_eq!(deckcraft_ui_egui::APP_NAME, "Slide");
}

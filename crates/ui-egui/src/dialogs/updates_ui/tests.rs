//! The update UI: restart through the quit prompt, the files it opens again, and what the card,
//! prompt and windows show. No test checks the network: the worker's messages are fed by hand.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::{Message, Phase, Runtime, begin, begin_restart, fail, outcome_notice, receive, reopen_paths, tick};
use crate::{Services, SlideApp};
use craft_update::http::Transfer;
use deckcraft_engine::Session;
use egui_kittest::kittest::Queryable;
use serde_json::json;
use std::sync::mpsc::channel;

/// A folder of its own per test, removed afterwards.
struct Folder(std::path::PathBuf);

impl Folder {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("slide-update-reopen-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
    fn path(&self, name: &str) -> String {
        self.0.join(name).to_str().unwrap().to_string()
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn app() -> SlideApp {
    SlideApp::new(Session::with_new(), Services::default())
}

fn edit(a: &mut SlideApp) {
    a.session.execute("shape.insert", &json!({"preset": "rect", "rect": [80.0, 80.0, 100.0, 60.0]})).unwrap();
}

/// A presentation saved at `path`, with no changes since.
fn saved_app(path: &str) -> SlideApp {
    let mut a = app();
    edit(&mut a);
    a.session.execute("file.saveAs", &json!({"path": path})).unwrap();
    assert!(!a.session.active().unwrap().is_dirty());
    a
}

fn dirty(a: &SlideApp) -> bool {
    a.session.documents().iter().any(|d| d.is_dirty())
}

fn ready(a: &mut SlideApp) {
    a.updates_rt.phase = Phase::Ready("0.19.9".into());
    a.updates_rt.visible = true;
}

/// Only the update UI, drawn alone, with the app's interface fonts (the prompt's title is bold).
fn update_harness(a: SlideApp) -> egui_kittest::Harness<'static, SlideApp> {
    egui_kittest::Harness::builder()
        .with_size(egui::vec2(800.0, 480.0))
        .with_step_dt(1.0 / 60.0)
        .with_context_setup(crate::theme::install_fonts)
        .build_ui_state(|ui, a| super::show(a, ui.ctx()), a)
}

/// The whole window, as the app draws it.
fn app_harness(a: SlideApp) -> egui_kittest::Harness<'static, SlideApp> {
    egui_kittest::Harness::builder().with_size(egui::vec2(1440.0, 900.0)).with_step_dt(1.0 / 60.0).build_ui_state(
        |ui, a: &mut SlideApp| {
            let ctx = ui.ctx().clone();
            a.logic(&ctx);
            a.ui(ui);
        },
        a,
    )
}

/// Restart to Update with unsaved work: the quit prompt is up and the restart waits for it.
fn restart_asking(a: SlideApp) -> egui_kittest::Harness<'static, SlideApp> {
    let mut h = app_harness(a);
    h.run_steps(2);
    let ctx = h.ctx.clone();
    begin_restart(h.state_mut(), &ctx);
    h.run_steps(2);
    assert!(!h.state().quit_confirmed, "the unsaved change is asked about first");
    assert!(h.state().dialog.as_ref().is_some_and(|d| d.id == "quit"));
    assert!(h.state().updates_rt.restart_requested);
    h
}

#[test]
fn restart_remembers_every_file_and_opens_the_shown_one_last() {
    let folder = Folder::new();
    let first = folder.path("첫 발표.deckcraft");
    let second = folder.path("둘째.deckcraft");
    let mut a = saved_app(&first);
    a.session.execute("file.new", &json!({})).unwrap();
    a.session.execute("file.saveAs", &json!({"path": second})).unwrap();
    a.session.set_active(0);
    begin_restart(&mut a, &egui::Context::default());
    assert!(a.quit_confirmed, "nothing unsaved: the app quits at once");
    assert!(a.updates_rt.restart_requested);
    assert_eq!(reopen_paths(&a), vec![second, first]);
}

#[test]
fn saving_in_the_quit_prompt_keeps_the_restart_and_the_file_to_open_again() {
    let folder = Folder::new();
    let path = folder.path("보고.deckcraft");
    let mut a = saved_app(&path);
    edit(&mut a);
    let mut h = restart_asking(a);
    h.key_press(egui::Key::Enter);
    h.run_steps(2);
    assert!(h.state().quit_confirmed);
    assert!(h.state().updates_rt.restart_requested, "the restart goes on");
    assert!(!dirty(h.state()), "saved");
    assert_eq!(reopen_paths(h.state()), vec![path]);
}

#[test]
fn saving_a_new_presentation_reopens_it_from_where_save_as_put_it() {
    let folder = Folder::new();
    let path = folder.path("새 발표.deckcraft");
    let picked = path.clone();
    let mut a = SlideApp::new(Session::with_new(), Services { pick_save: Some(Box::new(move |_| Some(picked.clone()))), ..Default::default() });
    edit(&mut a);
    assert!(reopen_paths(&a).is_empty(), "a new presentation has no file yet");
    let mut h = restart_asking(a);
    h.key_press(egui::Key::Enter);
    h.run_steps(2);
    assert!(h.state().quit_confirmed);
    assert_eq!(reopen_paths(h.state()), vec![path.clone()]);
    assert!(std::path::Path::new(&path).is_file());
}

#[test]
fn dont_save_reopens_only_files() {
    let folder = Folder::new();
    let path = folder.path("원본.deckcraft");
    let mut a = saved_app(&path);
    edit(&mut a);
    a.session.execute("file.new", &json!({})).unwrap();
    edit(&mut a);
    let mut h = restart_asking(a);
    h.get_by_label("Don't Save").click();
    h.run_steps(2);
    assert!(h.state().quit_confirmed);
    assert!(h.state().updates_rt.restart_requested);
    assert_eq!(reopen_paths(h.state()), vec![path], "the unsaved new presentation isn't reopened");
}

#[test]
fn cancelled_save_prompt_cancels_update_restart() {
    for how in ["button", "escape"] {
        let folder = Folder::new();
        let path = folder.path("작업.deckcraft");
        let mut a = saved_app(&path);
        edit(&mut a);
        let mut h = restart_asking(a);
        if how == "button" {
            h.get_by_label("Cancel").click();
        } else {
            h.key_press(egui::Key::Escape);
        }
        h.run_steps(3);
        assert!(h.state().dialog.is_none(), "{how}");
        assert!(!h.state().updates_rt.restart_requested, "{how}: Cancel ends the restart");
        assert!(!h.state().quit_confirmed, "{how}");
        assert!(dirty(h.state()), "{how}: the work stays");
        // A later normal quit installs nothing.
        h.state_mut().start_pending_update();
        assert!(!h.state().updates_rt.restart_requested, "{how}");
    }
}

#[test]
fn a_quit_abandoned_without_a_prompt_ends_the_restart() {
    let mut a = app();
    a.updates_rt.restart_requested = true;
    a.updates_rt.restart_active = Some(1);
    tick(&mut a, &egui::Context::default());
    assert!(!a.updates_rt.restart_requested);
    assert!(a.updates_rt.restart_active.is_none());
}

/// Commit 1597f9aa of the Word fork: a check Slide starts itself (at start, every six hours) that
/// fails is only logged.
#[test]
fn unrequested_check_failure_shows_no_window() {
    let mut a = app();
    let (tx, rx) = channel::<Message>();
    a.updates_rt.pending = Some(rx);
    a.updates_rt.phase = Phase::Checking;
    tx.send(Message::Done(Err("GitHub's request limit was reached".into()))).unwrap();
    tick(&mut a, &egui::Context::default());
    assert!(matches!(a.updates_rt.phase, Phase::Idle) && a.updates_rt.pending.is_none());
    assert!(!a.updates_rt.visible);
    let mut h = update_harness(a);
    h.run_steps(3);
    assert!(h.query_by_label("Slide Update").is_none());
    assert!(h.query_by_label("Couldn't prepare the update.").is_none());
}

/// Help ▸ Check for Updates that fails says why, in a window that closes.
#[test]
fn requested_check_failure_shows_window() {
    let mut a = app();
    let (tx, rx) = channel::<Message>();
    a.updates_rt.pending = Some(rx);
    a.updates_rt.phase = Phase::Checking;
    a.updates_rt.requested = true;
    drop(tx);
    tick(&mut a, &egui::Context::default());
    assert!(matches!(a.updates_rt.phase, Phase::Failed(_)) && a.updates_rt.pending.is_none());
    assert!(a.updates_rt.visible);
    let mut h = update_harness(a);
    h.run_steps(3);
    assert!(h.query_by_label("Slide Update").is_some());
    assert!(h.query_by_label("Couldn't prepare the update.").is_some());
    h.get_by_label("Close").click();
    h.run_steps(2);
    assert!(h.query_by_label("Slide Update").is_none());
}

#[test]
fn a_check_at_start_that_finds_nothing_shows_nothing() {
    let mut a = app();
    let (tx, rx) = channel();
    a.updates_rt.pending = Some(rx);
    a.updates_rt.phase = Phase::Checking;
    tx.send(Message::Done(Ok(None))).unwrap();
    tick(&mut a, &egui::Context::default());
    assert!(matches!(a.updates_rt.phase, Phase::Idle) && a.updates_rt.pending.is_none());
    assert!(!a.updates_rt.visible);
}

#[test]
fn a_failed_start_of_the_worker_is_quiet_unless_asked_for() {
    let mut rt = Runtime::default();
    fail(&mut rt, "no thread".into());
    assert!(matches!(rt.phase, Phase::Idle) && !rt.visible);
    rt.requested = true;
    fail(&mut rt, "no thread".into());
    assert!(matches!(rt.phase, Phase::Failed(_)) && rt.visible);
}

#[test]
fn a_check_by_hand_during_the_one_at_start_still_says_slide_is_current() {
    let mut rt = Runtime::default();
    let (_tx, rx) = channel();
    rt.pending = Some(rx);
    begin(&mut rt, &egui::Context::default(), true);
    receive(&mut rt, Message::Done(Ok(None)));
    assert!(rt.visible);
    assert!(matches!(rt.phase, Phase::Current));
}

#[test]
fn help_check_for_updates_asks_for_a_check() {
    let mut a = app();
    a.run("app.checkUpdates", json!({})).unwrap();
    assert!(a.updates_rt.manual);
}

#[test]
fn a_background_download_shows_progress_without_opening_a_window() {
    let mut a = app();
    receive(&mut a.updates_rt, Message::Progress(Transfer::Bytes { done: 25, total: 100 }));
    let mut h = update_harness(a);
    h.run_steps(3);
    assert!(h.query_by_label("Downloading the update").is_some());
    assert!(h.query_by_label("25%").is_some());
    assert!(h.query_by_label("Slide Update").is_none());
    assert!(!h.state().updates_rt.visible);
}

#[test]
fn an_unknown_size_and_a_reconnect_claim_neither_zero_nor_done() {
    let mut a = app();
    receive(&mut a.updates_rt, Message::Progress(Transfer::Bytes { done: 0, total: 0 }));
    let mut h = update_harness(a);
    h.run_steps(3);
    assert!(h.query_by_label("Preparing the update download").is_some());
    assert!(h.query_by_label("0%").is_none());
    receive(&mut h.state_mut().updates_rt, Message::Progress(Transfer::Reconnecting { done: 25, total: 100 }));
    h.run_steps(2);
    assert!(h.query_by_label("Trying to connect again").is_some());
    receive(&mut h.state_mut().updates_rt, Message::Progress(Transfer::Bytes { done: 99_999, total: 100_000 }));
    h.run_steps(2);
    assert!(h.query_by_label("99%").is_some());
    receive(&mut h.state_mut().updates_rt, Message::Progress(Transfer::Bytes { done: 100, total: 100 }));
    h.run_steps(2);
    assert!(h.query_by_label("Verifying the update").is_some());
    assert!(h.query_by_label("100%").is_none());
}

#[test]
fn the_ready_prompt_can_be_put_off_without_quitting_or_changing_the_presentation() {
    let mut a = app();
    edit(&mut a);
    ready(&mut a);
    let mut h = update_harness(a);
    h.run_steps(3);
    assert!(h.query_by_label("Restart to Update").is_some());
    h.get_by_label("Later").click();
    h.run_steps(3);
    assert!(!h.state().updates_rt.visible);
    assert!(!h.state().updates_rt.restart_requested);
    assert!(!h.state().quit_confirmed);
    assert!(dirty(h.state()));
    h.get_by_label("Update ready").click();
    h.run_steps(2);
    assert!(h.query_by_label("Restart to Update").is_some(), "the corner button brings it back");
    assert!(!h.state().updates_rt.restart_requested);
}

#[test]
fn a_successful_install_shows_a_toast() {
    let mut a = app();
    outcome_notice(&mut a, deckcraft_update::helper::Outcome { success: true, version: "0.19.2".into(), message: String::new() });
    assert!(a.updates_rt.install_failed.is_none());
    assert_eq!(a.toast.as_ref().map(|t| t.text.as_str()), Some("Slide was updated to 0.19.2."));
}

#[test]
fn a_failed_install_says_why_until_closed() {
    let mut a = app();
    outcome_notice(&mut a, deckcraft_update::helper::Outcome { success: false, version: "0.19.2".into(), message: "disk full".into() });
    assert!(a.toast.is_none());
    let mut h = update_harness(a);
    h.run_steps(3);
    assert!(h.query_by_label("Couldn't install the update. disk full").is_some());
    h.get_by_label("Close").click();
    h.run_steps(2);
    assert!(h.query_by_label("Couldn't install the update. disk full").is_none());
}

/// The prompt doesn't cover work in progress: it waits, as the corner button, until it ends.
#[test]
fn the_restart_prompt_waits_for_a_dialog_or_the_palette() {
    for kind in ["dialog", "palette"] {
        let mut a = app();
        ready(&mut a);
        if kind == "dialog" {
            a.dialog = Some(crate::dialogs::Dialog::new("about"));
        } else {
            a.palette = Some((String::new(), 0));
        }
        let mut h = update_harness(a);
        h.run_steps(3);
        assert!(h.query_by_label("Restart to Update").is_none(), "{kind}: waits");
        assert!(h.query_by_label("Update ready").is_some(), "{kind}");
        h.state_mut().dialog = None;
        h.state_mut().palette = None;
        h.run_steps(3);
        assert!(h.query_by_label("Restart to Update").is_some(), "{kind}: shown once it ends");
    }
}

#[test]
fn the_restart_prompt_keeps_keys_from_the_slide_and_escape_only_puts_it_off() {
    let mut a = app();
    edit(&mut a);
    let id = a.session.active().unwrap().shapes().last().unwrap().id;
    a.session.execute("edit.select", &json!({"ids": [id.0]})).unwrap();
    let shapes = a.session.active().unwrap().shapes().len();
    ready(&mut a);
    let mut h = app_harness(a);
    h.run_steps(4);
    assert!(h.query_by_label("Restart to Update").is_some());
    assert!(h.state().update_prompt_open());
    h.key_press(egui::Key::Delete);
    h.key_press(egui::Key::Backspace);
    h.key_press(egui::Key::Enter);
    h.run_steps(2);
    assert_eq!(h.state().session.active().unwrap().shapes().len(), shapes, "nothing reached the slide");
    assert!(!h.state().quit_confirmed && !h.state().updates_rt.restart_requested, "Enter didn't restart");
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    assert!(!h.state().updates_rt.visible);
    assert!(h.query_by_label("Update ready").is_some());
    assert!(!h.state().quit_confirmed);
    assert!(!h.state().updates_rt.restart_requested);
    assert!(!h.state().update_prompt_open());
}

//! Software updates. The desktop app looks for a newer fork release (signed `SlideUpdate.json` on
//! JeongEEE/deckcraft, through `deckcraft-update`) at start, from Help ▸ Check for Updates and
//! every six hours, downloads and verifies it in the background, and installs it only after an
//! explicit restart that first goes through the normal quit prompt about unsaved presentations.
//! The files open at the restart open again afterwards. craft-update's own updater is not used:
//! it would look at the upstream releases and install with its own detection.

use crate::SlideApp;
use craft_update::{Release, Stop, http::Transfer};
use deckcraft_update::{download::Prepared, target::Target};
use std::sync::{
    Arc,
    mpsc::{Receiver, TryRecvError, channel},
};
use std::time::{Duration, Instant};

mod view;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const CHECK_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);

#[derive(Default)]
enum Phase {
    #[default]
    Idle,
    Checking,
    Current,
    Downloading {
        done: u64,
        total: u64,
        reconnecting: bool,
    },
    Ready(String),
    /// A newer release this copy can't install itself (no supported install found).
    Manual {
        version: String,
        reason: String,
    },
    Failed(String),
}

enum Message {
    Progress(Transfer),
    Manual(String, String),
    Done(Result<Option<Prepared>, String>),
}

#[derive(Default)]
pub(crate) struct Runtime {
    /// The desktop app turns the check at start on; tests leave it off.
    startup: bool,
    /// Help ▸ Check for Updates asked, to be started on the next frame.
    manual: bool,
    /// The first check ran (and read the last update's outcome).
    started: bool,
    /// The update window or the restart prompt is shown (else only the corner card).
    visible: bool,
    /// The running check was asked for by hand: say so when Slide is current, or why it failed.
    requested: bool,
    next_check: Option<Instant>,
    pending: Option<Receiver<Message>>,
    stop: Arc<Stop>,
    phase: Phase,
    ready: Option<Prepared>,
    /// Why the last update didn't install, until the window saying so is closed.
    install_failed: Option<String>,
    /// The restart prompt drew last frame: the slide takes no keys ([`SlideApp::update_prompt_open`]).
    pub(crate) modal: bool,
    /// Restart to Update was chosen and the quit is under way.
    restart_requested: bool,
    /// The presentation shown when Restart to Update was chosen: it opens last, so it is shown
    /// again (saving in the quit prompt makes each unsaved one active in turn).
    restart_active: Option<u64>,
}

impl Drop for Runtime {
    fn drop(&mut self) {
        self.stop.cancel();
    }
}

/// Turns on the check at start (desktop shell only; tests never touch the network on their own).
pub(crate) fn enable(app: &mut SlideApp) {
    app.updates_rt.startup = true;
}

/// Starts the helper of an update that is ready, once Restart to Update has been chosen and the
/// quit prompt answered, so that it installs once the process has ended. A normal quit installs
/// nothing, and later calls do nothing.
pub(crate) fn start_pending_update(app: &mut SlideApp) {
    let asking = app.dialog.as_ref().is_some_and(|d| d.id == "quit");
    if !std::mem::take(&mut app.updates_rt.restart_requested) || asking {
        return;
    }
    let reopen = reopen_paths(app);
    if let Some(ready) = app.updates_rt.ready.take()
        && let Err(reason) = ready.launch(reopen)
    {
        log::error!("Slide update helper: {reason}");
    }
}

fn check_and_download(stop: &Stop, send: &dyn Fn(Message)) -> Result<Option<Prepared>, String> {
    let http = craft_update::Http::new("Slide/Update")?;
    let Some(release) = craft_update::check_for_update(&crate::UPDATE_APP, &http, VERSION)? else { return Ok(None) };
    let target = match Target::current() {
        Ok(target) => target,
        Err(reason) => {
            send(Message::Manual(release.version_label(), reason));
            return Ok(None);
        }
    };
    send(Message::Progress(Transfer::Bytes { done: 0, total: 0 }));
    download(&release, target, stop, send).map(Some)
}

fn download(release: &Release, target: Target, stop: &Stop, send: &dyn Fn(Message)) -> Result<Prepared, String> {
    let mut last = deckcraft_update::instant().checked_sub(Duration::from_secs(1)).unwrap_or_else(deckcraft_update::instant);
    Prepared::download(release, target, stop, &mut |progress| {
        if last.elapsed() >= Duration::from_millis(100) {
            send(Message::Progress(progress));
            last = deckcraft_update::instant();
        }
    })
}

/// Starts a check (and the download of what it finds) on a thread of its own. The corner card
/// it shows ends with the thread: craft-update's requests have timeouts, a panic is caught, and
/// a thread that goes away without an answer counts as a failure ([`tick`]).
fn begin(rt: &mut Runtime, ctx: &egui::Context, manual: bool) {
    rt.visible |= manual;
    if rt.pending.is_some() {
        rt.requested |= manual;
        return;
    }
    if rt.ready.is_some() {
        return;
    }
    rt.requested = manual;
    rt.phase = Phase::Checking;
    rt.next_check = Some(deckcraft_update::instant() + CHECK_INTERVAL);
    rt.stop = Arc::new(Stop::default());
    let stop = rt.stop.clone();
    let (tx, rx) = channel();
    let repaint = ctx.clone();
    let spawned = std::thread::Builder::new().name("slide-auto-update".into()).spawn(move || {
        let send = |message| {
            let _ = tx.send(message);
            repaint.request_repaint();
        };
        let answer = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| check_and_download(&stop, &send)))
            .unwrap_or_else(|_| Err("Slide's update worker stopped unexpectedly.".into()));
        send(Message::Done(answer));
    });
    match spawned {
        Ok(_) => rt.pending = Some(rx),
        Err(e) => fail(rt, e.to_string()),
    }
}

/// A check failed: always logged, but only a check asked for by hand says so in a window.
/// The ones Slide starts itself stay quiet (offline, rate limits and missing assets would
/// otherwise greet every launch).
fn fail(rt: &mut Runtime, reason: String) {
    log::warn!("Slide auto-update: {reason}");
    if rt.requested {
        rt.phase = Phase::Failed(reason);
        rt.visible = true;
    } else {
        rt.phase = Phase::Idle;
    }
}

fn receive(rt: &mut Runtime, message: Message) {
    match message {
        Message::Progress(Transfer::Bytes { done, total }) => {
            rt.phase = Phase::Downloading { done, total, reconnecting: false };
        }
        Message::Progress(Transfer::Reconnecting { done, total }) => {
            rt.phase = Phase::Downloading { done, total, reconnecting: true };
        }
        Message::Manual(version, reason) => {
            rt.phase = Phase::Manual { version, reason };
            rt.visible = true;
        }
        Message::Done(result) => {
            rt.pending = None;
            match result {
                Ok(Some(ready)) => {
                    rt.phase = Phase::Ready(ready.version.clone());
                    rt.ready = Some(ready);
                    rt.visible = true;
                }
                Ok(None) if matches!(rt.phase, Phase::Manual { .. }) => {}
                Ok(None) => rt.phase = if rt.requested { Phase::Current } else { Phase::Idle },
                Err(reason) => fail(rt, reason),
            }
        }
    }
}

/// A quit is under way: confirmed (the window is closing), or its prompt about unsaved
/// presentations still waiting for an answer.
fn quit_is_pending(app: &SlideApp) -> bool {
    app.quit_confirmed || app.dialog.as_ref().is_some_and(|d| d.id == "quit")
}

/// What the last update came to, read once at the first check: a success shows a toast, a
/// failure a window with the reason.
fn outcome_notice(app: &mut SlideApp, outcome: deckcraft_update::helper::Outcome) {
    if outcome.success {
        app.notify(format!("{} was updated to {}.", crate::APP_NAME, outcome.version));
    } else {
        app.updates_rt.install_failed = Some(outcome.message);
    }
}

/// Help ▸ Check for Updates: runs a check and reports the outcome either way.
pub(crate) fn request_check(app: &mut SlideApp) {
    app.updates_rt.manual = true;
}

pub(crate) fn tick(app: &mut SlideApp, ctx: &egui::Context) {
    // Cancel (or Esc, or a Save that didn't save) in the quit prompt ends the quit: the update
    // waits again.
    if app.updates_rt.restart_requested && !quit_is_pending(app) {
        cancel_restart(app);
    }
    let rt = &mut app.updates_rt;
    let manual = std::mem::take(&mut rt.manual);
    let startup = std::mem::take(&mut rt.startup);
    let periodic = rt.next_check.is_some_and(|at| deckcraft_update::instant() >= at) && rt.ready.is_none() && rt.pending.is_none();
    if manual || (startup && !rt.started) || periodic {
        if !rt.started {
            rt.started = true;
            if let Some(outcome) = deckcraft_update::helper::take_outcome() {
                outcome_notice(app, outcome);
            }
        }
        begin(&mut app.updates_rt, ctx, manual);
    }
    let rt = &mut app.updates_rt;
    while let Some(rx) = &rt.pending {
        match rx.try_recv() {
            Ok(message) => receive(rt, message),
            Err(TryRecvError::Empty) => {
                ctx.request_repaint_after(Duration::from_millis(200));
                break;
            }
            Err(TryRecvError::Disconnected) => {
                receive(rt, Message::Done(Err("Slide's update worker disconnected.".into())));
                break;
            }
        }
    }
    if let Some(at) = rt.next_check {
        ctx.request_repaint_after(at.saturating_duration_since(deckcraft_update::instant()));
    }
}

fn cancel_restart(app: &mut SlideApp) {
    app.updates_rt.restart_requested = false;
    app.updates_rt.restart_active = None;
}

/// The files to open again after the restart, read as the app exits (the quit prompt saves in
/// place, and Save As gives a new presentation its file): each presentation's own, made
/// absolute, once, the one shown at Restart to Update last. One never saved has none.
fn reopen_paths(app: &SlideApp) -> Vec<String> {
    let mut docs: Vec<_> = app.session.documents().iter().map(|d| (Some(d.uid) == app.updates_rt.restart_active, d.path.as_deref())).collect();
    docs.sort_by_key(|(active, _)| *active);
    let mut seen = std::collections::BTreeSet::new();
    docs.into_iter().filter_map(|(_, path)| absolute(path?)).filter(|path| seen.insert(path.clone())).collect()
}

fn absolute(path: &str) -> Option<String> {
    let path = std::path::Path::new(path);
    let path = if path.is_absolute() { path.to_path_buf() } else { std::env::current_dir().ok()?.join(path) };
    path.to_str().map(str::to_string)
}

fn request_restart(app: &mut SlideApp, ctx: &egui::Context) {
    if app.updates_rt.ready.is_none() {
        return;
    }
    begin_restart(app, ctx);
}

/// Quit as the window's close button does: with unsaved presentations, the quit prompt asks
/// about them first (its Cancel ends the restart); with none, the window closes at once.
fn begin_restart(app: &mut SlideApp, ctx: &egui::Context) {
    app.updates_rt.visible = false;
    app.updates_rt.restart_requested = true;
    app.updates_rt.restart_active = app.session.active().map(|d| d.uid);
    if app.session.documents().iter().any(|d| d.is_dirty()) {
        app.show = None;
        app.dialog = Some(crate::dialogs::Dialog::new("quit"));
    } else {
        app.quit_confirmed = true;
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
}

/// The restart prompt waits while something in progress would sit under it: a dialog, the
/// command palette, a slide show, an input method's composition, or a quit.
fn prompt_allowed(app: &SlideApp) -> bool {
    app.dialog.is_none()
        && app.palette.is_none()
        && app.show.is_none()
        && app.session.active().is_none_or(|d| d.composition.is_none())
        && !quit_is_pending(app)
}

pub(crate) fn show(app: &mut SlideApp, ctx: &egui::Context) {
    let allowed = prompt_allowed(app);
    let response = view::show(&mut app.updates_rt, ctx, allowed);
    app.updates_rt.modal = response.modal;
    if response.releases {
        ctx.open_url(egui::OpenUrl::new_tab(crate::UPDATE_APP.releases_url()));
    }
    if response.restart {
        request_restart(app, ctx);
    }
}

#[cfg(test)]
mod tests;

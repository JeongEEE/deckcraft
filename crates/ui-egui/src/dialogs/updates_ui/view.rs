//! Presentation only: a compact background status card, the restart prompt and the update window.
//! The notice once an update has installed is the app's toast.
use super::{Phase, Runtime, VERSION};
use crate::APP_NAME;
use crate::icons::{self, Icon};
use crate::theme::{self, Tokens};
use egui::{Align, Align2, RichText, Sense, Stroke, vec2};

#[derive(Default)]
pub(super) struct Response {
    pub restart: bool,
    pub releases: bool,
    pub modal: bool,
}

fn card_frame(ctx: &egui::Context) -> egui::Frame {
    let t = Tokens::get(ctx);
    egui::Frame::popup(&ctx.global_style()).fill(t.card).stroke(Stroke::new(1.0, t.card_border)).corner_radius(12).inner_margin(16)
}

fn badge(ui: &mut egui::Ui, size: f32) {
    let t = Tokens::get(ui.ctx());
    let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    ui.painter().rect_filled(rect, 10.0, t.selected);
    icons::paint(ui.painter(), rect.shrink(size * 0.2), Icon::Check, t.accent, false);
}

fn progress(ui: &mut egui::Ui, phase: &Phase) {
    let t = Tokens::get(ui.ctx());
    let (title, fraction, subtitle) = match phase {
        Phase::Checking => ("Checking for updates", None, "Looking for a new version".to_string()),
        Phase::Downloading { reconnecting: true, .. } => ("Downloading the update", None, "Trying to connect again".to_string()),
        Phase::Downloading { total: 0, .. } => ("Preparing the update download", None, "You can keep working".to_string()),
        Phase::Downloading { done, total, .. } if done >= total => ("Verifying the update", None, "Checking the downloaded file".to_string()),
        Phase::Downloading { done, total, .. } => (
            "Downloading the update",
            Some((*done as f64 / *total as f64).clamp(0.0, 1.0) as f32),
            format!("{:.1} / {:.1} MB", *done as f64 / 1_000_000.0, *total as f64 / 1_000_000.0),
        ),
        _ => return,
    };
    ui.horizontal(|ui| {
        ui.add(egui::Spinner::new().size(16.0).color(t.accent));
        ui.label(RichText::new(title).strong().color(t.text));
        if let Some(fraction) = fraction {
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                // Never 100 % while the file is still being checked.
                ui.label(RichText::new(format!("{}%", (fraction * 100.0).min(99.0) as u32)).color(t.accent));
            });
        }
    });
    ui.add_space(6.0);
    ui.label(RichText::new(subtitle).small().color(t.text_dim));
    if let Some(fraction) = fraction {
        ui.add_space(8.0);
        ui.add(egui::ProgressBar::new(fraction).desired_height(4.0).fill(t.accent));
    }
}

/// Bottom right, above the status bar: the check or download under way, or the button that
/// brings the restart prompt back.
fn corner(rt: &mut Runtime, ctx: &egui::Context) {
    let ready = matches!(rt.phase, Phase::Ready(_));
    egui::Area::new(egui::Id::unique("slide.update.status"))
        .anchor(Align2::RIGHT_BOTTOM, vec2(-18.0, -44.0))
        .order(egui::Order::Foreground)
        .sense(Sense::hover())
        .interactable(ready)
        .show(ctx, |ui| {
            card_frame(ctx).show(ui, |ui| {
                if ready {
                    let t = Tokens::get(ctx);
                    ui.horizontal(|ui| {
                        badge(ui, 28.0);
                        if ui.button(RichText::new("Update ready").color(t.text)).on_hover_text("Open the restart options").clicked() {
                            rt.visible = true;
                        }
                    });
                } else {
                    ui.set_width(270.0);
                    progress(ui, &rt.phase);
                }
            });
        });
}

/// Restart to Update or Later. Esc and a click outside are Later: the corner button brings it back.
fn restart_prompt(rt: &mut Runtime, ctx: &egui::Context, version: &str, response: &mut Response) {
    let t = Tokens::get(ctx);
    let mut later = false;
    let modal = egui::Modal::new(egui::Id::unique("slide.update.ready")).frame(card_frame(ctx).inner_margin(24)).show(ctx, |ui| {
        ui.set_width(376.0);
        ui.horizontal(|ui| {
            badge(ui, 48.0);
            ui.add_space(6.0);
            ui.vertical(|ui| {
                ui.label(RichText::new("Update ready to install").font(theme::bold(20.0)).color(t.text));
                ui.label(RichText::new(format!("{APP_NAME} {VERSION}  →  {version}")).color(t.text_dim));
            });
        });
        ui.add_space(18.0);
        ui.label(RichText::new(format!("The new version is ready.\nRestart {APP_NAME} now?")).color(t.text));
        ui.add_space(12.0);
        egui::Frame::NONE.fill(t.selected).corner_radius(8).inner_margin(12).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new("Presentations with unsaved changes are asked about first,\nand the open files open again after the restart.")
                    .small()
                    .color(t.text),
            );
        });
        ui.add_space(20.0);
        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
            let restart = egui::Button::new(RichText::new("Restart to Update").strong().color(t.accent_text)).fill(t.accent).corner_radius(7);
            response.restart = ui.add_sized(vec2(178.0, 36.0), restart).clicked();
            later = ui.add_sized(vec2(84.0, 36.0), egui::Button::new("Later").corner_radius(7)).clicked();
        });
    });
    response.modal = true;
    if later || modal.should_close() {
        rt.visible = false;
    }
}

/// Help ▸ Check for Updates, a release this copy can't install, or a failure asked about.
fn update_window(rt: &mut Runtime, ctx: &egui::Context, response: &mut Response) {
    let mut open = true;
    egui::Window::new(format!("{APP_NAME} Update"))
        .id(egui::Id::unique("slide.update.window"))
        .collapsible(false)
        .resizable(false)
        .open(&mut open)
        .show(ctx, |ui| {
            ui.set_max_width(440.0);
            match &rt.phase {
                Phase::Idle => {
                    ui.label("You can check for updates.");
                }
                Phase::Checking | Phase::Downloading { .. } => progress(ui, &rt.phase),
                Phase::Current => {
                    ui.label(format!("{APP_NAME} {VERSION} is up to date."));
                }
                Phase::Manual { version, reason } => {
                    ui.label(format!("{APP_NAME} {version} is available."));
                    ui.label(reason);
                    response.releases = ui.button("Get the installer from the releases page").clicked();
                }
                Phase::Failed(reason) => {
                    ui.label("Couldn't prepare the update.");
                    ui.label(reason);
                    rt.manual |= ui.button("Try Again").clicked();
                    response.releases = ui.button("Download from the releases page").clicked();
                }
                Phase::Ready(_) => {}
            }
            if ui.button("Close").clicked() {
                rt.visible = false;
            }
        });
    rt.visible &= open;
}

/// Why the last update didn't install, until closed.
fn install_failed(rt: &mut Runtime, ctx: &egui::Context) {
    let Some(reason) = &rt.install_failed else { return };
    let mut open = true;
    let mut close = false;
    egui::Window::new(format!("{APP_NAME} Update"))
        .id(egui::Id::unique("slide.update.failed"))
        .collapsible(false)
        .resizable(false)
        .open(&mut open)
        .show(ctx, |ui| {
            ui.set_max_width(440.0);
            ui.label(format!("Couldn't install the update. {reason}"));
            close = ui.button("Close").clicked();
        });
    if close || !open {
        rt.install_failed = None;
    }
}

pub(super) fn show(rt: &mut Runtime, ctx: &egui::Context, prompt_allowed: bool) -> Response {
    install_failed(rt, ctx);
    let mut response = Response::default();
    if !rt.visible || !prompt_allowed {
        if matches!(rt.phase, Phase::Checking | Phase::Downloading { .. } | Phase::Ready(_)) {
            corner(rt, ctx);
        }
        return response;
    }
    if let Phase::Ready(version) = &rt.phase {
        let version = version.clone();
        restart_prompt(rt, ctx, &version, &mut response);
        return response;
    }
    update_window(rt, ctx, &mut response);
    response
}

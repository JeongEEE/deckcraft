use super::{Align2, Color32, CornerRadius, Sense, SlideApp, Stroke, Tokens, Ui, Value, json, run, theme, vec2};
use crate::i18n::{self, fmt};

/// Review › Compare: the revised copy's slide-level changes; click to go there, Accept to take it,
/// Reject to drop it.
pub(super) fn revisions(app: &mut SlideApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let Ok(v) = app.session.execute("review.revisions", &json!({})) else {
        ui.label(egui::RichText::new(i18n::t("Compare this presentation with a revised copy from Review › Compare.")).color(t.text_dim));
        return;
    };
    let changes = v["changes"].as_array().cloned().unwrap_or_default();
    ui.label(egui::RichText::new(fmt(i18n::t("Compared with {name}"), &[("name", v["name"].as_str().unwrap_or("?"))])).color(t.text_dim));
    if v["byPosition"].as_bool() == Some(true) {
        ui.label(egui::RichText::new(i18n::t("Not a copy of this presentation: slides are matched by position.")).size(11.0).color(t.text_faint));
    }
    ui.add_space(4.0);
    if changes.is_empty() {
        ui.label(i18n::t("No differences left."));
        if ui.button(i18n::t("End Review")).clicked() {
            run(app, "review.endCompare", json!({}));
            app.ui.pane = None;
        }
        return;
    }
    ui.horizontal(|ui| {
        if ui.button(fmt(i18n::t("Accept All ({n})"), &[("n", &changes.len().to_string())])).clicked() {
            run(app, "review.accept", json!({"all": true}));
        }
        if ui.button(fmt(i18n::t("Reject All ({n})"), &[("n", &changes.len().to_string())])).clicked() {
            run(app, "review.reject", json!({"all": true}));
        }
    });
    ui.separator();
    for (i, c) in changes.iter().enumerate() {
        let slide = c["slide"].as_u64().unwrap_or(0);
        let kind = c["kind"].as_str().unwrap_or("");
        let (slide_s, number) = (slide.to_string(), slide.saturating_add(1).to_string());
        let (head, col) = match kind {
            "inserted" => (fmt(i18n::t("Slide inserted after {slide}"), &[("slide", &slide_s)]), Color32::from_rgb(0x2E, 0xA3, 0x6F)),
            "deleted" => (fmt(i18n::t("Slide {n} deleted"), &[("n", &number)]), Color32::from_rgb(0xD8, 0x3F, 0x6B)),
            _ => (fmt(i18n::t("Slide {n} changed"), &[("n", &number)]), Color32::from_rgb(0x2E, 0x6F, 0xD8)),
        };
        egui::Frame::NONE.fill(t.chrome).corner_radius(CornerRadius::same(6)).inner_margin(egui::Margin::same(8)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(4.0, 16.0), Sense::hover());
                ui.painter().rect_filled(r, CornerRadius::same(2), col);
                if ui.link(egui::RichText::new(&head).font(theme::bold(12.5))).clicked() {
                    let max = app.session.active().map(|d| d.doc.slides.len().saturating_sub(1)).unwrap_or(0);
                    run(app, "slide.go", json!({"index": (slide as usize).min(max)}));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button(i18n::t("Reject")).clicked() {
                        run(app, "review.reject", json!({"index": i}));
                    }
                    if ui.small_button(i18n::t("Accept")).clicked() {
                        run(app, "review.accept", json!({"index": i}));
                    }
                });
            });
            for d in c["details"].as_array().into_iter().flatten().filter_map(Value::as_str).take(8) {
                // A changed slide's details are the engine's wording (shape names in them stay); an
                // inserted or deleted slide's detail is its title, the document's.
                let d = if kind == "changed" { revision_detail(d) } else { d.into() };
                ui.label(egui::RichText::new(format!("• {d}")).size(11.5).color(t.text_dim));
            }
        });
        ui.add_space(4.0);
    }
}

pub(super) fn comments(app: &mut SlideApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    if ui.button(i18n::t("+ New Comment")).clicked() {
        app.dialog = Some(crate::dialogs::Dialog::new("comment"));
    }
    ui.separator();
    let threads: Vec<(usize, Vec<deckcraft_model::Comment>)> = app
        .session
        .active()
        .map(|st| st.doc.slides.iter().enumerate().filter(|(_, sl)| !sl.comments.is_empty()).map(|(i, sl)| (i, sl.comments.clone())).collect())
        .unwrap_or_default();
    if threads.is_empty() {
        ui.label(egui::RichText::new(i18n::t("No comments.")).color(t.text_dim));
        return;
    }
    let focus = app.ui.comment_focus;
    for (si, list) in threads {
        ui.label(
            egui::RichText::new(fmt(i18n::tc("number", "Slide {n}"), &[("n", &(si + 1).to_string())])).font(theme::bold(12.0)).color(t.text_dim),
        );
        ui.add_space(2.0);
        for (i, c) in list.iter().enumerate() {
            comment_card(app, ui, si, i, c, focus == Some((si, i)));
            ui.add_space(6.0);
        }
    }
}

fn focus_comment(app: &mut SlideApp, slide: usize, index: usize) {
    let _ = app.run("slide.go", json!({"index": slide}));
    app.ui.comment_focus = Some((slide, index));
}

/// Render one comment card, preserving its slide/index identity and focus state through actions.
fn comment_card(app: &mut SlideApp, ui: &mut Ui, slide: usize, index: usize, c: &deckcraft_model::Comment, focused: bool) {
    let t = Tokens::get(ui.ctx());
    let stroke = if focused { Stroke::new(2.0, t.accent) } else { Stroke::NONE };
    let shown = egui::Frame::NONE
        .fill(if focused { t.selected } else { t.chrome })
        .stroke(stroke)
        .corner_radius(CornerRadius::same(8))
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let head = ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::hover());
                ui.painter().circle_filled(r.center(), 12.0, t.accent);
                ui.painter().text(r.center(), Align2::CENTER_CENTER, &c.initials, theme::bold(11.0), Color32::WHITE);
                ui.vertical(|ui| {
                    let name = ui.add(egui::Label::new(egui::RichText::new(&c.author).font(theme::bold(12.5))).sense(Sense::click()));
                    if name.clicked() {
                        focus_comment(app, slide, index);
                    }
                    let when = c.date.replace('T', " ").trim_end_matches('Z').to_string();
                    let when = if c.resolved { fmt(i18n::t("{when} · Resolved"), &[("when", &when)]) } else { when };
                    ui.label(egui::RichText::new(when).size(10.5).color(t.text_faint));
                });
            });
            if head.response.clicked() {
                focus_comment(app, slide, index);
            }
            let body = ui.add(egui::Label::new(&c.text).sense(Sense::click()));
            if body.clicked() {
                focus_comment(app, slide, index);
            }
            for r in &c.replies {
                ui.label(egui::RichText::new(format!("↳ {}: {}", r.author, r.text)).color(t.text_dim));
            }
            // The buttons are laid out first, from the right, so the reply field takes exactly the
            // width they leave. A fixed allowance overflowed the card and widened the pane every
            // frame until it stuck at its widest (#1431).
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("🗑").clicked() {
                        let _ = app.run("slide.go", json!({"index": slide}));
                        let _ = app.run("comment.delete", json!({"index": index}));
                        match app.ui.comment_focus {
                            Some((fs, fi)) if fs == slide && fi == index => app.ui.comment_focus = None,
                            Some((fs, fi)) if fs == slide && fi > index => app.ui.comment_focus = Some((fs, fi - 1)),
                            _ => {}
                        }
                    }
                    if ui.small_button(i18n::t(if c.resolved { "Reopen" } else { "Resolve" })).clicked() {
                        focus_comment(app, slide, index);
                        run(app, "comment.resolve", json!({"index": index, "resolved": !c.resolved}));
                    }
                    let id = ui.scope_id().with(("reply", slide, index));
                    let mut txt: String = ui.data_mut(|d| d.get_temp(id).unwrap_or_default());
                    let r = ui.add(egui::TextEdit::singleline(&mut txt).hint_text(i18n::t("Reply…")).desired_width(ui.available_width()));
                    if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !txt.trim().is_empty() {
                        focus_comment(app, slide, index);
                        run(app, "comment.reply", json!({"index": index, "text": txt}));
                        txt.clear();
                    }
                    ui.data_mut(|d| d.insert_temp(id, txt));
                });
            });
        });
    if focused {
        let key = egui::Id::unique("comment-focus-scrolled");
        let seen = ui.data(|d| d.get_temp::<(usize, usize)>(key));
        if seen != Some((slide, index)) {
            ui.scroll_to_rect(shown.response.rect, Some(egui::Align::Center));
            ui.data_mut(|d| d.insert_temp(key, (slide, index)));
        }
    }
}

/// A changed slide's detail from the engine (deckcraft-engine compare.rs `slide_details`) in the
/// current language. A single word ("layout", "hidden") is a `revision` row (not a plain one: such
/// words look like ids); "<shape name>: text, moved" keeps the name (the text after the last ": "
/// is the list) and translates each word when every one has its row; the sentences with a name in
/// them ("Title 1 deleted (only in the deck)") go through their templates. Otherwise as written.
pub(crate) fn revision_detail(d: &str) -> std::borrow::Cow<'_, str> {
    fn word(w: &str) -> Option<&str> {
        Some(i18n::tc("revision", w)).filter(|k| *k != w)
    }
    if let Some(k) = word(d) {
        return k.into();
    }
    if let Some((label, list)) = d.rsplit_once(": ")
        && let Some(words) = list.split(", ").map(word).collect::<Option<Vec<_>>>()
    {
        return format!("{label}: {}", words.join(", ")).into();
    }
    i18n::t_dynamic(d)
}

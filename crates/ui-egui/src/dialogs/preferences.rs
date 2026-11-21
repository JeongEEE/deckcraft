use super::{Dialog, SlideApp, Ui, Value, json, theme};
use crate::i18n::{self, t};
use crate::widgets;

/// The interface language choices: the `language` setting and its name in the list. "한국어" and
/// "English" are languages' own names, the same in every interface language.
const LANGUAGES: [(&str, &str); 3] = [(i18n::AUTO, "Automatic (system)"), ("ko", "한국어"), ("en", "English")];

/// The Language row (General). The choice is saved in `ui.json` and the interface changes from
/// the next frame (`SlideApp::ui` reads the setting first).
pub(super) fn language_row(app: &mut SlideApp, ui: &mut Ui) {
    let current = LANGUAGES.iter().find(|l| l.0 == app.ui.language).map_or("Automatic (system)", |l| l.1);
    ui.horizontal(|ui| {
        ui.label(t("Language:"));
        widgets::dropdown(ui, "Language", current, 160.0, true, |ui| {
            for (code, name) in LANGUAGES {
                if ui.selectable_label(app.ui.language == code, t(name)).clicked() {
                    app.ui.language = code.to_string();
                    ui.close();
                }
            }
        });
    });
}

/// What the Preferences dialog can change, as it was when the dialog opened.
pub(super) fn preferences_snapshot(app: &SlideApp) -> Value {
    json!({
        "dark": app.ui.brightness == theme::Brightness::Dark,
        "uiScale": app.ui.ui_scale,
        "language": app.ui.language,
        "prefs": serde_json::to_value(&app.session.prefs).unwrap_or(Value::Null),
    })
}

/// Puts back the settings the Preferences dialog opened with (Cancel, Esc, the close button).
pub(super) fn restore_preferences(app: &mut SlideApp, d: &Dialog) {
    let Some(before) = d.params.get("before") else { return };
    if let Some(dark) = before.get("dark").and_then(Value::as_bool)
        && dark != (app.ui.brightness == theme::Brightness::Dark)
    {
        let _ = app.run("view.dark", json!({ "on": dark }));
    }
    if let Some(scale) = before.get("uiScale").and_then(Value::as_f64) {
        app.ui.ui_scale = scale as f32;
    }
    if let Some(language) = before.get("language").and_then(Value::as_str) {
        app.ui.language = language.to_string();
    }
    if let Some(prefs) = before.get("prefs").and_then(|p| serde_json::from_value(p.clone()).ok()) {
        app.session.prefs = prefs;
    }
}

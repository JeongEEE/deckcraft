//! Interface languages: English (the source) and Korean. Strings in the code stay English and
//! are the lookup keys; the catalogs map them to display text as the UI is drawn. Command ids,
//! document text, saved state, egui ids, the control channel, the CLI and MCP never see
//! translated text. A string without a translation shows in English.
//!
//! The catalogs live in this repository, embedded at build time:
//! - `i18n/ko/common.tsv`: a copy of craft-i18n's shared Korean catalog (source commit at its top
//!   and in `ATTRIBUTION.md`), wording common to the Craft apps.
//! - `i18n/ko/deckcraft.tsv`: Slide's own rows, in the same format; they win over shared ones.
//!
//! Lookup, fallback and plurals are the `craft-i18n` crate's (craft-libs).
//!
//! The `language` setting ([`crate::UiState::language`]) is [`AUTO`] by default: the system
//! language when the desktop app reported it ([`set_system_lang`], [`detect_system_lang`]),
//! otherwise English, so tests and other hosts draw English labels on any machine. File ›
//! Options picks one by hand. The current language is per thread and set every frame by
//! `SlideApp::ui`.
//!
//! - [`t`]: a string in the current language. [`tc`]: worded for a context (a tight spot).
//! - [`tn`]: a count's wording (`{n} slide|{n} slides`).
//! - [`fmt`]: fill `{name}` placeholders after a lookup; translators may reorder them.

use std::cell::Cell;
use std::sync::{LazyLock, OnceLock};

use craft_i18n::{I18n, Lang};

pub use craft_i18n::fmt;

/// The embedded catalogs: (locale, shared catalog, Slide's catalog).
static EMBEDDED: [(&str, &str, &str); 1] = [("ko", include_str!("../i18n/ko/common.tsv"), include_str!("../i18n/ko/deckcraft.tsv"))];

static I18N: LazyLock<I18n> = LazyLock::new(|| {
    let (i18n, problems) = I18n::new(&EMBEDDED, &["ko"]);
    problems.iter().for_each(|p| log::warn!("interface catalogs: {p}"));
    i18n
});

/// The system language, once the desktop app has reported it.
static SYSTEM: OnceLock<Lang> = OnceLock::new();

/// The `language` setting that follows the system.
pub const AUTO: &str = "auto";

/// The `language` setting values: [`AUTO`], Korean, English.
pub const SETTINGS: [&str; 3] = [AUTO, "ko", "en"];

/// Record the system's preferred languages (most preferred first, `ko-KR`, `en_US.UTF-8`) for
/// [`AUTO`]. The first call wins.
pub fn set_system_lang(tags: &[String]) {
    let _ = SYSTEM.set(I18N.match_tags(tags.iter().map(String::as_str)));
}

/// Read the system's preferred languages from the operating system (Windows display language,
/// macOS preferred languages, `LANG`/`LC_*` elsewhere) for [`AUTO`]. The desktop app calls this
/// once at start-up; nothing else does, so tests stay in English.
pub fn detect_system_lang() {
    let _ = SYSTEM.set(I18N.system_lang());
}

/// The language for a `language` setting: `ko`, `en`, or the system's for [`AUTO`] and anything
/// unknown (a value from a newer version).
pub fn resolve(pref: &str) -> Lang {
    match pref {
        "en" => Lang::EN,
        "ko" => I18N.from_code("ko").unwrap_or(Lang::EN),
        _ => SYSTEM.get().copied().unwrap_or(Lang::EN),
    }
}

thread_local! {
    // Per thread: parallel test harnesses must not change each other's language.
    static CURRENT: Cell<Lang> = const { Cell::new(Lang::EN) };
}

/// Set the language the UI is drawn in on this thread.
pub fn set_current(lang: Lang) {
    CURRENT.set(lang);
}

/// The language the UI is drawn in on this thread.
pub fn current() -> Lang {
    CURRENT.get()
}

/// Whether the UI is drawn in Korean.
pub fn is_korean() -> bool {
    I18N.code(current()) == "ko"
}

/// `s` in the current language, or `s` itself.
pub fn t(s: &str) -> &str {
    I18N.t(current(), s)
}

/// `s` worded for `context` in the current language, falling back to the plain translation and
/// then to English.
pub fn tc<'a>(context: &str, s: &'a str) -> &'a str {
    I18N.tc(current(), context, s)
}

/// A count's wording in the current language: `one` and `other` are the English forms with
/// `{n}`, and the key of the catalog's `@plural` row. The count is written in plain digits, as
/// `format!` wrote it before, so the English text is unchanged.
pub fn tn(n: u64, one: &str, other: &str) -> String {
    I18N.trn_plain(current(), n, one, other)
}

/// Whether the Korean catalogs have a plain row for `s`.
pub fn has_ko(s: &str) -> bool {
    I18N.from_code("ko").is_some_and(|ko| I18N.get(ko, "", s).is_some())
}

#[cfg(test)]
pub(crate) mod audit;
#[cfg(test)]
mod tests;

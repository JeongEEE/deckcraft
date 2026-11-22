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
//! - [`fmt`]: fill `{name}` placeholders after a lookup; translators may reorder them. No
//!   direction isolates.

use std::cell::Cell;
use std::sync::{LazyLock, OnceLock};

use craft_i18n::{I18n, Lang};

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

/// Fill `{name}` placeholders in `template` (after a lookup; translators may reorder them).
/// Without direction isolates (U+2066–U+2069), unlike `craft_i18n::fmt`: egui lays text out
/// left to right without bidi, so the marks would only change the English text (and could draw
/// as boxes). The text is what `format!` wrote before.
pub fn fmt(template: &str, args: &[(&str, &str)]) -> String {
    craft_i18n::fmt_plain(template, args)
}

/// A message made at run time (an engine error, a status sentence with a file name or a count in
/// it) in the current language. Its plain row if it has one; otherwise the catalogs' rows with
/// `{name}` placeholders are templates: the one whose fixed text matches `s` piece by piece (the
/// longest fixed text when several do) is translated and its values put back in. A value in a
/// wording placeholder ([`WORDING`]) that is itself a row's English (a disabled command's reason
/// in "command `{0}` is not available right now: {1}") is translated too; every other value (a
/// command id, a file or font name) stays as written. Without a matching template the message
/// stays English, whole.
pub fn t_dynamic(s: &str) -> std::borrow::Cow<'_, str> {
    use std::borrow::Cow;
    if current() == Lang::EN {
        return Cow::Borrowed(s);
    }
    let plain = t(s);
    if plain != s {
        return Cow::Borrowed(plain);
    }
    let lang = current();
    let best = TEMPLATES
        .iter()
        .filter(|tpl| I18N.get(lang, "", &tpl.source).is_some())
        .filter_map(|tpl| tpl.capture(s).map(|values| (tpl, values)))
        .max_by_key(|(tpl, _)| tpl.fixed_len);
    let Some((tpl, values)) = best else { return Cow::Borrowed(s) };
    let shown: Vec<(&str, &str)> = values.iter().map(|(name, value)| (*name, if WORDING.contains(name) { t(value) } else { *value })).collect();
    Cow::Owned(fmt(t(&tpl.source), &shown))
}

/// The placeholders whose value is itself the app's or the engine's English wording, so
/// [`t_dynamic`] translates it when it has a row: `{1}` a disabled command's reason or an internal
/// error's message, `{msg}` a parameter error's message, `{e}`/`{err}` an error's sentence
/// ("{name} was not applied: {e}"), `{reason}` an update failure, `{label}` a field's name
/// ("{label} must be a number"), `{colour}` a theme colour slot, `{page}` a File page, `{action}`
/// a command's name. The rest (`{name}`, `{a}`…`{c}` font names, `{path}`, `{0}` ids, counts)
/// are the user's or the document's and stay as written: a font named "Symbol" is not 기호.
const WORDING: [&str; 9] = ["1", "msg", "e", "err", "reason", "label", "colour", "page", "action"];

/// A catalog row with `{name}` placeholders, split into its fixed text and placeholder names.
struct Template {
    source: String,
    /// The fixed text before the first placeholder, then after each one.
    fixed: Vec<String>,
    names: Vec<String>,
    fixed_len: usize,
}

impl Template {
    /// `source` as a template, when it has a placeholder, some fixed text, and fixed text between
    /// every two placeholders (`{a}{b}` can't be told apart).
    fn parse(source: &str) -> Option<Template> {
        let (mut fixed, mut names) = (Vec::new(), Vec::new());
        let mut rest = source;
        while let Some((before, after)) = rest.split_once('{') {
            let (name, after) = after.split_once('}')?;
            fixed.push(before.to_string());
            names.push(name.to_string());
            rest = after;
        }
        fixed.push(rest.to_string());
        let fixed_len = fixed.iter().map(String::len).sum();
        let inner_empty = fixed.get(1..fixed.len().saturating_sub(1)).is_some_and(|m| m.iter().any(String::is_empty));
        (!names.is_empty() && fixed_len > 0 && !inner_empty).then(|| Template { source: source.to_string(), fixed, names, fixed_len })
    }

    /// The values `s` has for the placeholders, when its fixed text matches in order and every
    /// value is non-empty (and the same each time a name repeats).
    fn capture<'s>(&self, s: &'s str) -> Option<Vec<(&str, &'s str)>> {
        let (first, last) = (self.fixed.first()?, self.fixed.last()?);
        let mut rest = s.strip_prefix(first.as_str())?.strip_suffix(last.as_str())?;
        let mut values: Vec<(&str, &'s str)> = Vec::with_capacity(self.names.len());
        for (i, name) in self.names.iter().enumerate() {
            let value = match self.fixed.get(i + 1).filter(|_| i + 1 < self.names.len()) {
                Some(next) => {
                    // The value has at least one character: start looking after it. Before the
                    // last value, the last match: "{name}: {app} can't open…" with a name
                    // "a: b.pptx" splits after the name, not inside it.
                    let skip = rest.chars().next()?.len_utf8();
                    let after = rest.get(skip..)?;
                    let found = if i + 2 == self.names.len() { after.rfind(next.as_str()) } else { after.find(next.as_str()) };
                    let at = found? + skip;
                    let value = rest.get(..at)?;
                    rest = rest.get(at + next.len()..)?;
                    value
                }
                None => std::mem::take(&mut rest),
            };
            if value.is_empty() || values.iter().any(|(n, v)| *n == name.as_str() && *v != value) {
                return None;
            }
            values.push((name, value));
        }
        Some(values)
    }
}

/// The catalogs' plain rows with placeholders, read once. The rows are read as craft-i18n reads
/// them (a leading BOM, `\r\n`, exactly three columns, `\n` `\t` `\\` escapes read left to right);
/// a row it rejected isn't loaded there, so [`t_dynamic`] also skips any template without a
/// translation in the current language.
static TEMPLATES: LazyLock<Vec<Template>> = LazyLock::new(|| {
    let mut seen = std::collections::HashSet::new();
    EMBEDDED
        .iter()
        .flat_map(|(_, common, app)| [app, common])
        .flat_map(|text| text.strip_prefix('\u{feff}').unwrap_or(text).split('\n'))
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| match line.split('\t').collect::<Vec<_>>().as_slice() {
            ["", source, _] if source.contains('{') => unescape(source),
            _ => None,
        })
        .filter(|source| seen.insert(source.clone()))
        .filter_map(|source| Template::parse(&source))
        .collect()
});

/// A catalog column's text: `\n`, `\t` and `\\` undone; `None` for another escape.
fn unescape(text: &str) -> Option<String> {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        out.push(match chars.next()? {
            'n' => '\n',
            't' => '\t',
            '\\' => '\\',
            _ => return None,
        });
    }
    Some(out)
}

/// Names egui itself gives widgets for screen readers (a panel's resize handle, a window's close
/// button), English in egui's code. [`EguiNames`] translates them in each frame's output.
const EGUI_NAMES: [&str; 2] = ["Resize panel", "Close window"];

/// An egui plugin that shows [`EGUI_NAMES`] in the current language in the accessibility tree.
pub struct EguiNames;

impl egui::Plugin for EguiNames {
    fn debug_name(&self) -> &'static str {
        "i18n::EguiNames"
    }

    fn output_hook(&mut self, _ctx: &egui::Context, output: &mut egui::FullOutput) {
        if current() == Lang::EN {
            return;
        }
        let Some(update) = output.platform_output.accesskit_update.as_mut() else { return };
        for (_, node) in &mut update.nodes {
            if let Some(name) = node.label().and_then(|l| EGUI_NAMES.into_iter().find(|n| *n == l)) {
                node.set_label(t(name));
            }
        }
    }
}

/// Whether the Korean catalogs have a plain row for `s`.
pub fn has_ko(s: &str) -> bool {
    I18N.from_code("ko").is_some_and(|ko| I18N.get(ko, "", s).is_some())
}

#[cfg(test)]
pub(crate) mod audit;
#[cfg(test)]
mod tests;

//! Finds the interface text in the UI's source, so a test can check each string has a Korean
//! row (`i18n/ko/*.tsv`) or a reason to stay English (`i18n/keep_english.txt`). It reads the
//! source only; nothing here runs in the app.
//!
//! # Candidate rules
//!
//! A string literal is a candidate when its **text** looks like wording and its **place** in
//! the code is one where the UI shows it.
//!
//! Text: it starts with an ASCII letter and has a space or starts with a capital (`Cancel`,
//! `Interface size:`). Not wording: one word starting lower case (`fill`, `uiScale`), and
//! without spaces a command id or file name (`view.dark`, `a.png`: a `.`), a path (`/`, `\`),
//! a shortcut (`Shift+F9`: a `+`) or a function key (`F5`).
//!
//! Place, from the literal outwards:
//! - an argument of a call that draws text: egui's (`label`, `button`, `checkbox`,
//!   `on_hover_text`, `RichText::new`, `Window::new`, painter `text` … [`DISPLAY`]), the
//!   translation functions (`t`, `tc`, `fmt`, `tn`), or any call whose first or second argument
//!   is a `ui` (the app's widget helpers: `big_button(ui, …)`, `cmd_button(app, ui, …)`);
//! - through `format!`, `Some`, tuples, `unwrap_or` and the like ([`TRANSPARENT`]) to such a
//!   call;
//! - an element of an array or `vec![…]`, a tuple inside one (`[("Fill", …)]`), the value of a
//!   `const`/`static` (except names ending `_ID`/`_KEY`), a `match` arm's value (`=> "Name"`), a
//!   block's only expression (`if a { "Show" } else { "Hide" }`) or a struct field named like a
//!   label (`label: "…"`, `title`, `tip`, `text`, `hint`, `message`, `description` …).
//!
//! Never: attributes (`#[serde(rename = "…")]`), match patterns and comparisons (`"Home" =>`,
//! `== "Home"`), `log::…!`/`println!`/`assert!` and other macros not listed, ids (`id_salt`,
//! `Id::new`, `.with(…)`), map keys and lookups (`get`, `insert`, `contains` …), the `match`
//! arms of a `fn …_name(…) -> &'static str` (`key_name`: engine key ids), a `ui` that is a
//! field (`app.ui`) and test code: `tests.rs`, `*_tests.rs` and items or fields under
//! `#[cfg(test)]`.
//!
//! # Missing
//!
//! [`missing`] lists a candidate without a `keep_english.txt` entry when it has no Korean row
//! (a plain row; a context row only inside `tc(…)`) or when it is **not wrapped**: its nearest
//! drawing call is egui's own (`ui.label("…")`, `.on_hover_text("…")`, `RichText::new("…")`
//! … [`EGUI_METHODS`] as methods, [`EGUI_PATHS`]) with no `t`/`tc`/`tn` in between, so the row
//! is never looked up. `fmt` doesn't count: it only fills placeholders. A block's value
//! (`ui.button(if a { "Show" } else { "Hide" })`, `ui.label(match … { _ => "Name" })`) is
//! judged by the call outside its block. The crate's own widgets (`widgets::small_button(ui, …)`,
//! which translate inside), app helpers with a `ui` argument, arrays, `const`s, `toast` and
//! `set_status` are not judged: only the row has to exist.
//!
//! [`untranslated_helpers`] lists the helpers (`fn`s and `let f = |…|` closures) that take a
//! `ui` and a `&str` and hand it to egui without `t`, directly or captured in a `format!`
//! (`format!("{text}  →")`): rows don't help their callers, only `t(x)` inside them does
//! (`let (label, name) = (t(label), t(name));`, as the shared widgets do). [`problems`] is
//! both lists, what the `korean_coverage_*` tests check.
//!
//! When wrapping the text of `CollapsingHeader::new`, `ui.collapsing`, `ComboBox::from_label`
//! or `Window::new` in `t()`, add `.id_salt(<the English text>)` too: egui makes their id from
//! the text, so a translated label would lose the open state and the saved position.
//!
//! # Known limits
//!
//! Not judged; the on-screen check of Task 8 covers them:
//! - text through a local variable: `let x = "…";` or `let x = if … { "…" } else { "…" };`
//!   then `ui.label(x)`. The literal is a candidate only where its place says so, and only
//!   its row is checked;
//! - helpers taking their wording in an array or slice (`backstage.rs` `bullets`, `dialogs.rs`
//!   `choice`, `photo_album.rs` `album_choice`, `format_shape.rs` `line_choice`'s and
//!   `three_d_combo`'s options): the elements are candidates by the array rule (row only), and
//!   the helper's use of them is not followed.
//!
//! # Measured (2026-11-21, Task 1: only the shared widgets and the tab names translated)
//!
//! `candidate_scan_finds_interface_text_by_area` prints the counts (`--nocapture`; with
//! `I18N_AUDIT_LIST=1` also every problem). Candidates in `src/` (all / no Korean row / not
//! wrapped / untranslating helpers): ribbon 633 / 444 / 160 / 2, dialogs 571 / 377 / 210 / 6,
//! panels 365 / 206 / 117 / 10, backstage 149 / 111 / 26 / 8, canvas 81 / 63 / 32 / 0, show 28 /
//! 21 / 13 / 2, app 15 / 13 / 0 / 0, sorter 13 / 10 / 11 / 0, the top-level files 199 / 109 / 26 /
//! 3 (menus.rs 90 / 41 / 1 / 0, lib.rs 53 / 36 / 0 / 0, presenter.rs 22 / 16 / 12 / 0, status.rs
//! 7 / 3 / 5 / 3, widgets.rs 10 / 0 / 0 / 0), icons 1 / 0 / 0 / 0; 2055 / 1354 / 595 / 31 in all,
//! 1521 candidates missing.

use std::path::Path;

/// One interface string found in the source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    /// The file, relative to the scanned directory.
    pub file: String,
    pub line: usize,
    pub text: String,
    /// Handed straight to an egui call that draws it ([`EGUI_METHODS`], [`EGUI_PATHS`]) with no
    /// `t`/`tc`/`tn` in between: the Korean row, if any, is never looked up.
    pub unwrapped: bool,
    /// Inside a `tc(…)`, where a context row (`menu<TAB>Paste`) counts.
    pub in_tc: bool,
}

/// Calls that draw their text arguments (matched by the last path segment or `Type::new`).
const DISPLAY: &[&str] = &[
    "label",
    "button",
    "small_button",
    "heading",
    "monospace",
    "strong",
    "weak",
    "small",
    "code",
    "hyperlink_to",
    "link",
    "selectable_label",
    "selectable_value",
    "checkbox",
    "radio",
    "radio_value",
    "toggle_value",
    "menu_button",
    "on_hover_text",
    "on_disabled_hover_text",
    "on_hover_text_at_pointer",
    "hint_text",
    "text",
    "show_tooltip_text",
    "collapsing",
    "toast",
    "set_status",
    "selected_text",
    "t",
    "tc",
    "fmt",
    "tn",
    "RichText::new",
    "Button::new",
    "Label::new",
    "Checkbox::new",
    "RadioButton::new",
    "ComboBox::from_label",
    "SelectableLabel::new",
    "Window::new",
    "CollapsingHeader::new",
    "Hyperlink::from_label_and_url",
];

/// egui's own drawing methods (`ui.label(…)`, `.on_hover_text(…)`): a literal reaching one
/// without `t`/`tc`/`tn` shows in English. Only as a method: the crate's free functions with
/// the same names (`widgets::small_button(ui, …)`) translate inside.
const EGUI_METHODS: &[&str] = &[
    "label",
    "button",
    "small_button",
    "heading",
    "monospace",
    "strong",
    "weak",
    "small",
    "code",
    "hyperlink_to",
    "link",
    "selectable_label",
    "selectable_value",
    "checkbox",
    "radio",
    "radio_value",
    "toggle_value",
    "menu_button",
    "on_hover_text",
    "on_disabled_hover_text",
    "on_hover_text_at_pointer",
    "hint_text",
    "collapsing",
    "selected_text",
    "show_tooltip_text",
    // `painter.text(pos, align, "…", font, colour)`, `Slider::text("…")`. The crate's own
    // `.text(…)` (the icon painter's) draws an icon's letters, kept English in keep_english.txt.
    "text",
];

/// egui's constructors and free functions that draw their text argument.
const EGUI_PATHS: &[&str] = &[
    "RichText::new",
    "Button::new",
    "Label::new",
    "Checkbox::new",
    "RadioButton::new",
    "SelectableLabel::new",
    "Window::new",
    "CollapsingHeader::new",
    "ComboBox::from_label",
    "Hyperlink::from_label_and_url",
    "show_tooltip_text",
];

/// The calls that look a string up (`fmt` only fills placeholders, so it is passed through).
const TRANSLATE: &[&str] = &["t", "tc", "tn"];

/// Calls that hand a string on unchanged: look further out.
const TRANSPARENT: &[&str] =
    &["format!", "Some", "Ok", "into", "from", "String::from", "to_string", "to_owned", "unwrap_or", "map_or", "then_some", "Cow::Borrowed"];

/// Calls whose string arguments are never interface text, even with a `ui` argument.
const NEVER: &[&str] = &[
    "id_salt",
    "id_source",
    "Id::new",
    "with",
    "make_persistent_id",
    "push_id",
    "Grid::new",
    "Area::new",
    "get",
    "get_mut",
    "insert",
    "remove",
    "contains",
    "contains_key",
    "starts_with",
    "ends_with",
    "strip_prefix",
    "strip_suffix",
    "eq_ignore_ascii_case",
    "run",
    "find_command",
    "json!",
    "Dialog::new",
    // Errors: the control channel, MCP and scripts read them; the UI translates the ones it
    // shows where it shows them.
    "Err",
    "err",
    "ok_or",
    "ok_or_else",
    "map_err",
    "Error::new",
];

/// Field names whose string value is shown.
const LABEL_FIELDS: &[&str] = &[
    "label",
    "title",
    "tip",
    "tooltip",
    "text",
    "caption",
    "hint",
    "message",
    "msg",
    "desc",
    "description",
    "help",
    "heading",
    "prompt",
    "placeholder",
];

/// The interface strings in `path`: a `.rs` file or a directory (recursively, test files skipped).
pub fn candidates(path: &Path) -> Vec<Candidate> {
    sources(path).iter().flat_map(|(rel, src)| scan_all(rel, src).0).collect()
}

/// The helpers in `path` that take a `ui` and a string argument and hand that argument to egui
/// ([`EGUI_METHODS`], [`EGUI_PATHS`]) without `t`/`tc`/`tn`, directly or captured in a
/// `format!`: `field(ui, d, label: &str, …)` drawing `Label::new(label)`. Their
/// callers' literals then show in English whatever rows exist, so only `t(x)` inside the helper
/// clears one. `text` is `helper(x)`; once per helper and argument.
pub fn untranslated_helpers(path: &Path) -> Vec<Candidate> {
    sources(path).iter().flat_map(|(rel, src)| scan_all(rel, src).1).collect()
}

/// Everything the `korean_coverage_*` tests fail on in `path`, one line each: [`missing`]
/// candidates ([`listing`]) and [`untranslated_helpers`].
pub fn problems(path: &Path) -> Vec<String> {
    let mut out: Vec<String> = listing(&missing(&candidates(path))).lines().map(str::to_string).collect();
    out.extend(untranslated_helpers(path).iter().map(|c| {
        let arg = c.text.split_once('(').map_or("", |(_, a)| a.trim_end_matches(')'));
        format!("  {}:{}: {} passes {arg} to egui untranslated", c.file, c.line, c.text)
    }));
    out
}

/// `(path relative to the scanned directory, source)` of each non-test `.rs` file in `path`.
fn sources(path: &Path) -> Vec<(String, String)> {
    let mut files = Vec::new();
    if path.is_dir() {
        walk(path, &mut files);
    } else {
        files.push(path.to_path_buf());
    }
    files.sort();
    let root = if path.is_dir() { path } else { path.parent().unwrap_or(path) };
    let sources: Vec<(std::path::PathBuf, String)> = files.into_iter().filter_map(|f| Some((f.clone(), std::fs::read_to_string(&f).ok()?))).collect();
    // Files of modules declared `#[cfg(test)] mod name;` (`i18n/audit.rs`).
    let test_files: Vec<std::path::PathBuf> =
        sources.iter().flat_map(|(f, src)| test_modules(src).into_iter().map(move |m| module_file(f, &m))).collect();
    let mut out = Vec::new();
    for (f, src) in sources {
        let name = f.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name == "tests.rs" || name.ends_with("_tests.rs") || test_files.contains(&f) {
            continue;
        }
        let rel = f.strip_prefix(root).unwrap_or(&f).to_string_lossy().replace('\\', "/");
        out.push((rel, src));
    }
    out
}

/// The modules `src` declares as `#[cfg(test)] mod name;` (any visibility).
fn test_modules(src: &str) -> Vec<String> {
    let lines: Vec<&str> = src.lines().map(str::trim).collect();
    lines
        .windows(2)
        .filter(|w| w[0] == "#[cfg(test)]")
        .filter_map(|w| {
            let decl = w[1].strip_suffix(';')?;
            let name = decl.rsplit_once("mod ")?.1;
            name.chars().all(|c| c.is_alphanumeric() || c == '_').then(|| name.to_string())
        })
        .collect()
}

/// The file of module `name` declared in `parent` (`ribbon.rs` → `ribbon/name.rs`;
/// `lib.rs`/`mod.rs` → a sibling `name.rs`).
fn module_file(parent: &Path, name: &str) -> std::path::PathBuf {
    let dir = parent.parent().unwrap_or(parent);
    match parent.file_stem().and_then(|s| s.to_str()) {
        Some("lib" | "mod" | "main") | None => dir.join(format!("{name}.rs")),
        Some(stem) => dir.join(stem).join(format!("{name}.rs")),
    }
}

fn walk(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

/// Candidates without a `keep_english.txt` entry that have no Korean row or are
/// [`Candidate::unwrapped`].
pub fn missing(c: &[Candidate]) -> Vec<Candidate> {
    let keep = keep_english();
    let context_sources = context_sources();
    c.iter().filter(|c| !keep.contains(&c.text) && (c.unwrapped || !has_row(c, &context_sources))).cloned().collect()
}

/// A plain Korean row, or inside `tc(…)` a context row too.
fn has_row(c: &Candidate, context_sources: &std::collections::HashSet<String>) -> bool {
    super::has_ko(&c.text) || (c.in_tc && context_sources.contains(&c.text))
}

/// `file:line: "text" (why)` per candidate, for a failure message.
pub fn listing(c: &[Candidate]) -> String {
    let context_sources = context_sources();
    c.iter()
        .map(|c| {
            let why = if !has_row(c, &context_sources) { "no row" } else { "not wrapped in t()" };
            format!("  {}:{}: {:?} ({why})\n", c.file, c.line, c.text)
        })
        .collect()
}

/// Candidates without a `keep_english.txt` entry that have no Korean row (for the area counts).
pub fn no_row(c: &[Candidate]) -> Vec<Candidate> {
    let keep = keep_english();
    let context_sources = context_sources();
    c.iter().filter(|c| !keep.contains(&c.text) && !has_row(c, &context_sources)).cloned().collect()
}

/// The report area of a file relative to `src/`: its folder, or the file's own name minus a
/// `.rs` for the top-level modules with a folder of the same name (`ribbon.rs` → `ribbon`).
pub fn area(file: &str) -> String {
    let first = file.split('/').next().unwrap_or(file);
    let stem = first.strip_suffix(".rs").unwrap_or(first);
    match stem {
        "ribbon" | "dialogs" | "backstage" | "panels" | "canvas" | "show" | "sorter" | "app" | "i18n" => stem.to_string(),
        _ if file.contains('/') => stem.to_string(),
        _ => format!("top:{stem}"),
    }
}

/// The English text of every `keep_english.txt` entry (`<English>\t<reason>`; `\n` escapes).
pub fn keep_english() -> Vec<String> {
    include_str!("../../i18n/keep_english.txt")
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.split('\t').next().unwrap_or("").replace("\\n", "\n"))
        .collect()
}

/// English texts of the catalogs' context rows (`tc`), which [`super::has_ko`] doesn't see.
fn context_sources() -> std::collections::HashSet<String> {
    [include_str!("../../i18n/ko/common.tsv"), include_str!("../../i18n/ko/deckcraft.tsv")]
        .iter()
        .flat_map(|t| t.lines())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| {
            let mut cols = l.split('\t');
            let context = cols.next()?;
            (!context.is_empty() && !context.starts_with('@')).then(|| cols.next().map(|s| s.replace("\\n", "\n")))?
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Ident(String),
    Punct(char),
    Str(String),
    Other,
}

/// Rust source as tokens with their line: comments dropped, strings unescaped.
fn tokenize(src: &str) -> Vec<(Tok, usize)> {
    let c: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let (mut i, mut line) = (0, 1);
    let at = |k: usize| c.get(k).copied().unwrap_or('\0');
    while i < c.len() {
        let ch = at(i);
        if ch == '\n' {
            line += 1;
            i += 1;
        } else if ch.is_whitespace() {
            i += 1;
        } else if ch == '/' && at(i + 1) == '/' {
            while i < c.len() && at(i) != '\n' {
                i += 1;
            }
        } else if ch == '/' && at(i + 1) == '*' {
            let mut depth = 0;
            while i < c.len() {
                if at(i) == '/' && at(i + 1) == '*' {
                    depth += 1;
                    i += 2;
                } else if at(i) == '*' && at(i + 1) == '/' {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    if at(i) == '\n' {
                        line += 1;
                    }
                    i += 1;
                }
            }
        } else if ch == '"' || ((ch == 'r' || ch == 'b') && raw_or_byte_start(&c, i)) {
            let start_line = line;
            let (text, next) = read_string(&c, i, &mut line);
            out.push((Tok::Str(text), start_line));
            i = next;
        } else if ch == '\'' {
            // A char literal ('a', '\n', '\u{2026}') or a lifetime ('a, 'static).
            if at(i + 1) == '\\' {
                let mut k = i + 2;
                while k < c.len() && at(k) != '\'' {
                    k += 1;
                }
                i = k + 1;
                out.push((Tok::Other, line));
            } else if at(i + 2) == '\'' {
                i += 3;
                out.push((Tok::Other, line));
            } else {
                i += 1;
                while at(i).is_alphanumeric() || at(i) == '_' {
                    i += 1;
                }
                out.push((Tok::Other, line));
            }
        } else if ch.is_alphabetic() || ch == '_' {
            let s = i;
            while at(i).is_alphanumeric() || at(i) == '_' {
                i += 1;
            }
            out.push((Tok::Ident(c[s..i].iter().collect()), line));
        } else if ch.is_ascii_digit() {
            while at(i).is_alphanumeric() || at(i) == '_' || (at(i) == '.' && at(i + 1).is_ascii_digit()) {
                i += 1;
            }
            out.push((Tok::Other, line));
        } else {
            out.push((Tok::Punct(ch), line));
            i += 1;
        }
    }
    out
}

/// `r"`, `r#"`, `b"`, `br"`, `br#"` at `i`.
fn raw_or_byte_start(c: &[char], i: usize) -> bool {
    let at = |k: usize| c.get(k).copied().unwrap_or('\0');
    // Part of a longer identifier (`for"`, `attr`) is not a prefix.
    if i > 0 && (at(i - 1).is_alphanumeric() || at(i - 1) == '_') {
        return false;
    }
    let mut k = i;
    if at(k) == 'b' {
        k += 1;
    }
    if at(k) == 'r' {
        k += 1;
        while at(k) == '#' {
            k += 1;
        }
    }
    k > i && at(k) == '"'
}

/// The string literal starting at `i` (any prefix): its text and the index after it.
fn read_string(c: &[char], mut i: usize, line: &mut usize) -> (String, usize) {
    let at = |k: usize| c.get(k).copied().unwrap_or('\0');
    if at(i) == 'b' {
        i += 1;
    }
    let mut out = String::new();
    if at(i) == 'r' {
        i += 1;
        let mut hashes = 0;
        while at(i) == '#' {
            hashes += 1;
            i += 1;
        }
        i += 1;
        while i < c.len() {
            if at(i) == '"' && (1..=hashes).all(|h| at(i + h) == '#') {
                return (out, i + 1 + hashes);
            }
            if at(i) == '\n' {
                *line += 1;
            }
            out.push(at(i));
            i += 1;
        }
        return (out, i);
    }
    i += 1;
    while i < c.len() {
        match at(i) {
            '"' => return (out, i + 1),
            '\\' => {
                i += 1;
                match at(i) {
                    'n' => out.push('\n'),
                    't' => out.push('\t'),
                    'r' => out.push('\r'),
                    '0' => out.push('\0'),
                    'u' => {
                        let mut k = i + 2;
                        let mut hex = String::new();
                        while k < c.len() && at(k) != '}' {
                            hex.push(at(k));
                            k += 1;
                        }
                        if let Some(ch) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                            out.push(ch);
                        }
                        i = k;
                    }
                    'x' => {
                        let hex: String = [at(i + 1), at(i + 2)].iter().collect();
                        if let Some(ch) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                            out.push(ch);
                        }
                        i += 2;
                    }
                    '\n' => {
                        // A line continuation: the break and the next line's indent are dropped.
                        *line += 1;
                        while at(i + 1).is_whitespace() {
                            if at(i + 1) == '\n' {
                                *line += 1;
                            }
                            i += 1;
                        }
                    }
                    other => out.push(other),
                }
                i += 1;
            }
            ch => {
                if ch == '\n' {
                    *line += 1;
                }
                out.push(ch);
                i += 1;
            }
        }
    }
    (out, i)
}

/// Does `s` read as wording (see the module notes)?
fn looks_like_text(s: &str) -> bool {
    let Some(first) = s.chars().next() else { return false };
    // One letter is a glyph drawn as an icon (`B` for Bold), not wording.
    if !first.is_ascii_alphabetic() || s.chars().nth(1).is_none() {
        return false;
    }
    let spaced = s.contains(' ');
    if !spaced && !first.is_ascii_uppercase() {
        return false;
    }
    if !spaced && (s.contains(['.', '/', '\\', '+']) || (first == 'F' && s.len() <= 3 && s[1..].chars().all(|c| c.is_ascii_digit()))) {
        return false;
    }
    true
}

/// An open bracket and what it belongs to.
struct Frame {
    delim: char,
    /// The call or macro before it (`label`, `RichText::new`, `format!`); `None` for a tuple or
    /// a block.
    callee: Option<String>,
    /// An attribute (`#[…]`).
    attr: bool,
    /// A `ui` is the first or second argument.
    ui_arg: bool,
    /// Another argument is a command id (`("Cut", "edit.cut")`, `QatItem::new("file.new", "New", …)`).
    cmd_arg: bool,
    /// It is a `match` arm's value (`=> Some(…)`, `=> ("Close Path", …)`).
    arm: bool,
    /// The callee is a method (`ui.label(`), not a free function (`label(`, `widgets::label(`).
    method: bool,
    /// The body of a `fn …_name(…) -> &'static str`: its `match` arms give ids
    /// (`key_name`: the engine key names `Session::key` takes), not wording.
    ids: bool,
}

impl Frame {
    fn new(delim: char) -> Frame {
        Frame { delim, callee: None, attr: false, ui_arg: false, cmd_arg: false, arm: false, method: false, ids: false }
    }
}

/// The body of a helper that takes a `ui` and string arguments ([`untranslated_helpers`]).
struct Helper {
    /// Token range of the body.
    body: std::ops::Range<usize>,
    name: String,
    args: Vec<String>,
    /// `let x` inside the body (`let label = t(label);`): `x` is no longer the argument after it.
    shadows: Vec<(usize, String)>,
}

/// The index of the bracket closing the one at `k`.
fn closing(toks: &[(Tok, usize)], k: usize) -> Option<usize> {
    let mut depth = 0i32;
    for (j, (t, _)) in toks.iter().enumerate().skip(k) {
        match t {
            Tok::Punct('(' | '[' | '{') => depth += 1,
            Tok::Punct(')' | ']' | '}') => {
                depth -= 1;
                if depth == 0 {
                    return Some(j);
                }
            }
            _ => {}
        }
    }
    None
}

/// The helpers of a file: `fn`s and `let name = |…|` closures that take a `ui` (`ui`, `*_ui`,
/// `cui`, `sui` or a `Ui` type) and wording arguments (`&str`, `&'static str`, `impl
/// Into<String>`; not an owned `String` or `&mut String`, which carry data or state, and not a
/// `glyph`, an icon's character).
fn helper_scopes(toks: &[(Tok, usize)]) -> Vec<Helper> {
    let mut out = Vec::new();
    for k in 0..toks.len() {
        let (name, open) = match ident(toks.get(k)) {
            Some("fn") => {
                let Some(name) = ident(toks.get(k + 1)) else { continue };
                let Some(open) = (k + 2..(k + 64).min(toks.len())).find(|&j| punct(toks.get(j), '(')) else { continue };
                (name, open)
            }
            Some("let") => {
                let at = if ident(toks.get(k + 1)) == Some("mut") { k + 2 } else { k + 1 };
                let Some(name) = ident(toks.get(at)) else { continue };
                let bar = if ident(toks.get(at + 2)) == Some("move") { at + 3 } else { at + 2 };
                if !punct(toks.get(at + 1), '=') || !punct(toks.get(bar), '|') {
                    continue;
                }
                (name, bar)
            }
            _ => continue,
        };
        let closure = punct(toks.get(open), '|');
        let mut params: Vec<Vec<&Tok>> = vec![Vec::new()];
        let mut depth = 0i32;
        let mut j = open;
        let close = loop {
            j += 1;
            let Some((t, _)) = toks.get(j) else { break None };
            match t {
                Tok::Punct(')' | ']' | '}') if depth == 0 => break Some(j),
                Tok::Punct('|') if closure && depth == 0 => break Some(j),
                Tok::Punct(',') if depth == 0 => params.push(Vec::new()),
                _ => {
                    match t {
                        Tok::Punct('(' | '[' | '{') => depth += 1,
                        Tok::Punct(')' | ']' | '}') => depth -= 1,
                        _ => {}
                    }
                    if let Some(p) = params.last_mut() {
                        p.push(t);
                    }
                }
            }
        };
        let Some(close) = close else { continue };
        let mut has_ui = false;
        let mut args = Vec::new();
        for p in &params {
            let Some(colon) = p.iter().position(|t| **t == Tok::Punct(':')) else { continue };
            let Some(Tok::Ident(arg)) = p[..colon].iter().rev().find(|t| matches!(t, Tok::Ident(s) if s != "mut")) else { continue };
            let ty = &p[colon + 1..];
            let has = |name: &str| ty.iter().any(|t| matches!(t, Tok::Ident(s) if s == name));
            if arg == "ui" || arg.ends_with("_ui") || arg == "cui" || arg == "sui" || has("Ui") {
                has_ui = true;
            } else if (has("str") || (has("String") && has("Into"))) && !has("mut") && arg != "glyph" {
                args.push(arg.clone());
            }
        }
        if !has_ui || args.is_empty() {
            continue;
        }
        // The body: the first `{` outside brackets (`;` first: a declaration only); a closure's
        // may be an expression, up to the `;` (or `,`, `)`, `}`) that ends it.
        let mut b = close;
        let mut depth = 0i32;
        let body = loop {
            b += 1;
            match toks.get(b).map(|t| &t.0) {
                None => break None,
                Some(Tok::Punct('{')) if depth == 0 => break closing(toks, b).map(|e| b..e),
                Some(Tok::Punct(';')) if depth == 0 && !closure => break None,
                Some(Tok::Punct(';' | ',')) if depth == 0 => break Some(close + 1..b),
                Some(Tok::Punct(')' | ']' | '}')) if depth == 0 => break Some(close + 1..b),
                Some(Tok::Punct('(' | '[')) => depth += 1,
                Some(Tok::Punct(')' | ']')) => depth -= 1,
                // A closure's `-> T {`: the block is its body. Any other first token starts an
                // expression body, which brackets are part of.
                _ => {}
            }
        };
        let Some(body) = body else { continue };
        // `let x`, `let mut x`, `let (x, y)` (`let (label, name) = (t(label), t(name));`).
        let shadows = body
            .clone()
            .filter(|&j| ident(toks.get(j)) == Some("let"))
            .flat_map(|j| {
                let at = if ident(toks.get(j + 1)) == Some("mut") { j + 2 } else { j + 1 };
                let names: Vec<String> = if punct(toks.get(at), '(') {
                    let end = closing(toks, at).unwrap_or(at);
                    (at + 1..end).filter_map(|i| ident(toks.get(i)).filter(|n| !matches!(*n, "mut" | "ref")).map(str::to_string)).collect()
                } else {
                    ident(toks.get(at)).map(str::to_string).into_iter().collect()
                };
                names.into_iter().map(move |n| (j, n))
            })
            .collect();
        out.push(Helper { body, name: name.to_string(), args, shadows });
    }
    out
}

fn ident(t: Option<&(Tok, usize)>) -> Option<&str> {
    match t {
        Some((Tok::Ident(s), _)) => Some(s),
        _ => None,
    }
}

fn punct(t: Option<&(Tok, usize)>, ch: char) -> bool {
    matches!(t, Some((Tok::Punct(p), _)) if *p == ch)
}

/// The callee before the open bracket at `k`: `name`, `name!` or `Type::name`.
fn callee_at(toks: &[(Tok, usize)], k: usize) -> Option<String> {
    let before = |n: usize| k.checked_sub(n).and_then(|j| toks.get(j));
    if punct(before(1), '!')
        && let Some(name) = ident(before(2))
    {
        return Some(format!("{name}!"));
    }
    let name = ident(before(1))?;
    if matches!(name, "if" | "match" | "while" | "for" | "in" | "return" | "else" | "loop" | "move") {
        return None;
    }
    if punct(before(2), ':')
        && punct(before(3), ':')
        && let Some(ty) = ident(before(4))
    {
        return Some(format!("{ty}::{name}"));
    }
    Some(name.to_string())
}

/// Whether the callee before the open bracket at `k` is called as a method (`ui.label(`).
fn method_at(toks: &[(Tok, usize)], k: usize) -> bool {
    k >= 2 && ident(toks.get(k - 1)).is_some() && punct(toks.get(k - 2), '.')
}

/// Whether the `{` at `k` opens a `fn …_name(…) -> &'static str` body (see [`Frame::ids`]).
fn id_fn_body(toks: &[(Tok, usize)], k: usize) -> bool {
    let mut j = k;
    while j > 0 {
        j -= 1;
        match toks.get(j) {
            Some((Tok::Punct(';' | '{' | '}'), _)) | None => return false,
            Some((Tok::Ident(s), _)) if s == "fn" => {
                let named = ident(toks.get(j + 1)).is_some_and(|n| n.ends_with("_name"));
                // `-> & 'static str {`: the lifetime is one `Other` token.
                let returns = k >= 4
                    && punct(toks.get(k - 4), '>')
                    && punct(toks.get(k - 3), '&')
                    && matches!(toks.get(k - 2), Some((Tok::Other, _)))
                    && ident(toks.get(k - 1)) == Some("str");
                return named && returns;
            }
            _ => {}
        }
    }
    false
}

/// Whether the arguments after the `(` at `k` start with a `ui` (first or second argument).
/// A field (`app.ui`) is not one.
fn ui_argument(toks: &[(Tok, usize)], k: usize) -> bool {
    let mut args = 0;
    let mut depth = 0;
    for (i, (t, _)) in toks.iter().enumerate().skip(k + 1) {
        match t {
            Tok::Ident(_) if punct(toks.get(i - 1), '.') => {}
            Tok::Punct('(' | '[' | '{') => depth += 1,
            Tok::Punct(')' | ']' | '}') if depth == 0 => return false,
            Tok::Punct(')' | ']' | '}') => depth -= 1,
            Tok::Punct(',') if depth == 0 => {
                args += 1;
                if args >= 2 {
                    return false;
                }
            }
            Tok::Ident(s) if depth == 0 && (s == "ui" || s.ends_with("_ui") || s == "cui" || s == "sui") => return true,
            _ => {}
        }
    }
    false
}

/// Whether one of the arguments after the `(` at `k` is a command id literal (`edit.cut`).
fn command_argument(toks: &[(Tok, usize)], k: usize) -> bool {
    let mut depth = 0;
    for (t, _) in toks.iter().skip(k + 1) {
        match t {
            Tok::Punct('(' | '[' | '{') => depth += 1,
            Tok::Punct(')' | ']' | '}') if depth == 0 => return false,
            Tok::Punct(')' | ']' | '}') => depth -= 1,
            Tok::Str(s) if depth == 0 && is_command_id(s) => return true,
            _ => {}
        }
    }
    false
}

/// `edit.cut`, `view.gridOptions`, `app.dialog`: lower-case words joined by dots.
fn is_command_id(s: &str) -> bool {
    let mut parts = s.split('.');
    let first = parts.next().unwrap_or("");
    let rest: Vec<&str> = parts.collect();
    !rest.is_empty()
        && first.chars().next().is_some_and(|c| c.is_ascii_lowercase())
        && std::iter::once(first).chain(rest.iter().copied()).all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_alphanumeric()))
}

/// Whether the bracket at `k` (after its callee's path, if any) follows a `=>`.
fn after_arrow(toks: &[(Tok, usize)], k: usize) -> bool {
    let mut j = k;
    while j > 0 && matches!(toks.get(j - 1), Some((Tok::Ident(_) | Tok::Punct(':' | '!'), _))) {
        j -= 1;
    }
    j >= 2 && punct(toks.get(j - 1), '>') && punct(toks.get(j - 2), '=')
}

fn is_name(callee: &str, list: &[&str]) -> bool {
    let last = callee.rsplit("::").next().unwrap_or(callee);
    list.contains(&callee) || (!callee.ends_with("::new") && list.contains(&last))
}

/// The candidates of one file's source; `file` names it in the results.
#[cfg(test)]
fn scan(file: &str, src: &str) -> Vec<Candidate> {
    scan_all(file, src).0
}

/// The candidates and the [`untranslated_helpers`] of one file's source.
fn scan_all(file: &str, src: &str) -> (Vec<Candidate>, Vec<Candidate>) {
    let toks = tokenize(src);
    let mut out = Vec::new();
    let mut helpers: Vec<Candidate> = Vec::new();
    let mut stack: Vec<Frame> = Vec::new();
    let scopes = helper_scopes(&toks);
    // While skipping a `#[cfg(test)]` item: the stack depth it started at, whether it has opened
    // its body yet, and whether it is an item (`fn`, `mod` …) rather than a struct field.
    let mut skip: Option<(usize, bool, bool)> = None;
    let mut cfg_test_pending = false;
    let get = |j: usize| toks.get(j);
    for k in 0..toks.len() {
        let (tok, line) = &toks[k];
        let depth = stack.len();
        if let Some((d, opened, item)) = skip {
            match tok {
                Tok::Punct('(' | '[' | '{') => {
                    if *tok == Tok::Punct('{') && depth == d {
                        skip = Some((d, true, item));
                    }
                    stack.push(Frame::new('{'));
                }
                Tok::Punct(')' | ']' | '}') => {
                    stack.pop();
                    // The last field of a struct or struct literal ends with its `}`.
                    if stack.len() < d || (stack.len() == d && opened && *tok == Tok::Punct('}')) {
                        skip = None;
                    }
                }
                Tok::Punct(';') if depth == d && !opened => skip = None,
                // A field (`#[cfg(test)] spawns: usize,`, `S { #[cfg(test)] spawns: 0, … }`).
                Tok::Punct(',') if depth == d && !opened && !item => skip = None,
                Tok::Ident(s)
                    if depth == d
                        && matches!(
                            s.as_str(),
                            "fn" | "mod" | "impl" | "struct" | "enum" | "trait" | "use" | "const" | "static" | "type" | "let" | "extern" | "union"
                        ) =>
                {
                    skip = Some((d, opened, true));
                }
                _ => {}
            }
            continue;
        }
        match tok {
            Tok::Punct('(' | '[' | '{') => {
                let delim = match tok {
                    Tok::Punct(c) => *c,
                    _ => '(',
                };
                let before = |n: usize| k.checked_sub(n).and_then(get);
                let attr = delim == '[' && (punct(before(1), '#') || (punct(before(1), '!') && punct(before(2), '#')));
                let callee = if delim == '{' { None } else { callee_at(&toks, k) };
                let ui_arg = delim == '(' && ui_argument(&toks, k);
                let cmd_arg = delim == '(' && command_argument(&toks, k);
                let arm = delim == '(' && after_arrow(&toks, k);
                let method = delim == '(' && method_at(&toks, k);
                let ids = delim == '{' && id_fn_body(&toks, k);
                // `#[cfg(test)]`: skip the item that follows the attribute.
                if attr && ident(get(k + 1)) == Some("cfg") && punct(get(k + 2), '(') && ident(get(k + 3)) == Some("test") && punct(get(k + 4), ')') {
                    cfg_test_pending = true;
                }
                stack.push(Frame { delim, callee, attr, ui_arg, cmd_arg, arm, method, ids });
            }
            Tok::Punct(')' | ']' | '}') => {
                let was_attr = stack.pop().is_some_and(|f| f.attr);
                if was_attr && cfg_test_pending && *tok == Tok::Punct(']') {
                    cfg_test_pending = false;
                    skip = Some((stack.len(), false, false));
                }
            }
            Tok::Str(text) => {
                if looks_like_text(text) && shown(&toks, k, &stack) {
                    let (unwrapped, in_tc) = wrapping(&stack, block_value(&toks, k));
                    out.push(Candidate { file: file.to_string(), line: *line, text: text.clone(), unwrapped, in_tc });
                }
                // `format!("{text}  →")`: each captured name is used where the literal is.
                if stack.last().is_some_and(|f| f.delim == '(' && f.callee.as_deref() == Some("format!")) {
                    for x in format_captures(text) {
                        if let Some(text) = helper_use(&scopes, x, k, &stack).filter(|t| !helpers.iter().any(|c| c.text == *t)) {
                            helpers.push(Candidate { file: file.to_string(), line: *line, text, unwrapped: true, in_tc: false });
                        }
                    }
                }
            }
            Tok::Ident(x) => {
                let before = |n: usize| k.checked_sub(n).and_then(get);
                // `v.x`, `x: …`, `a == x` / `x != b` (a comparison draws nothing).
                let compared = (punct(before(1), '=') && matches!(before(2), Some((Tok::Punct('=' | '!'), _))))
                    || (matches!(get(k + 1), Some((Tok::Punct('=' | '!'), _))) && punct(get(k + 2), '='));
                if punct(before(1), '.') || punct(get(k + 1), ':') || compared {
                    continue;
                }
                // `x.into()` hands `x` on; `x.len()` doesn't.
                let passed =
                    !punct(get(k + 1), '.') || matches!(ident(get(k + 2)), Some("into" | "to_string" | "to_owned" | "as_str" | "as_ref" | "clone"));
                if let Some(text) = helper_use(&scopes, x, k, &stack).filter(|t| passed && !helpers.iter().any(|c| c.text == *t)) {
                    helpers.push(Candidate { file: file.to_string(), line: *line, text, unwrapped: true, in_tc: false });
                }
            }
            _ => {}
        }
    }
    (out, helpers)
}

/// `helper(x)` when `x` at `k`, inside `stack`, is an argument of the innermost helper around it
/// (not shadowed by a `let` before it) and reaches egui untranslated.
fn helper_use(scopes: &[Helper], x: &str, k: usize, stack: &[Frame]) -> Option<String> {
    let h = scopes
        .iter()
        .filter(|h| h.body.contains(&k) && h.args.iter().any(|a| a == x) && !h.shadows.iter().any(|(at, n)| n == x && *at < k && h.body.contains(at)))
        .min_by_key(|h| h.body.len())?;
    wrapping(stack, false).0.then(|| format!("{}({x})", h.name))
}

/// The names a `format!` string captures: `{name}`, `{name:>8}` (not `{}`, `{0}`, `{{`).
fn format_captures(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = s;
    while let Some(i) = rest.find('{') {
        let after = &rest[i + 1..];
        if let Some(escaped) = after.strip_prefix('{') {
            rest = escaped;
            continue;
        }
        let end = after.find('}').unwrap_or(after.len());
        let name = after[..end].split(':').next().unwrap_or("").trim();
        if name.chars().next().is_some_and(|c| c.is_alphabetic() || c == '_') && name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            out.push(name);
        }
        rest = after.get(end..).unwrap_or("");
    }
    out
}

/// Whether the string at `k` is a block's only expression (`{ "Show" }`, `{ "Show".into() }`,
/// `{ "Show".to_string() }`) or a `match` arm's value (`=> "Name"`): the block's value.
fn block_value(toks: &[(Tok, usize)], k: usize) -> bool {
    let prev = |n: usize| k.checked_sub(n).and_then(|j| toks.get(j));
    let next = |n: usize| toks.get(k + n);
    let only = punct(prev(1), '{')
        && (punct(next(1), '}') || (punct(next(1), '.') && matches!(ident(next(2)), Some("into" | "to_string" | "to_owned")) && punct(next(5), '}')));
    only || (punct(prev(1), '>') && punct(prev(2), '='))
}

/// Whether the string at `k`, inside `stack`, is in a place the UI shows (module notes).
fn shown(toks: &[(Tok, usize)], k: usize, stack: &[Frame]) -> bool {
    let get = |j: Option<usize>| j.and_then(|j| toks.get(j));
    let prev = |n: usize| get(k.checked_sub(n));
    let next = |n: usize| get(Some(k + n));
    if stack.iter().any(|f| f.attr) {
        return false;
    }
    // Comparisons and match patterns: `== "Home"`, `"Home" =>`, `"A" | "B" =>`.
    if (punct(prev(1), '=') && (punct(prev(2), '=') || punct(prev(2), '!'))) || (punct(next(1), '=') && punct(next(2), '=')) {
        return false;
    }
    if (punct(next(1), '=') && punct(next(2), '>')) || punct(next(1), '|') {
        return false;
    }
    if punct(prev(1), '>') && punct(prev(2), '=') {
        return !stack.iter().any(|f| f.ids);
    }
    if block_value(toks, k) {
        return true;
    }
    if ident(prev(1)) == Some("return") {
        return true;
    }
    if punct(prev(1), ':') && !punct(prev(2), ':') {
        let field = ident(prev(2)).unwrap_or("");
        return LABEL_FIELDS.iter().any(|f| field == *f || field.ends_with(&format!("_{f}")));
    }
    if punct(prev(1), '=') {
        // `const NAME: &str = "…";` (any visibility).
        let mut j = k.saturating_sub(1);
        while j > 0 {
            j -= 1;
            match toks.get(j) {
                Some((Tok::Punct(';' | '{' | '}'), _)) => break,
                Some((Tok::Ident(s), _)) if s == "const" || s == "static" => {
                    let name = ident(toks.get(j + 1)).unwrap_or("");
                    return !(name.ends_with("_ID") || name.ends_with("_KEY"));
                }
                _ => {}
            }
        }
        return false;
    }
    for f in stack.iter().rev() {
        if f.delim == '(' && (f.arm || f.cmd_arg) && !f.callee.as_deref().is_some_and(|c| is_name(c, NEVER)) {
            return true;
        }
        match (f.delim, f.callee.as_deref()) {
            ('[', _) => return true,
            ('{', _) => return false,
            (_, None) => continue,
            (_, Some(c)) if is_name(c, NEVER) => return false,
            (_, Some(c)) if is_name(c, DISPLAY) => return true,
            (_, Some(c)) if is_name(c, TRANSPARENT) => continue,
            _ => return f.ui_arg,
        }
    }
    false
}

/// For a candidate inside `stack`: whether its nearest drawing call is egui's own with no
/// `t`/`tc`/`tn` in between ([`Candidate::unwrapped`]), and whether that lookup is a `tc`.
/// Anything else it reaches first (the crate's widgets, an app helper with a `ui` argument, an
/// array, a block) is not judged: those translate inside or hand the string on. A `value` of
/// its block ([`block_value`]: `if a { "Show" } else { "Hide" }`, `match … { _ => "Name" }`)
/// looks past that one block.
fn wrapping(stack: &[Frame], value: bool) -> (bool, bool) {
    for (i, f) in stack.iter().rev().enumerate() {
        let Some(c) = f.callee.as_deref().filter(|_| f.delim == '(') else {
            if (f.delim == '(' && !f.arm && !f.cmd_arg) || (f.delim == '{' && value && i == 0) {
                continue;
            }
            return (false, false);
        };
        let last = c.rsplit("::").next().unwrap_or(c);
        if TRANSLATE.contains(&last) {
            return (false, last == "tc");
        }
        if (f.method && EGUI_METHODS.contains(&last)) || EGUI_PATHS.contains(&c) || last == "show_tooltip_text" {
            return (true, false);
        }
        if f.arm || f.cmd_arg || !(c == "fmt" || is_name(c, TRANSPARENT)) {
            return (false, false);
        }
    }
    (false, false)
}

#[cfg(test)]
mod scan_tests {
    use super::*;

    fn texts(src: &str) -> Vec<String> {
        scan("x.rs", src).into_iter().map(|c| c.text).collect()
    }

    #[test]
    fn display_calls_arrays_and_consts_are_candidates() {
        let src = r##"
            const TITLE: &str = "Slide Title";
            const TAB_ID: &str = "SmartArt Design";
            fn f(ui: &mut Ui) {
                ui.label("Dark appearance");
                big_button(ui, Icon::Paste, "Paste", true);
                ui.label(format!("{n} Slides"));
                let items = [("fill", "Solid Fill"), ("none", "No Fill")];
                let s = if on { "Hide Notes" } else { "Show Notes".to_string() };
                let k = match x { "Home" => "Start Here", _ => "Other Tab" };
                let d = match e { E::B => Some("From Bottom"), E::C => ("Close Path", 1), E::D => Err("Not Here") };
                let q = QatItem::new("file.new", "New Deck", Icon::File);
                set_status("Saved Copy");
                ui.label(r#"Raw "quoted" text"#);
                ui.label("Line\
                          continued");
            }
        "##;
        assert_eq!(
            texts(src),
            [
                "Slide Title",
                "Dark appearance",
                "Paste",
                "Solid Fill",
                "No Fill",
                "Hide Notes",
                "Show Notes",
                "Start Here",
                "Other Tab",
                "From Bottom",
                "Close Path",
                "New Deck",
                "Saved Copy",
                "Raw \"quoted\" text",
                "Linecontinued"
            ]
        );
    }

    #[test]
    fn ids_logs_patterns_attributes_and_tests_are_not() {
        let src = r#"
            #[serde(rename = "Some Name")]
            struct S;
            fn f(ui: &mut Ui, app: &mut App) {
                log::warn!("Could not open file");
                ui.push_id("Some Id", |ui| {});
                egui::Id::new("Panel State");
                if app.ui.tab == "Home Tab" {}
                let _ = app.run("view.dark", json!({"on": true}));
                let _ = 'a'; let _: &'static str = "x";
                // ui.label("Commented Out");
                /* ui.label("Block Comment"); */
                ui.label("Shift+F9");
                icons.text("B", pos);
                return Err(format!("Could Not {x}"));
            }
            #[cfg(test)]
            mod tests {
                fn g(ui: &mut Ui) { ui.label("Test Only"); }
            }
            #[cfg(test)]
            fn h(ui: &mut Ui) { ui.label("Test Fn"); }
            fn after(ui: &mut Ui) { ui.label("After Tests"); }
        "#;
        assert_eq!(texts(src), ["After Tests"]);
    }

    /// `(text, unwrapped, in_tc)` per candidate.
    fn wraps(src: &str) -> Vec<(String, bool, bool)> {
        scan("x.rs", src).into_iter().map(|c| (c.text, c.unwrapped, c.in_tc)).collect()
    }

    #[test]
    fn egui_calls_without_a_lookup_are_unwrapped() {
        let src = r#"
            fn f(ui: &mut Ui, app: &mut App) {
                ui.label("A b");
                ui.label(t("C d"));
                widgets::small_button(ui, Icon::Paste, "E f", true, false);
                small_button(ui, Icon::Paste, "G h", true, false);
                ui.small_button("I j");
                ui.label(egui::RichText::new(fmt(t("K {n}"), &[("n", &n)])));
                ui.label(fmt("L {n}", &[("n", &n)]));
                ui.button(tc("menu", "M n")).on_hover_text(format!("O p {x}"));
                big(app, ui, "Q r");
                let items = ["S t"];
                field(app.ui, "U v");
            }
        "#;
        assert_eq!(
            wraps(src),
            [
                ("A b".to_string(), true, false),
                ("C d".to_string(), false, false),
                ("E f".to_string(), false, false),
                ("G h".to_string(), false, false),
                ("I j".to_string(), true, false),
                ("K {n}".to_string(), false, false),
                ("L {n}".to_string(), true, false),
                ("M n".to_string(), false, true),
                ("O p {x}".to_string(), true, false),
                ("Q r".to_string(), false, false),
                ("S t".to_string(), false, false),
            ],
            "`app.ui` is a field, not a `ui` argument, so \"U v\" is not shown"
        );
    }

    #[test]
    fn block_values_and_painter_text_are_judged_by_the_call_outside() {
        let src = r#"
            fn f(ui: &mut Ui, p: &Painter) {
                ui.button(if a { "X y" } else { "Z w" });
                ui.button(t(if a { "Q r" } else { "S t" }));
                ui.label(match k { K::A => "M n", _ => "O p" });
                p.text(pos, Align2::LEFT_CENTER, "Painted Text", font, ink);
                let s = if a { "Let Value" } else { "Other Value" };
            }
        "#;
        let unwrapped: Vec<(String, bool)> = scan("x.rs", src).into_iter().map(|c| (c.text, c.unwrapped)).collect();
        let expected = [
            ("X y", true),
            ("Z w", true),
            ("Q r", false),
            ("S t", false),
            ("M n", true),
            ("O p", true),
            ("Painted Text", true),
            ("Let Value", false),
            ("Other Value", false),
        ];
        assert_eq!(unwrapped, expected.map(|(s, w)| (s.to_string(), w)));
    }

    #[test]
    fn helpers_that_hand_a_string_to_egui_untranslated_are_found() {
        let src = r#"
            fn field(ui: &mut Ui, d: &mut Dialog, label: &str, key: &str) {
                ui.horizontal(|ui| { ui.add(egui::Label::new(label)); });
                d.fields.insert(key.into(), v);
            }
            fn check(ui: &mut Ui, label: &str) -> bool { ui.checkbox(&mut v, t(label)).changed() }
            fn shadowed(ui: &mut Ui, label: &str) { let label = t(label); ui.label(label); }
            fn big(app: &mut App, ui: &mut Ui, label: &str) { widgets::big_button(ui, Icon::Paste, label, true); }
            fn tip(ui: &mut Ui, tip: impl Into<String>) { ui.label("x").on_hover_text(tip.into()); }
            fn no_ui(label: &str) -> String { label.to_string() }
            fn choice(ui: &mut Ui, current: &str, picked: &mut String, glyph: &str) {
                ui.selectable_label(v == current, l);
                ui.label(picked.as_str());
                ui.painter().text(c, a, glyph, f, ink);
            }
            fn f(ctx: &Context) {
                let button = |ui: &mut Ui, s: &str| ui.add(egui::Button::new(egui::RichText::new(s).size(15.0)));
                let row = |ui: &mut Ui, k: &str| -> bool { ui.label(t(k)).clicked() };
                let info = |ui: &mut Ui, val: String| { ui.label(val); };
            }
        "#;
        let found: Vec<String> = scan_all("x.rs", src).1.into_iter().map(|c| format!("{}:{}", c.line, c.text)).collect();
        assert_eq!(found, ["3:field(label)", "9:tip(tip)", "17:button(s)"]);
    }

    #[test]
    fn format_captures_and_tuple_lets_count_for_helpers() {
        let src = r#"
            fn arrow_link(ui: &mut Ui, text: &str) -> bool {
                ui.add(egui::Label::new(RichText::new(format!("{text}  →")).font(f))).clicked()
            }
            fn width(ui: &mut Ui, text: &str) { ui.label(format!("{n:>4} {{text}} {0}", text.len())); }
            fn named(ui: &mut Ui, label: &str, name: &str) {
                let (label, name) = (t(label), t(name));
                ui.label(format!("{label}: {name}"));
            }
        "#;
        let found: Vec<String> = scan_all("x.rs", src).1.into_iter().map(|c| format!("{}:{}", c.line, c.text)).collect();
        assert_eq!(found, ["3:arrow_link(text)"]);
        assert_eq!(format_captures("{a} {b:>8} {} {0} {{c}} {d.e}"), ["a", "b"]);
    }

    #[test]
    fn cfg_test_fields_and_key_names_are_skipped() {
        let src = r#"
            struct S {
                a: u8,
                #[cfg(test)]
                preview_spawns: usize,
            }
            fn after(ui: &mut Ui) { ui.label("After Struct"); }
            fn lit() -> S { S { a: 0, #[cfg(test)] preview_spawns: 0, } }
            fn last() -> S { S { a: 0, #[cfg(test)] preview_spawns: 0 } }
            fn tail(ui: &mut Ui) { ui.label("Tail Text"); }
            pub fn key_name(key: egui::Key) -> &'static str {
                match key { Enter => "Enter", PageUp => "PageUp", other => other.name() }
            }
            fn tool_label(t: Tool) -> &'static str { match t { Tool::Pen => "Pen Tool" } }
        "#;
        assert_eq!(texts(src), ["After Struct", "Tail Text", "Pen Tool"]);
    }
}

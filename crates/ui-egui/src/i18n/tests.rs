#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use super::*;

const COMMON: &str = include_str!("../../i18n/ko/common.tsv");
const DECKCRAFT: &str = include_str!("../../i18n/ko/deckcraft.tsv");

/// One catalog row: (line, context, English, translation), escapes undone.
fn rows(text: &str) -> Vec<(usize, String, String, String)> {
    text.lines()
        .enumerate()
        .filter(|(_, l)| !l.is_empty() && !l.starts_with('#'))
        .map(|(i, l)| {
            let cols: Vec<&str> = l.split('\t').collect();
            assert_eq!(cols.len(), 3, "line {}: {l:?}", i + 1);
            (i + 1, unescape(cols[0]), unescape(cols[1]), unescape(cols[2]))
        })
        .collect()
}

fn unescape(s: &str) -> String {
    s.replace("\\n", "\n").replace("\\t", "\t").replace("\\\\", "\\")
}

fn placeholders(s: &str) -> BTreeSet<&str> {
    let mut out = BTreeSet::new();
    let mut rest = s;
    while let Some((_, after)) = rest.split_once('{') {
        let Some((name, after)) = after.split_once('}') else { break };
        out.insert(name);
        rest = after;
    }
    out
}

fn src() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

#[test]
fn auto_is_english_without_system_language() {
    assert_eq!(resolve(AUTO), craft_i18n::Lang::EN);
    assert_eq!(resolve("xx"), craft_i18n::Lang::EN, "an unknown setting follows the system, English here");
}

#[test]
fn korean_pref_translates_common_rows() {
    set_current(resolve("ko"));
    assert!(is_korean());
    assert_eq!(t("Cancel"), "취소");
    assert_eq!(t("Shape"), "도형", "deckcraft.tsv's term wins over common.tsv's 모양");
    for (en, ko) in [("Comment", "댓글"), ("Align", "정렬"), ("Center", "가운데"), ("3-D", "3차원")] {
        assert_eq!(t(en), ko, "Slide's term for {en:?} wins over common.tsv's (Ruling 15)");
    }
    set_current(resolve("en"));
    assert!(!is_korean());
    assert_eq!(t("Cancel"), "Cancel");
}

#[test]
fn unknown_string_falls_back_to_english() {
    set_current(resolve("ko"));
    assert_eq!(t("zz-no-such-key"), "zz-no-such-key");
    assert_eq!(tc("menu", "zz-no-such-key"), "zz-no-such-key");
    set_current(craft_i18n::Lang::EN);
}

#[test]
fn counts_keep_their_english_digits() {
    // The English wording stays what `format!("{n} …")` wrote before: no digit grouping.
    set_current(craft_i18n::Lang::EN);
    assert_eq!(tn(1, "{n} slide", "{n} slides"), "1 slide");
    assert_eq!(tn(1234, "{n} slide", "{n} slides"), "1234 slides");
    assert_eq!(fmt("{name} options", &[("name", "Paste")]), "Paste options");
}

#[test]
fn fmt_writes_what_format_wrote() {
    // No direction isolates around right-to-left values: egui doesn't do bidi (Ruling 13).
    for value in ["Paste", "שלום", "مرحبا.pptx"] {
        assert_eq!(fmt("{name} options", &[("name", value)]), format!("{value} options"));
        assert_eq!(fmt("{title} — Edited", &[("title", value)]), format!("{value} — Edited"));
    }
    assert!(!fmt("{a} {b}", &[("a", "עברית"), ("b", "1")]).chars().any(|c| ('\u{2066}'..='\u{2069}').contains(&c)));
}

#[test]
fn the_language_is_per_thread() {
    set_current(resolve("ko"));
    let other = std::thread::spawn(current).join().unwrap();
    assert_eq!(other, craft_i18n::Lang::EN, "a new thread (a parallel test) starts in English");
    set_current(craft_i18n::Lang::EN);
}

#[test]
fn catalog_rows_keep_placeholders_ellipsis_and_newlines() {
    for (name, text) in [("common", COMMON), ("deckcraft", DECKCRAFT)] {
        for (line, context, en, ko) in rows(text) {
            if context == "@plural" || context == "@id" {
                continue;
            }
            let at = format!("{name}.tsv line {line}: {en:?} → {ko:?}");
            assert_eq!(placeholders(&en), placeholders(&ko), "{at}: placeholders");
            assert_eq!(en.ends_with('…'), ko.ends_with('…'), "{at}: trailing …");
            assert_eq!(en.matches('\n').count(), ko.matches('\n').count(), "{at}: line breaks");
        }
    }
    assert!(I18N.check_all().is_empty(), "{:?}", I18N.check_all());
}

#[test]
fn no_duplicate_keys_in_fork_catalog() {
    let mut seen = BTreeMap::new();
    for (line, context, en, _) in rows(DECKCRAFT) {
        if let Some(first) = seen.insert((context.clone(), en.clone()), line) {
            panic!("deckcraft.tsv line {line} repeats line {first}: ({context:?}, {en:?})");
        }
    }
}

#[test]
fn fork_rows_do_not_repeat_common_rows() {
    // A fork row with the same key hides the shared one; it must say why by differing.
    let common: BTreeMap<(String, String), String> = rows(COMMON).into_iter().map(|(_, c, en, ko)| ((c, en), ko)).collect();
    for (line, context, en, ko) in rows(DECKCRAFT) {
        assert_ne!(common.get(&(context, en.clone())), Some(&ko), "deckcraft.tsv line {line} repeats the common row for {en:?}");
    }
}

#[test]
fn keep_english_entries_are_still_in_source() {
    let found: BTreeSet<String> = audit::candidates(&src()).into_iter().map(|c| c.text).collect();
    for entry in audit::keep_english() {
        assert!(found.contains(&entry), "keep_english.txt lists {entry:?}, which is no longer interface text in src/");
        assert!(!has_ko(&entry), "keep_english.txt lists {entry:?}, which has a Korean row");
    }
}

#[test]
fn ui_state_language_defaults_to_auto_and_sanitizes() {
    let d = crate::UiState::default();
    assert_eq!(d.language, AUTO);
    let s: crate::UiState = serde_json::from_str(r#"{"language":"xx"}"#).unwrap();
    assert_eq!(s.sanitized().language, AUTO);
    let s: crate::UiState = serde_json::from_str(r#"{"language":"ko"}"#).unwrap();
    assert_eq!(s.sanitized().language, "ko");
    let s: crate::UiState = serde_json::from_str("{}").unwrap();
    assert_eq!(s.language, AUTO, "an older ui.json without the field follows the system");
}

/// Every interface string of the shared ribbon widgets has a Korean row or a reason to stay
/// English.
#[test]
fn korean_coverage_widgets() {
    let found = audit::candidates(&src().join("widgets.rs"));
    assert!(found.iter().any(|c| c.text == "Theme Colors"), "the scan sees the colour menu's headings");
    let problems = audit::problems(&src().join("widgets.rs"));
    assert!(problems.is_empty(), "no Korean row (deckcraft.tsv) or keep_english.txt entry, or not wrapped in t():\n{}", problems.join("\n"));
}

/// The ribbon's tab names are shown through `t()`; `app.ui.tab` keeps the English name.
#[test]
fn korean_ribbon_tab_names() {
    let ribbon = std::fs::read_to_string(src().join("ribbon.rs")).unwrap();
    let body = ribbon.split("fn contextual(").nth(1).and_then(|b| b.split("\n}\n").next()).unwrap();
    let contextual: Vec<&str> = body.split("v.push(\"").skip(1).filter_map(|s| s.split('"').next()).collect();
    assert!(contextual.len() >= 8, "{contextual:?}");
    let mut names: Vec<&str> = crate::ribbon::TABS.to_vec();
    names.push("File");
    names.extend(contextual.iter().map(|n| crate::ribbon::tab_label(n)));
    names.push(crate::ribbon::tab_label("SmartArt Design"));
    let missing: Vec<&&str> = names.iter().filter(|n| !has_ko(n)).collect();
    assert!(missing.is_empty(), "ribbon tabs without a Korean row: {missing:?}");
}

/// The scan finds interface text and skips ids, logs and tests; it also reports how many
/// candidates each area of `src/` has (run with `--nocapture`; `I18N_AUDIT_LIST=1` also lists
/// every problem), which the later translation tasks size and find their work by.
#[test]
fn candidate_scan_finds_interface_text_by_area() {
    let all = audit::candidates(&src());
    let texts: BTreeSet<&str> = all.iter().map(|c| c.text.as_str()).collect();
    for expected in ["Dark appearance", "Interface size:", "Theme Colors", "Grid Options…"] {
        assert!(texts.contains(expected), "{expected:?} is interface text");
    }
    for skipped in ["view.dark", "ribbon_scroll", "Shift+F9"] {
        assert!(!texts.contains(skipped), "{skipped:?} is not interface text");
    }
    let keep = audit::keep_english();
    let no_row = audit::no_row(&all);
    let unwrapped: Vec<audit::Candidate> = all.iter().filter(|c| c.unwrapped && !keep.contains(&c.text)).cloned().collect();
    let helpers = audit::untranslated_helpers(&src());
    let mut by_area: BTreeMap<String, [usize; 4]> = BTreeMap::new();
    for (column, list) in [&all, &no_row, &unwrapped, &helpers].into_iter().enumerate() {
        for c in list {
            by_area.entry(audit::area(&c.file)).or_default()[column] += 1;
        }
    }
    eprintln!("candidates (all / no row / not wrapped in t() / untranslating helpers) by area:");
    for (area, [n, r, w, h]) in &by_area {
        eprintln!("  {area:<14} {n:>5} / {r:>5} / {w:>5} / {h:>3}");
    }
    eprintln!("  {:<14} {:>5} / {:>5} / {:>5} / {:>3}", "total", all.len(), no_row.len(), unwrapped.len(), helpers.len());
    eprintln!("  missing (no row or not wrapped): {}", audit::missing(&all).len());
    if std::env::var_os("I18N_AUDIT_LIST").is_some() {
        eprintln!("{}", audit::problems(&src()).join("\n"));
    }
}

/// Every interface string of the first ribbon tabs (Home … Animations, the galleries, the Quick
/// Access Toolbar and the title bar) has a Korean row or a reason to stay English, and egui's
/// own calls look it up.
#[test]
fn korean_coverage_ribbon_1() {
    let mut problems = Vec::new();
    for file in ["home", "insert", "draw", "design", "transitions", "animations", "galleries", "qat", "title_bar"] {
        problems.extend(audit::problems(&src().join("ribbon").join(format!("{file}.rs"))));
    }
    assert!(problems.is_empty(), "no Korean row (deckcraft.tsv) or keep_english.txt entry, or not wrapped in t():\n{}", problems.join("\n"));
}

/// The names the first ribbon tabs show from built-in data (themes and their colour, font and
/// effect schemes, transitions, animations and their options, sounds, shapes, diagram layouts,
/// chart types, Insert › Icons' groups and names) have Korean rows. The data and what the document stores stay English; only the
/// display is looked up.
#[test]
fn korean_gallery_names() {
    use deckcraft_model::anim::{ANIMATIONS, TRANSITIONS, transition_option_label};
    use deckcraft_model::theme;
    let mut names: BTreeSet<String> = BTreeSet::new();
    let builtin = theme::builtin_themes();
    for th in &builtin {
        names.insert(th.name.clone());
    }
    names.extend(theme::builtin_color_schemes().into_iter().map(|c| c.name));
    names.extend(theme::builtin_font_schemes().into_iter().map(|f| f.name));
    names.extend(theme::builtin_format_schemes().into_iter().map(|f| f.name));
    for &(kind, name, category, _, options) in TRANSITIONS {
        names.extend([name.to_string(), category.to_string()]);
        names.extend(options.iter().map(|o| transition_option_label(kind, o)));
    }
    for &(effect, name, _, _, _, options) in ANIMATIONS {
        names.insert(name.to_string());
        names.extend(options.iter().map(|o| crate::ribbon::animation_option_label(effect, o)));
    }
    for icon in deckcraft_engine::icons::icons() {
        names.extend([icon.category.to_string(), icon.label.to_string()]);
    }
    names.extend(deckcraft_engine::sounds::BUILTIN.iter().map(|(_, l)| l.to_string()));
    names.extend(deckcraft_engine::tools::FREEFORM_TOOLS.iter().map(|(_, l)| l.to_string()));
    names.extend(deckcraft_geom::preset::Category::ALL.iter().map(|c| c.label().to_string()));
    names.extend(deckcraft_geom::preset::CATALOG.iter().map(|p| p.label.to_string()));
    names.extend(deckcraft_engine::cmd::smartart::LAYOUTS.iter().map(|(_, l)| l.to_string()));
    let ko = I18N.from_code("ko").unwrap();
    let charts: Vec<&str> = crate::ribbon::chart_menu_types().into_iter().flatten().map(|k| k.label()).collect();
    names.extend(charts.iter().filter(|l| I18N.get(ko, "chart", l).is_none()).map(|l| l.to_string()));
    let keep = audit::keep_english();
    // Kept in every language: the built-in theme's name and the font schemes named after fonts.
    let named = ["DeckCraft", "Inter", "Times / Arial"];
    let missing: Vec<&String> = names.iter().filter(|n| !has_ko(n) && !keep.contains(*n) && !named.contains(&n.as_str())).collect();
    assert!(
        missing.is_empty(),
        "{} gallery names without a Korean row:\n{}",
        missing.len(),
        missing.iter().map(|n| format!("\t{n}\t")).collect::<Vec<_>>().join("\n")
    );
}

/// The whole app with a new presentation, drawn in Korean. A build without craft-fonts has no
/// Hangul face, so missing glyphs draw as boxes; the tests check names, not glyphs.
fn korean_harness() -> egui_kittest::Harness<'static, crate::SlideApp> {
    let app = crate::SlideApp::new(deckcraft_engine::Session::with_new(), crate::Services::default());
    let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(1400.0, 900.0)).build_ui_state(
        |ui, app: &mut crate::SlideApp| {
            app.logic(ui.ctx());
            app.ui(ui);
        },
        app,
    );
    h.ctx.set_missing_glyph_policy(egui::MissingGlyphPolicy::Tofu);
    h.state_mut().ui.language = "ko".into();
    // The app keeps asking for frames, so step instead of run().
    h.run_steps(3);
    h
}

/// Clicking a ribbon tab by its Korean name saves its English id, and a ribbon button added to
/// the Quick Access Toolbar from Korean saves its command and English label.
#[test]
fn tab_id_stays_english_in_korean() {
    use egui_kittest::kittest::Queryable;
    let mut h = korean_harness();
    h.get_by_role_and_label(egui::accesskit::Role::Button, "삽입").click();
    h.run_steps(3);
    assert_eq!(h.state().ui.tab, "Insert");
    h.get_by_role_and_label(egui::accesskit::Role::Button, "수식").click_secondary();
    h.run_steps(3);
    h.get_by_label("빠른 실행 도구 모음에 추가").click();
    h.run_steps(3);
    let added = h.state().ui.qat.last().cloned().unwrap();
    assert_eq!((added.id.as_str(), added.label.as_str()), ("app.dialog", "Equation"), "ui.json keeps the command and the English label");
    assert_eq!(added.params, serde_json::json!({"id": "equation"}));
    // The toolbar's tooltip adds the shortcut (and Alt+N off the Mac), so match the start.
    assert!(h.query_all_by_label_contains("수식").count() >= 2, "the toolbar shows the added button in Korean");
}

/// A ribbon button's tooltip (its label and shortcut, made by `ribbon::tip`) is in Korean.
#[test]
fn ribbon_tooltips_are_korean() {
    use egui_kittest::kittest::Queryable;
    let mut h = korean_harness();
    h.get_by_role_and_label(egui::accesskit::Role::Button, "잘라내기").hover();
    h.run_steps(5);
    let shortcut = crate::ribbon::pretty_shortcut(deckcraft_engine::find_command("edit.cut").and_then(|c| c.shortcut).unwrap());
    assert!(h.query_by_label(&format!("잘라내기 ({shortcut})")).is_some(), "the Cut button's tooltip is translated");
    assert!(h.query_by_label_contains("Cut (").is_none());
}

/// Ribbon buttons save their label on one line in the Quick Access Toolbar ("Add Animation");
/// the toolbar still shows it translated. Every two-line ribbon label with a Korean row, in every
/// ribbon file (later tabs included), goes through the toolbar's display.
#[test]
fn qat_shows_one_line_ribbon_labels_in_korean() {
    let mut found = audit::candidates(&src().join("ribbon"));
    found.extend(audit::candidates(&src().join("ribbon.rs")));
    let two_line: BTreeSet<String> = found.into_iter().map(|c| c.text.replace("\\n", "\n")).filter(|t| t.contains('\n')).collect();
    assert!(two_line.contains("Add\nAnimation"), "{two_line:?}");
    set_current(resolve("ko"));
    let wrong: Vec<String> = two_line
        .iter()
        .filter(|l| has_ko(l))
        .filter_map(|l| {
            let saved = l.replace('\n', " ");
            let want = t(l).replace('\n', " ");
            let got = crate::qat::shown_label(&saved);
            (got != want).then(|| format!("{saved:?}: {got:?}, want {want:?}"))
        })
        .collect();
    set_current(craft_i18n::Lang::EN);
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    assert_eq!(crate::qat::shown_label("Add Animation"), "Add Animation", "English stays as saved");
}

/// The Sound box shows a built-in sound's name translated; a file the user chose keeps its name.
#[test]
fn transition_sound_names_show_translated() {
    set_current(resolve("ko"));
    assert_eq!(crate::ribbon::sound_name("Chime.wav"), t("Chime"));
    assert_ne!(crate::ribbon::sound_name("Chime.wav"), "Chime");
    assert_eq!(crate::ribbon::sound_name("my applause.wav"), "my applause");
    assert_eq!(crate::ribbon::sound_name("Chime.mp3"), "Chime.mp3");
    set_current(craft_i18n::Lang::EN);
    assert_eq!(crate::ribbon::sound_name("Chime.wav"), "Chime");
}

/// Insert › Icons' search finds an icon by its id, its English name and its Korean name.
#[test]
fn icon_search_matches_english_and_korean_names() {
    let icons = deckcraft_engine::icons::icons();
    let found = |q: &str| icons.iter().filter(|i| crate::ribbon::icon_matches(i, q)).map(|i| i.name).collect::<Vec<_>>();
    set_current(resolve("ko"));
    let korean = t("Calendar").to_lowercase();
    assert_ne!(korean, "calendar");
    for q in ["calendar", korean.as_str()] {
        assert!(found(q).contains(&"calendar"), "{q:?} finds the calendar icon: {:?}", found(q));
    }
    assert!(found("arrow-up").contains(&"arrow-up"), "and by id");
    set_current(craft_i18n::Lang::EN);
    assert!(found(&korean).is_empty(), "English doesn't match Korean names");
}

/// Every interface string of the remaining ribbon tabs (Slide Show, Review, View) and the
/// contextual tabs (Shape/Picture Format, Table, Chart, SmartArt, Masters, Playback, Arrange),
/// plus `ribbon.rs`, has a Korean row or a reason to stay English, and egui's own calls look it up.
#[test]
fn korean_coverage_ribbon_2() {
    let mut problems = audit::problems(&src().join("ribbon.rs"));
    for file in ["slide_show", "review", "view", "shape_format", "picture_format", "table", "chart", "smartart", "masters", "playback", "arrange"] {
        problems.extend(audit::problems(&src().join("ribbon").join(format!("{file}.rs"))));
    }
    assert!(problems.is_empty(), "no Korean row (deckcraft.tsv) or keep_english.txt entry, or not wrapped in t():\n{}", problems.join("\n"));
}

/// The whole ribbon: every non-test file under `src/ribbon/` and `ribbon.rs`.
#[test]
fn korean_coverage_ribbon_all() {
    let mut problems = audit::problems(&src().join("ribbon.rs"));
    let mut files: Vec<PathBuf> = std::fs::read_dir(src().join("ribbon")).unwrap().map(|e| e.unwrap().path()).collect();
    files.sort();
    for file in files.iter().filter(|f| f.extension().is_some_and(|e| e == "rs") && !f.to_string_lossy().ends_with("_tests.rs")) {
        problems.extend(audit::problems(file));
    }
    assert!(problems.is_empty(), "no Korean row (deckcraft.tsv) or keep_english.txt entry, or not wrapped in t():\n{}", problems.join("\n"));
}

/// The names the contextual tabs show from built-in data (text warps, merge operations, artistic
/// effects, recolour presets, picture styles, table styles, SmartArt colours and styles) have
/// Korean rows. The commands and the document keep the English ids.
#[test]
fn korean_contextual_gallery_names() {
    use deckcraft_engine::cmd::{artistic, merge, picture_style, recolor, smartart};
    let mut names: BTreeSet<String> = BTreeSet::new();
    names.extend(deckcraft_geom::warp::CATALOG.iter().map(|p| p.label.to_string()));
    names.extend(merge::OPS.iter().map(|(_, l)| l.to_string()));
    names.extend(artistic::EFFECTS.iter().map(|(_, l)| l.to_string()));
    names.extend(recolor::presets().into_iter().map(|(_, l)| l));
    names.extend(picture_style::STYLES.iter().map(|(_, l)| l.to_string()));
    names.extend(deckcraft_model::table::table_styles().into_iter().map(|(_, l)| l));
    names.extend(smartart::COLORS.iter().chain(smartart::STYLES.iter()).map(|(_, l)| l.to_string()));
    let missing: Vec<&String> = names.iter().filter(|n| !has_ko(n)).collect();
    assert!(
        missing.is_empty(),
        "{} gallery names without a Korean row:\n{}",
        missing.len(),
        missing.iter().map(|n| format!("\t{n}\t")).collect::<Vec<_>>().join("\n")
    );
}

/// Chart Design › Change Chart Type names the chart types in their chart sense: a pie chart is
/// 원형, not the pie shape's 부채꼴.
#[test]
fn change_chart_type_menu_uses_chart_names() {
    use egui_kittest::kittest::Queryable;
    let mut h = korean_harness();
    h.state_mut().run("insert.chart", serde_json::json!({"type": "Pie"})).unwrap();
    h.state_mut().ui.tab = "Chart Design".into();
    h.run_steps(3);
    h.get_by_label_contains("차트 종류").click();
    h.run_steps(3);
    assert!(h.query_by_label(tc("chart", "Pie")).is_some(), "the menu lists the pie chart");
    assert_eq!(tc("chart", "Pie"), "원형");
    assert!(h.query_by_label(t("Pie")).is_none(), "not the shape's name");
}

/// Every engine command's name and the ribbon tab and group (or menu) it lives in, and every UI
/// command's name (the command palette lists both), have Korean rows. The ids and params stay
/// English; only the display is looked up.
#[test]
fn korean_coverage_command_labels() {
    let mut names: BTreeSet<&str> = BTreeSet::new();
    for c in deckcraft_engine::cmd::command_specs() {
        names.insert(c.label);
        names.extend(c.menu.iter().copied());
    }
    names.extend(crate::UI_COMMANDS.iter().map(|c| c.1));
    assert!(names.len() > 300, "{} names", names.len());
    let keep = audit::keep_english();
    let missing: Vec<&&str> = names.iter().filter(|n| !has_ko(n) && !keep.iter().any(|k| k == **n)).collect();
    assert!(
        missing.is_empty(),
        "{} command names without a Korean row:\n{}",
        missing.len(),
        missing.iter().map(|n| format!("\t{}\t", n.replace('\n', "\\n"))).collect::<Vec<_>>().join("\n")
    );
}

/// The reasons the engine's `enabled` checks give ("nothing selected"), read from its source, have
/// Korean rows; a `format!` reason by its template with `{0}`, `{1}` for the values. They reach
/// the screen inside "command `…` is not available right now: …" in the status bar, which has a
/// template row too.
#[test]
fn korean_coverage_disabled_reasons() {
    let reasons = engine_disabled_reasons();
    for expected in ["nothing selected", "no presentation open", "open View ▸ Notes Master first"] {
        assert!(reasons.contains(expected), "the scan finds {expected:?}: {reasons:?}");
    }
    // What the checks say with no presentation open and with a new one is among what the scan found.
    for session in [deckcraft_engine::Session::new(), deckcraft_engine::Session::with_new()] {
        for c in deckcraft_engine::cmd::command_specs() {
            if let Err(reason) = (c.enabled)(&session) {
                assert!(reasons.contains(&reason), "{}: the scan misses {reason:?}", c.id);
            }
        }
    }
    let mut missing: Vec<&String> = reasons.iter().filter(|r| !has_ko(r)).collect();
    let wrapper = "command `{0}` is not available right now: {1}".to_string();
    if !has_ko(&wrapper) {
        missing.push(&wrapper);
    }
    assert!(
        missing.is_empty(),
        "{} disabled reasons without a Korean row:\n{}",
        missing.len(),
        missing.iter().map(|n| format!("\t{n}\t")).collect::<Vec<_>>().join("\n")
    );
}

/// The engine's `enabled` functions (`fn …(s: &Session) -> std::result::Result<(), String>`) in
/// `cmd.rs` and `cmd/**.rs`, and the strings they return: literals and the `&str` consts they
/// name (`Err(NOTHING_SELECTED.into())`), `format!`'s `{}` numbered `{0}`, `{1}`.
fn engine_disabled_reasons() -> BTreeSet<String> {
    let engine = Path::new(env!("CARGO_MANIFEST_DIR")).join("../engine/src");
    let mut files = vec![engine.join("cmd.rs")];
    let mut dirs = vec![engine.join("cmd")];
    while let Some(dir) = dirs.pop() {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                dirs.push(p);
            } else if p.extension().is_some_and(|x| x == "rs") {
                files.push(p);
            }
        }
    }
    let sources: Vec<String> = files.iter().map(|f| std::fs::read_to_string(f).unwrap()).collect();
    let mut consts: BTreeMap<String, String> = BTreeMap::new();
    for src in &sources {
        for line in src.lines() {
            let Some((_, rest)) = line.split_once("const ") else { continue };
            let Some((name, value)) = rest.split_once(": &str = \"") else { continue };
            if let Some(value) = value.strip_suffix("\";") {
                consts.insert(name.trim().to_string(), value.to_string());
            }
        }
    }
    let mut out = BTreeSet::new();
    for src in &sources {
        let mut lines = src.lines();
        while let Some(line) = lines.next() {
            if !(line.contains("fn ") && line.contains(": &Session) -> std::result::Result<(), String>")) {
                continue;
            }
            let mut body = line.to_string();
            if !line.trim_end().ends_with('}') {
                for l in lines.by_ref() {
                    body.push('\n');
                    body.push_str(l);
                    if l == "}" {
                        break;
                    }
                }
            }
            let body = body.split_once('{').map(|(_, b)| b).unwrap_or_default();
            for (i, piece) in body.split('"').enumerate() {
                if i % 2 == 1 {
                    let mut n = 0;
                    let mut template = String::new();
                    let mut rest = piece;
                    while let Some((before, after)) = rest.split_once("{}") {
                        template.push_str(&format!("{before}{{{n}}}"));
                        n += 1;
                        rest = after;
                    }
                    template.push_str(rest);
                    out.insert(template);
                }
            }
            for (name, value) in &consts {
                if body.contains(&format!("{name}.into()")) || body.contains(&format!("{name}.to_string()")) {
                    out.insert(value.clone());
                }
            }
        }
    }
    out
}

/// The command palette's and the menu tree's own wording has Korean rows and is looked up.
#[test]
fn korean_coverage_menus() {
    let problems = audit::problems(&src().join("menus.rs"));
    assert!(problems.is_empty(), "no Korean row (deckcraft.tsv) or keep_english.txt entry, or not wrapped in t():\n{}", problems.join("\n"));
}

/// In Korean the command palette finds a command by its Korean name and by its English name, and
/// lists it under the Korean one; it still runs the command's id.
#[test]
fn command_search_matches_korean_and_english() {
    use egui_kittest::kittest::Queryable;
    let session = deckcraft_engine::Session::with_new();
    let ids = |q: &str| crate::menus::palette_items(&session, &q.to_lowercase()).into_iter().map(|(id, ..)| id).collect::<Vec<_>>();
    set_current(resolve("ko"));
    let korean = t("New Slide").to_string();
    assert_ne!(korean, "New Slide");
    for q in ["New Slide", korean.as_str()] {
        assert!(ids(q).contains(&"slide.new".to_string()), "{q:?} finds slide.new: {:?}", ids(q));
    }
    set_current(craft_i18n::Lang::EN);
    assert!(!ids(&korean).contains(&"slide.new".to_string()), "English doesn't match Korean names");

    let mut h = korean_harness();
    h.state_mut().palette = Some(("command palette".into(), 0));
    h.run_steps(3);
    assert!(h.query_by_label(t("Command Palette")).is_some(), "the palette lists its entries in Korean");
    assert!(h.query_by_label("Command Palette").is_none());
}

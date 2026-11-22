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
    assert_eq!((t("Video"), t("Video Format")), ("동영상", "동영상 서식"), "Video is 동영상 everywhere, over common.tsv's 비디오");
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
    // Kept in every language: the font schemes named after fonts (the built-in "DeckCraft" theme
    // is in keep_english.txt).
    let named = ["Inter", "Times / Arial"];
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

/// Dialogs 1: the shared dialog helpers and title map (`dialogs.rs`) and the dialogs from About
/// to Language.
#[test]
fn korean_coverage_dialogs_1() {
    let mut problems = audit::problems(&src().join("dialogs.rs"));
    for file in [
        "about",
        "alt_text",
        "body",
        "columns",
        "credits",
        "date_time",
        "equation",
        "export",
        "find",
        "font",
        "grid_options",
        "hyperlink",
        "language",
    ] {
        problems.extend(audit::problems(&src().join("dialogs").join(format!("{file}.rs"))));
    }
    assert!(problems.is_empty(), "no Korean row (deckcraft.tsv) or keep_english.txt entry, or not wrapped in t():\n{}", problems.join("\n"));
}

/// The Font dialog opened in Korean has a Korean title and OK / Cancel buttons.
#[test]
fn font_dialog_is_korean() {
    use egui_kittest::kittest::Queryable;
    let mut h = korean_harness();
    // The ribbon's Font group says 글꼴 too: the dialog adds its title and its Font field.
    let before = h.query_all_by_label(t("Font")).count();
    h.state_mut().dialog = Some(crate::dialogs::Dialog::new("font"));
    h.run_steps(3);
    assert_eq!(t("Font"), "글꼴");
    assert!(h.query_all_by_label(t("Font")).count() >= before + 2, "the title and the Font field are Korean");
    for en in ["OK", "Cancel"] {
        assert_ne!(t(en), en, "{en:?} has a Korean row");
        assert!(h.query_by_role_and_label(egui::accesskit::Role::Button, t(en)).is_some(), "the dialog shows {:?} for {en:?}", t(en));
    }
    assert!(h.query_by_label("Font").is_none() && h.query_by_label("Cancel").is_none(), "no English title or button");
    assert_eq!(h.state().dialog.as_ref().map(|d| d.id.as_str()), Some("font"));
}

/// Names the dialogs show from outside `src/` and translate where they draw them: the Slide Size
/// presets (deckcraft-model; the dialog keeps the English name as its field value) and the
/// Accessibility Checker's fixed issues (deckcraft-engine `review.accessibility`).
#[test]
fn korean_dialog_data_names() {
    let mut names: Vec<&str> = deckcraft_model::defaults::SLIDE_SIZES.iter().map(|(n, _, _)| *n).collect();
    names.extend(["Missing slide title", "Missing alternative text"]);
    let missing: Vec<&&str> = names.iter().filter(|n| !has_ko(n)).collect();
    assert!(missing.is_empty(), "no Korean row: {missing:?}");
}

/// Dialog titles are translated, except where the title is the app's name (Quit, unknown dialogs).
#[test]
fn dialog_titles_keep_the_app_name() {
    set_current(resolve("ko"));
    assert_eq!(crate::dialogs::shown_title("font"), "글꼴");
    assert_eq!(crate::dialogs::shown_title("quit"), crate::APP_NAME);
    assert_eq!(crate::dialogs::shown_title("zz-no-such-dialog"), crate::APP_NAME);
    set_current(craft_i18n::Lang::EN);
    assert_eq!(crate::dialogs::shown_title("font"), "Font");
}

/// Dialogs 2: the dialogs from Paragraph to Zoom and the fork's update card / modal.
#[test]
fn korean_coverage_dialogs_2() {
    let dialogs = src().join("dialogs");
    let mut problems = Vec::new();
    for file in [
        "paragraph",
        "photo_album",
        "preferences",
        "size_position",
        "slide_size",
        "tabs",
        "theme_colors",
        "theme_fonts",
        "trim",
        "zoom",
        "updates_ui",
        "updates_ui/view",
    ] {
        problems.extend(audit::problems(&dialogs.join(format!("{file}.rs"))));
    }
    assert!(problems.is_empty(), "no Korean row (deckcraft.tsv) or keep_english.txt entry, or not wrapped in t():\n{}", problems.join("\n"));
}

/// Every dialog source file (tests aside) and `dialogs.rs` is covered.
#[test]
fn korean_coverage_dialogs_all() {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            if path.is_dir() {
                walk(&path, out);
            } else if name.ends_with(".rs") && name != "tests.rs" && !name.ends_with("_tests.rs") {
                out.push(path);
            }
        }
    }
    let mut files = vec![src().join("dialogs.rs")];
    walk(&src().join("dialogs"), &mut files);
    files.sort();
    assert!(files.len() > 25, "found the dialog files: {files:?}");
    let problems: Vec<String> = files.iter().flat_map(|f| audit::problems(f)).collect();
    assert!(problems.is_empty(), "no Korean row (deckcraft.tsv) or keep_english.txt entry, or not wrapped in t():\n{}", problems.join("\n"));
}

/// Choice lists the audit only checks for a row (arrays, a match, a helper that takes them):
/// Paragraph's line spacing, the tab kinds and the Photo Album lists show translated.
#[test]
fn dialogs_2_choice_lists_are_korean() {
    use egui_kittest::kittest::Queryable;
    for en in ["Single", "1.5 lines", "Double", "Multiple", "Exactly", "Left", "Center", "Right", "Decimal"] {
        assert!(has_ko(en), "{en:?} has a Korean row");
    }
    let mut h = korean_harness();
    h.state_mut().dialog = Some(crate::dialogs::Dialog::new("tabs"));
    h.run_steps(3);
    assert!(h.query_by_label(t("Decimal")).is_some() && h.query_by_label("Decimal").is_none(), "the tab kinds are Korean");
    h.state_mut().dialog = Some(crate::dialogs::Dialog::new("paragraph"));
    h.run_steps(3);
    let combos: Vec<Option<String>> = h.query_all_by_role(egui::accesskit::Role::ComboBox).map(|n| n.value()).collect();
    assert!(
        h.query_by_label(t("Single")).is_some() || combos.iter().any(|v| v.as_deref() == Some(t("Single"))),
        "line spacing shows {:?}: {combos:?}",
        t("Single")
    );
    h.state_mut().dialog = Some(crate::dialogs::Dialog::new("photoAlbum"));
    h.run_steps(3);
    assert!(h.query_by_label("Picture layout").is_none(), "no English list name");
    let layout = h.get_by_role_and_label(egui::accesskit::Role::ComboBox, t("Picture layout"));
    assert_eq!(layout.value().as_deref(), Some(t("1 picture")), "the chosen layout is shown translated");
    // With the real fonts the album body is taller than the harness, so the combo sits below the
    // scrolled-out part of the body; bring it into view before opening it.
    layout.scroll_to_me();
    h.run_steps(2);
    h.get_by_role_and_label(egui::accesskit::Role::ComboBox, t("Picture layout")).click();
    h.run_steps(2);
    assert!(h.query_by_label("Fit to slide").is_none(), "no English item");
    h.get_by_label(t("Fit to slide")).click();
    h.run_steps(2);
    let fields = &h.state().dialog.as_ref().expect("the album stays open").fields;
    assert_eq!(fields.get("layout").map(String::as_str), Some("fit"), "the English key is stored");
}

/// The fixed reasons deckcraft-update gives for a failed check or install reach the update window
/// (through `t()`) or the window after a failed install (the helper's recorded outcome), so each
/// has a Korean row. Reasons with a value in them wait for t_dynamic.
#[test]
fn update_failure_reasons_are_korean() {
    // Never shown: the helper checks its own invocation before it records an outcome, launch
    // errors are only logged, and the rest are test doubles.
    const INTERNAL: [&str; 7] = [
        "Slide's update restart record is too large.",
        "Invalid Slide update helper arguments.",
        "Missing Slide update plan.",
        "No update folder.",
        "The update helper is outside Slide's own update folder.",
        "An update restart file must have an absolute path.",
        "Simulated native restart failure",
    ];
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../update/src");
    let mut reasons = BTreeSet::new();
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        for opener in ["Err(\"", "ok_or(\"", "ok_or_else(|| \"", "map_err(|_| \""] {
            for (at, _) in text.match_indices(opener) {
                let rest = &text[at + opener.len()..];
                let Some(end) = rest.find('"') else { continue };
                let s = &rest[..end];
                if !s.contains(['{', '\\']) && !INTERNAL.contains(&s) {
                    reasons.insert(s.to_string());
                }
            }
        }
    }
    assert!(reasons.len() > 30, "found the updater's reasons: {reasons:?}");
    let missing: Vec<&String> = reasons.iter().filter(|r| !has_ko(r)).collect();
    assert!(missing.is_empty(), "no Korean row: {missing:#?}");
}

/// The non-test `.rs` files under `dir`, recursively.
fn source_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if path.is_dir() {
            source_files(&path, out);
        } else if name.ends_with(".rs") && name != "tests.rs" && !name.ends_with("_tests.rs") {
            out.push(path);
        }
    }
}

fn assert_no_problems(files: &[PathBuf]) {
    let problems: Vec<String> = files.iter().flat_map(|f| audit::problems(f)).collect();
    assert!(problems.is_empty(), "no Korean row (deckcraft.tsv) or keep_english.txt entry, or not wrapped in t():\n{}", problems.join("\n"));
}

/// The canvas, the slide show and the slide sorter: every interface string has a Korean row or a
/// reason to stay English, and egui's own calls look it up.
#[test]
fn korean_coverage_canvas_show_sorter() {
    let mut files = vec![src().join("canvas.rs"), src().join("show.rs"), src().join("sorter.rs")];
    for dir in ["canvas", "show", "sorter"] {
        source_files(&src().join(dir), &mut files);
    }
    assert!(files.len() > 15, "found the files: {files:?}");
    assert_no_problems(&files);
}

/// The top-level files outside the ribbon, dialogs, menus, panels, backstage, i18n and the shared
/// widgets (the status bar, toasts, the presenter view, the app's frame, files and keys …), with
/// `app/`, `capture/` and `icons/`.
#[test]
fn korean_coverage_top_level() {
    const ELSEWHERE: [&str; 10] =
        ["ribbon.rs", "dialogs.rs", "menus.rs", "panels.rs", "backstage.rs", "i18n.rs", "widgets.rs", "canvas.rs", "show.rs", "sorter.rs"];
    let mut files = Vec::new();
    for entry in std::fs::read_dir(src()).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if path.is_file() && name.ends_with(".rs") && name != "tests.rs" && !name.ends_with("_tests.rs") && !ELSEWHERE.contains(&name.as_str()) {
            files.push(path);
        }
    }
    for dir in ["app", "capture", "icons"] {
        source_files(&src().join(dir), &mut files);
    }
    files.sort();
    assert!(files.iter().any(|f| f.ends_with("status.rs")) && files.len() > 30, "found the files: {files:?}");
    assert_no_problems(&files);
}

/// An engine error with values in it reads in Korean through its template, the values as they were.
#[test]
fn dynamic_message_keeps_values() {
    let err = deckcraft_engine::cmd::file::open_checked("deck v2 (final).key", b"not a deck").err().unwrap().to_string();
    assert_eq!(err, "deck v2 (final).key: not a presentation DeckCraft can read", "the engine's sentence");
    set_current(resolve("en"));
    assert_eq!(t_dynamic(&err), err, "English stays as written");
    set_current(resolve("ko"));
    assert_eq!(t_dynamic(&err), "deck v2 (final).key: DeckCraft가 읽을 수 있는 프레젠테이션이 아닙니다");
    assert_eq!(
        t_dynamic("/tmp/a: b.pptx: file size 3221225472 exceeds 2 GiB limit"),
        "/tmp/a: b.pptx: 파일 크기(3221225472)가 2 GiB 제한을 넘습니다"
    );
    // A plain row still wins, and the longest matching template does.
    assert_eq!(t_dynamic("nothing selected"), "선택한 항목이 없습니다");
    set_current(resolve("en"));
}

/// A sentence no template matches is shown exactly as the engine wrote it, never half translated.
#[test]
fn dynamic_message_without_template_stays_english() {
    set_current(resolve("ko"));
    for s in [
        "deck.pptx: not a presentation DeckCraft can open",
        "the cache folder could not be created: permission denied",
        ": not a presentation DeckCraft can read",
        "",
    ] {
        let shown = t_dynamic(s);
        assert!(matches!(shown, std::borrow::Cow::Borrowed(_)) && shown == s, "{s:?} became {shown:?}");
    }
    set_current(resolve("en"));
}

/// The status-bar sentence for a command that can't run reads in Korean with its reason; the
/// command id stays as written.
#[test]
fn disabled_reason_message_is_fully_korean() {
    let mut s = deckcraft_engine::Session::with_new();
    let err = s.execute("edit.undo", &serde_json::json!({})).err().unwrap().to_string();
    assert_eq!(err, "command `edit.undo` is not available right now: nothing to undo");
    set_current(resolve("ko"));
    assert_eq!(t_dynamic(&err), "지금은 `edit.undo` 명령을 쓸 수 없습니다: 실행 취소할 작업이 없습니다");
    // A reason without a row stays English inside the Korean sentence.
    assert_eq!(t_dynamic("command `x.y` is not available right now: zz no row"), "지금은 `x.y` 명령을 쓸 수 없습니다: zz no row");
    set_current(resolve("en"));
}

/// Picture Format › Compress Pictures saves its result in English (ribbon/picture_format.rs); the
/// status bar shows it through the template.
#[test]
fn compress_status_is_translated() {
    set_current(resolve("ko"));
    assert_eq!(t_dynamic("Compressed 3 picture(s): 10 KB → 5 KB"), "그림 3개 압축: 10 KB → 5 KB");
    set_current(resolve("en"));
}

/// The status message stays English in `app.status` (the control channel reports it) and is
/// translated as the status bar draws it; the bar's own controls, the toasts and egui's own names
/// ("Resize panel", "Close window") are Korean too.
#[test]
fn status_bar_translates_at_draw_time() {
    use egui_kittest::kittest::Queryable;
    let mut h = korean_harness();
    let msg = "command `edit.undo` is not available right now: nothing to undo";
    h.state_mut().set_status(msg);
    h.state_mut().notify("Aptos isn't installed on this computer. Text that uses it is shown in a similar font.");
    h.run_steps(3);
    assert_eq!(h.state().status.as_ref().map(|s| s.0.as_str()), Some(msg), "saved in English");
    assert!(h.query_by_value(msg).is_none() && h.query_by_label(msg).is_none(), "the English sentence is not shown");
    assert!(h.query_by_value(&t_dynamic(msg)).is_some(), "the status bar shows {:?}", t_dynamic(msg));
    assert!(h.query_by_label_contains("Aptos 글꼴이 설치되어 있지 않습니다").is_some(), "the toast is Korean");
    assert!(h.query_by_value(t("Slide 1 of 1")).is_none());
    assert!(h.query_by_value(&fmt(t("Slide {n} of {total}"), &[("n", "1"), ("total", "1")])).is_some(), "the slide counter");
    for en in [
        "Language",
        "Fit slide to current window",
        "Zoom level",
        "Zoom In",
        "Zoom Out",
        "Reading View",
        "Slide Sorter",
        "Normal",
        "Notes",
        "Comments",
    ] {
        assert!(h.query_all_by_label(en).next().is_none(), "{en:?} is shown in English");
        assert!(h.query_all_by_label(t(en)).next().is_some(), "{en:?} is shown as {:?}", t(en));
    }
    assert!(h.query_by_role_and_label(egui::accesskit::Role::Slider, t("Zoom")).is_some());
    assert_eq!(h.query_all_by_label("Slide Show").count(), 0, "no English Slide Show (status bar and ribbon tab)");
    assert!(
        h.query_all_by_label("Resize panel").next().is_none() && h.query_all_by_label(t("Resize panel")).next().is_some(),
        "egui's splitter name"
    );
    h.state_mut().dialog = Some(crate::dialogs::Dialog::new("font"));
    h.run_steps(3);
    assert!(
        h.query_all_by_label("Close window").next().is_none() && h.query_all_by_label(t("Close window")).next().is_some(),
        "egui's window close button"
    );
}

/// An empty title placeholder's prompt is drawn in Korean on the canvas (the renderer asks for its
/// wording); the document's title stays empty, and English draws what it drew before.
#[test]
fn placeholder_prompts_are_korean() {
    let s = deckcraft_engine::Session::with_new();
    let doc = &s.active().unwrap().doc;
    let slide = doc.slides.first().unwrap();
    let title = slide.shapes.iter().find(|x| x.ph_type().is_some_and(|k| k.is_title())).expect("a title placeholder");
    assert!(title.text.as_ref().is_none_or(|t| t.is_empty()), "the new slide's title is empty");
    for prompt in ["Click to add title", "Click to add subtitle", "Click to add text", "Click icon to add picture"] {
        assert!(has_ko(prompt), "{prompt:?} has a Korean row");
    }
    let draw = |wording: Option<fn(&str) -> &str>| {
        deckcraft_render::render_slide(doc, 0, &deckcraft_render::RenderOpts { scale: 0.5, edit: true, prompt_text: wording, ..Default::default() })
            .pixels
    };
    let english = draw(None);
    set_current(resolve("en"));
    assert!(draw(Some(t)) == english, "English draws the same prompts");
    set_current(resolve("ko"));
    assert!(draw(Some(t)) != english, "Korean draws other prompts");
    set_current(resolve("en"));
    assert!(
        s.active().unwrap().doc.slides[0].shapes.iter().all(|x| x.text.as_ref().is_none_or(|t| t.is_empty())),
        "nothing was written into the slide"
    );
}

/// The canvas texture is cached; switching the interface language draws the slide again, so its
/// placeholder prompts follow the language. (Slide thumbnails draw no prompts.)
#[test]
fn canvas_redraws_its_prompts_when_the_language_changes() {
    let s = deckcraft_engine::Session::with_new();
    let doc = &s.active().unwrap().doc;
    let ctx = egui::Context::default();
    let mut tex = crate::textures::Textures::default();
    let visible = deckcraft_render::Region::slide(320, 180);
    let draw = |tex: &mut crate::textures::Textures| {
        let _ = tex.slide(&ctx, doc, 0, (320, 180), visible, true, 0, false, &[], None, None).unwrap();
        tex.canvas_renders
    };
    set_current(resolve("en"));
    let first = draw(&mut tex);
    assert_eq!(draw(&mut tex), first, "the same language reuses the texture");
    set_current(resolve("ko"));
    let korean = draw(&mut tex);
    assert!(korean > first, "Korean draws the slide again");
    assert_eq!(draw(&mut tex), korean, "and caches that");
    set_current(resolve("en"));
    assert!(draw(&mut tex) > korean, "back to English draws it again");
}

/// A font's name in the missing-fonts toast is the document's: "Symbol" or "System" stays, though
/// the catalogs translate those words; a disabled command's reason is wording and is translated.
#[test]
fn dynamic_message_keeps_names_that_are_also_words() {
    set_current(resolve("ko"));
    for names in [&["Symbol"][..], &["Modern", "System"], &["Symbol", "Modern", "System", "Aptos"]] {
        let fonts: Vec<String> = names.iter().map(|n| n.to_string()).collect();
        let shown = t_dynamic(&crate::toast::missing_fonts_message(&fonts).unwrap()).into_owned();
        assert!(shown.contains("글꼴이 설치되어 있지 않습니다"), "{shown}");
        for n in names.iter().take(3) {
            assert!(shown.contains(n), "{n:?} stays as written: {shown}");
        }
    }
    assert_ne!(t("Symbol"), "Symbol", "the word has a row (the test means something)");
    assert_eq!(
        t_dynamic("command `edit.undo` is not available right now: nothing to undo"),
        "지금은 `edit.undo` 명령을 쓸 수 없습니다: 실행 취소할 작업이 없습니다"
    );
    set_current(resolve("en"));
}

/// A wording value that is a message with a value of its own (a disabled command's reason "no
/// slide 3") is translated through its template too, one level deep; a value that matches no row
/// stays as written, and the fonts in a nested message stay names.
#[test]
fn dynamic_message_translates_its_reason_one_level_deep() {
    set_current(resolve("ko"));
    assert_eq!(
        t_dynamic("command `slide.go` is not available right now: no slide 3"),
        "지금은 `slide.go` 명령을 쓸 수 없습니다: 슬라이드가 없습니다: 3"
    );
    assert_eq!(t_dynamic("command `slide.go` is not available right now: something new"), "지금은 `slide.go` 명령을 쓸 수 없습니다: something new");
    let fonts = crate::toast::missing_fonts_message(&["Symbol".to_string()]).unwrap();
    let shown = t_dynamic(&format!("command `x.y` is not available right now: {fonts}")).into_owned();
    assert!(shown.contains("Symbol") && !shown.contains("기호"), "{shown}");
    set_current(resolve("en"));
    assert_eq!(t_dynamic("command `slide.go` is not available right now: no slide 3"), "command `slide.go` is not available right now: no slide 3");
}

/// A new presentation is "프레젠테이션1" on screen in Korean; the engine's name (the Save dialog's
/// file name, the control channel's title) stays "Presentation1", and a saved file or another name
/// is shown as it is.
#[test]
fn new_presentation_name_is_translated_for_display_only() {
    let mut session = deckcraft_engine::Session::with_new();
    let d = session.active().unwrap();
    assert_eq!(d.title(), "Presentation1");
    set_current(resolve("ko"));
    assert_eq!(crate::i18n::doc_title(d), "프레젠테이션1");
    set_current(resolve("en"));
    assert_eq!(crate::i18n::doc_title(d), "Presentation1");
    set_current(resolve("ko"));
    let d = session.active_mut().unwrap();
    d.title = "Presentation draft".into();
    assert_eq!(crate::i18n::doc_title(d), "Presentation draft");
    d.title = "Presentation2".into();
    d.path = Some("/tmp/Presentation2.pptx".into());
    assert_eq!(crate::i18n::doc_title(d), "Presentation2", "a saved file keeps its file's name");
    set_current(resolve("en"));
}

/// The fixed text before the last value is the last match, so a value holding it keeps it.
#[test]
fn template_splits_before_the_last_value_at_its_last_match() {
    let tpl = Template::parse("{name}: {app} can't open this kind of file").unwrap();
    assert_eq!(tpl.capture("a: b.pptx: Slide can't open this kind of file"), Some(vec![("name", "a: b.pptx"), ("app", "Slide")]));
    set_current(resolve("ko"));
    assert_eq!(t_dynamic("a: b.pptx: Slide can't open this kind of file"), "a: b.pptx: Slide에서 열 수 없는 종류의 파일입니다");
    set_current(resolve("en"));
}

/// A file the system asked to open that isn't a presentation (apple_events.rs's status "Couldn't
/// open {path}: {e}" with the engine's real error "deck.key: not a presentation DeckCraft can
/// read"): the sentence splits at the path, not at the last ": ", and the error inside is
/// translated too.
#[test]
fn couldnt_open_splits_before_the_engine_error() {
    let dir = std::env::temp_dir().join(format!("deckcraft-i18n-open-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("deck.key");
    std::fs::write(&path, b"not a deck").unwrap();
    let path = path.to_string_lossy().to_string();
    let mut app = crate::SlideApp::new(deckcraft_engine::Session::new(), crate::Services::default());
    let e = app.open_path(&path).unwrap_err();
    assert!(e.ends_with(": not a presentation DeckCraft can read"), "the engine's error: {e}");
    set_current(resolve("ko"));
    let inner = t_dynamic(&e).into_owned();
    let shown = t_dynamic(&format!("Couldn't open {path}: {e}")).into_owned();
    set_current(resolve("en"));
    assert!(inner.contains("DeckCraft가 읽을 수 있는 프레젠테이션이 아닙니다"), "{inner}");
    assert_eq!(shown, format!("{path}을(를) 열 수 없습니다: {inner}"));
}

/// The slide's Layout menu (canvas right-click) shows every built-in layout in Korean; the command
/// keeps the layout's id and the document its English name.
#[test]
fn layout_menu_names_are_korean() {
    use egui_kittest::kittest::Queryable;
    let mut h = korean_harness();
    let at = h.state().slide_rect.unwrap().min + egui::vec2(6.0, 6.0);
    h.event(egui::Event::PointerMoved(at));
    h.step();
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Secondary, pressed, modifiers: egui::Modifiers::NONE });
        h.step();
    }
    h.run_steps(2);
    // The ribbon's Layout button is above the slide; the menu's item opens below the click.
    h.get_all_by_label(t("Layout")).find(|n| n.rect().min.y > at.y).expect("the menu's Layout item").hover();
    h.run_steps(3);
    let names: Vec<String> = h.state().session.active().unwrap().doc.masters[0].layouts.iter().map(|l| l.name.clone()).collect();
    assert!(names.len() >= 11, "{names:?}");
    for name in &names {
        assert_ne!(tc("layout", name), name.as_str(), "{name:?} has a Korean name");
        assert!(h.query_by_label(tc("layout", name)).is_some(), "the menu lists {name:?} as {:?}", tc("layout", name));
        assert!(h.query_by_label(name).is_none(), "{name:?} is not shown in English");
    }
    h.get_by_label(tc("layout", "Blank")).click();
    h.run_steps(3);
    let d = h.state().session.active().unwrap();
    let slide = &d.doc.slides[d.selection.slide];
    let layout = d.doc.masters[0].layouts.iter().find(|l| l.id == slide.layout).unwrap();
    assert_eq!(layout.name, "Blank", "the document keeps the English name");
}

/// The task panes (format pane, selection pane, animation pane, thumbnails, notes, comments …):
/// every interface string has a Korean row or a reason to stay English, and egui's calls look it up.
#[test]
fn korean_coverage_panels() {
    let mut files = vec![src().join("panels.rs")];
    source_files(&src().join("panels"), &mut files);
    assert!(files.len() > 12, "found the files: {files:?}");
    assert_no_problems(&files);
}

/// The File backstage (start, new, open, info, print, export …).
#[test]
fn korean_coverage_backstage() {
    let mut files = vec![src().join("backstage.rs")];
    source_files(&src().join("backstage"), &mut files);
    assert!(files.len() > 3, "found the files: {files:?}");
    assert_no_problems(&files);
}

/// All of `src/` at once: a file no area test lists (a new top-level module, a new folder) is
/// still checked. The area tests stay to say where a failure is.
#[test]
fn korean_coverage_all() {
    let found = audit::candidates(&src());
    assert!(found.len() > 2000, "the scan sees the whole crate: {}", found.len());
    for area in ["ribbon", "dialogs", "backstage", "panels", "canvas", "show", "sorter", "app", "top:status", "top:menus"] {
        assert!(found.iter().any(|c| audit::area(&c.file) == area), "the scan reaches {area}");
    }
    let problems = audit::problems(&src());
    assert!(problems.is_empty(), "no Korean row (deckcraft.tsv) or keep_english.txt entry, or not wrapped in t():\n{}", problems.join("\n"));
}

/// An id's placeholder (`{cmd}`, `{0}`) before the last value is the first match: a command id has
/// no "`: " in it, while the engine's message after it may ("gradient needs at least two
/// `stops`: …"). A file name before the last value is still the last match.
#[test]
fn template_splits_after_an_id_at_its_first_match() {
    let mut s = deckcraft_engine::Session::with_new();
    let id = s.execute("shape.insert", &serde_json::json!({"preset": "rect", "rect": [10, 10, 50, 50]})).unwrap()["id"].clone();
    s.execute("edit.select", &serde_json::json!({"ids": [id]})).unwrap();
    let err = s.execute("shape.fill", &serde_json::json!({"gradient": {"stops": []}})).err().unwrap().to_string();
    let msg = "gradient needs at least two `stops`: [[0, \"#fff\"], [1, \"#000\"]]";
    assert_eq!(err, format!("invalid parameters for `shape.fill`: {msg}"), "the engine's sentence");
    let tpl = Template::parse("invalid parameters for `{cmd}`: {msg}").unwrap();
    assert_eq!(tpl.capture(&err), Some(vec![("cmd", "shape.fill"), ("msg", msg)]));
    let tpl = Template::parse("{name}: {app} can't open this kind of file").unwrap();
    assert_eq!(tpl.capture("a: b.pptx: Slide can't open this kind of file"), Some(vec![("name", "a: b.pptx"), ("app", "Slide")]));
    set_current(resolve("ko"));
    let shown = t_dynamic(&err);
    assert!(shown.contains("`shape.fill`") && shown.contains(msg) && !shown.contains("invalid parameters"), "{shown}");
    set_current(resolve("en"));
}

/// The slide thumbnail's Layout menu shows the built-in layouts translated; the command takes the
/// layout's id, so the document keeps "Title Slide". A layout the user named like a built-in one
/// ("Title Only") reads translated in the menu too, and its saved name stays as typed.
#[test]
fn layout_names_are_translated_for_display_only() {
    use egui_kittest::kittest::Queryable;
    let mut h = korean_harness();
    let layouts = |h: &egui_kittest::Harness<'_, crate::SlideApp>| -> Vec<(u32, String)> {
        h.state().session.active().unwrap().doc.masters[0].layouts.iter().map(|l| (l.id.0, l.name.clone())).collect()
    };
    let built_in = layouts(&h);
    h.state_mut().run("master.insertLayout", serde_json::json!({"name": "Title Only"})).unwrap();
    let _ = h.state_mut().run("view.closeMaster", serde_json::json!({}));
    h.run_steps(3);
    let before = layouts(&h);
    let mine = before.iter().find(|l| !built_in.contains(l)).unwrap().0;
    assert_eq!(before.iter().filter(|(_, n)| n == "Title Only").count(), 2, "{before:?}");
    // The slide's thumbnail menu (the ribbon's Layout button is above it).
    let thumb = h.get_by_label(&fmt(tc("number", "Slide {n}"), &[("n", "1")])).rect();
    h.get_by_label(&fmt(tc("number", "Slide {n}"), &[("n", "1")])).click_secondary();
    h.run_steps(3);
    h.get_all_by_label(t("Layout")).find(|n| n.rect().min.y > thumb.min.y).expect("the menu's Layout item").hover();
    h.run_steps(3);
    for (_, name) in &before {
        assert!(h.query_all_by_label(tc("layout", name)).next().is_some(), "the menu lists {name:?} as {:?}", tc("layout", name));
        assert!(h.query_by_label(name).is_none(), "{name:?} is not shown in English");
    }
    assert_eq!(tc("layout", "Title Slide"), "제목 슬라이드");
    h.get_by_label("제목 슬라이드").click();
    h.run_steps(3);
    let layout_of_slide = |h: &egui_kittest::Harness<'_, crate::SlideApp>| {
        let d = h.state().session.active().unwrap();
        let slide = &d.doc.slides[d.selection.slide];
        d.doc.masters[0].layouts.iter().find(|l| l.id == slide.layout).map(|l| (l.id.0, l.name.clone())).unwrap()
    };
    assert_eq!(layout_of_slide(&h).1, "Title Slide", "the document keeps the English name");
    // The user's "Title Only" is the menu's last 제목만; picking it keeps its name as typed.
    h.get_by_label(&fmt(tc("number", "Slide {n}"), &[("n", "1")])).click_secondary();
    h.run_steps(3);
    h.get_all_by_label(t("Layout")).find(|n| n.rect().min.y > thumb.min.y).unwrap().hover();
    h.run_steps(3);
    h.get_all_by_label(tc("layout", "Title Only")).max_by(|a, b| a.rect().min.y.total_cmp(&b.rect().min.y)).unwrap().click();
    h.run_steps(3);
    assert_eq!(layout_of_slide(&h), (mine, "Title Only".to_string()));
    assert_eq!(layouts(&h), before, "no layout was renamed");
}

/// A label, or a combo box's or a field's text.
fn shows(h: &egui_kittest::Harness<'_, crate::SlideApp>, s: &str) -> bool {
    use egui_kittest::kittest::Queryable;
    h.query_all_by_label(s).next().is_some() || h.query_all_by_value(s).next().is_some()
}

/// The File tab's frame on `page`: one side-menu entry and the heading say the page's Korean name,
/// the side menu's names are all different (Info 정보, About 앱 정보), nothing says `en`.
fn assert_backstage_frame(h: &egui_kittest::Harness<'_, crate::SlideApp>, page: &str, en: &str) {
    use egui_kittest::kittest::{NodeT, Queryable};
    let nav_w = crate::backstage::NAV_W + 20.0;
    let name = tc("backstage", en);
    // From the back arrow down (the title bar above has its own Home).
    let top = h.get_by_role_and_label(egui::accesskit::Role::Button, t("Back")).rect().top();
    let menu: Vec<String> = h
        .query_all_by_role(egui::accesskit::Role::Button)
        .filter(|n| n.rect().right() <= nav_w && n.rect().left() < 20.0 && n.rect().top() >= top)
        .filter_map(|n| n.accesskit_node().label())
        .collect();
    let unique: BTreeSet<&String> = menu.iter().collect();
    assert!(menu.len() >= 10 && unique.len() == menu.len(), "{page}: the side menu {menu:?}");
    assert_eq!(menu.iter().filter(|m| *m == name).count(), 1, "{page}: one side-menu entry says {name:?}");
    let heading = |n: &egui_kittest::Node<'_>| {
        n.rect().left() > nav_w && (n.accesskit_node().label().as_deref() == Some(name) || n.value().as_deref() == Some(name))
    };
    assert!(h.query_all_by_role(egui::accesskit::Role::Label).any(|n| heading(&n)), "{page}: the heading says {name:?}");
    assert!(!shows(h, en), "{page}: {en:?} is not shown in English");
    assert!(!shows(h, "Back"));
}

/// What `page` of the File tab shows in Korean (and keeps as written).
fn assert_backstage_page(h: &egui_kittest::Harness<'_, crate::SlideApp>, page: &str) {
    use egui_kittest::kittest::Queryable;
    match page {
        "new" => {
            for en in ["Blank Presentation", "Harbor", "Briefing", "Templates"] {
                assert!(has_ko(en) && h.query_all_by_label(t(en)).next().is_some(), "New shows {en:?} as {:?}", t(en));
                assert!(h.query_all_by_label(en).next().is_none(), "{en:?} in English");
            }
            assert!(h.query_by_label("DeckCraft").is_some(), "the DeckCraft theme keeps its name");
        }
        "open" => {
            assert!(h.query_by_label("Quarterly Review.pptx").is_some(), "the file's name as written");
            assert!(h.query_all_by_label(t("Remove from Recent")).next().is_some() && !shows(h, "Remove from Recent"));
            assert!(h.query_by_label(t("Presentations")).is_some() && h.query_by_label(t("Browse")).is_some());
        }
        "info" => {
            for en in ["Properties", "Title", "Slides", "Location", "Inspect Presentation", "Manage Presentation"] {
                assert!(h.query_all_by_label(t(en)).next().is_some(), "Info shows {en:?} as {:?}", t(en));
            }
            assert!(h.query_by_label(tc("property", "Comments")).is_some(), "the Comments property is 메모");
            let props = h.state().session.active().unwrap().doc.props.clone();
            assert_eq!((props.title.as_str(), props.comments.as_str()), ("Title", "Notes"), "the document's values stay");
        }
        "print" => {
            for en in ["Print All Slides", "Full Page Slides", "Print Hidden Slides", "Frame Slides", "Settings"] {
                assert!(shows(h, t(en)), "Print shows {en:?} as {:?}", t(en));
                assert!(!shows(h, en), "{en:?} in English");
            }
            assert!(shows(h, "Letter (8.5 × 11 in)"), "the paper's name as written");
        }
        "export" => {
            for en in ["Create PDF Document", "Create Handouts", "Change File Type", "Keeps the layout, fonts and pictures"] {
                assert!(h.query_all_by_label_contains(t(en)).next().is_some(), "Export shows {en:?} as {:?}", t(en));
                assert!(h.query_all_by_label_contains(en).next().is_none() && !shows(h, en), "{en:?} in English");
            }
        }
        "about" => {
            assert_eq!(tc("appearance", "Appearance"), "화면 모드");
            assert!(shows(h, "화면 모드") && !shows(h, t("Appearance")), "the appearance menu is 화면 모드, not {:?}", t("Appearance"));
            assert!(shows(h, "밝은 모드") || shows(h, "어두운 모드"), "its value says 밝은/어두운 모드");
            assert!(!shows(h, "Light") && !shows(h, "Dark") && !shows(h, "Appearance"));
        }
        _ => assert!(shows(h, t("PowerPoint Presentation (*.pptx)")), "Save As shows the type in Korean"),
    }
}

/// The File tab in Korean: every page's heading and the side menu, the New page's tiles (a theme's
/// and a template's name translated, the DeckCraft theme as it is), a recent file's name as written,
/// the Info page's labels (not the document's values), Print's settings (the paper names as
/// written), Export's tasks and About's appearance menu. Searching New in Korean finds a theme by
/// its Korean name.
#[test]
fn backstage_pages_are_korean() {
    use egui_kittest::kittest::Queryable;
    let mut h = korean_harness();
    h.state_mut().ui.recent = vec!["/nowhere/Quarterly Review.pptx".into()];
    h.state_mut().run("file.properties", serde_json::json!({"title": "Title", "comments": "Notes"})).unwrap();
    let pages =
        [("new", "New"), ("open", "Open"), ("info", "Info"), ("print", "Print"), ("export", "Export"), ("saveAs", "Save As"), ("about", "About")];
    for (page, en) in pages {
        h.state_mut().run("app.backstage", serde_json::json!({"page": page})).unwrap();
        h.run_steps(3);
        assert_backstage_frame(&h, page, en);
        assert_backstage_page(&h, page);
    }
    h.state_mut().run("app.backstage", serde_json::json!({"page": "new"})).unwrap();
    h.run_steps(3);
    h.state_mut().backstage.as_mut().unwrap().query = t("Harbor").into();
    h.run_steps(3);
    assert!(h.query_by_label(t("Harbor")).is_some(), "a Korean search finds the Harbor theme");
    assert!(h.query_by_label(t("Briefing")).is_none(), "and leaves the rest out");
}

/// Format Shape's choices the source check can't follow (their context is a variable): every 3-D
/// preset the pane lists or a file can bring (the engine's `ALL_*` lists: an opened shape shows its
/// legacy camera or light) has its row by id, and the compound line's Single and Double, a cap's
/// Square, a join's Bevel and the gradient direction buttons' short names have theirs.
#[test]
fn format_pane_context_rows_cover_their_choices() {
    use deckcraft_engine::cmd::three_d::{ALL_CAMERAS, ALL_LIGHTS, ALL_MATERIALS, BEVELS, CAMERAS, LIGHTS, MATERIALS};
    for (all, listed) in [(ALL_CAMERAS, CAMERAS), (ALL_LIGHTS, LIGHTS), (ALL_MATERIALS, MATERIALS)] {
        assert!(listed.iter().all(|id| all.contains(id)), "the pane's lists are part of the engine's");
    }
    set_current(resolve("ko"));
    for (context, ids) in [("bevel", BEVELS), ("camera", ALL_CAMERAS), ("material", ALL_MATERIALS), ("light", ALL_LIGHTS)] {
        for id in ids.iter().chain(["none"].iter().filter(|_| context == "bevel")) {
            assert_ne!(tc(context, id), *id, "{context} {id:?} has a Korean row");
        }
    }
    for (context, en, ko) in
        [("compound", "Single", "단일선"), ("compound", "Double", "이중선"), ("cap", "Square", "사각형"), ("join", "Bevel", "빗면형")]
    {
        assert_eq!(tc(context, en), ko);
    }
    for short in ["center", "top left", "top right", "bottom left", "bottom right"] {
        assert_ne!(tc("gradient", short), short, "{short:?}");
    }
    for (context, en) in [("reflection", "Tight"), ("arrowsize", "Small"), ("arrowsize", "Large")] {
        assert_ne!(tc(context, en), t(en), "{context} {en:?} has its own (noun) row");
    }
    set_current(resolve("en"));
}

/// The engine's words for a changed slide in Review › Compare (deckcraft-engine compare.rs
/// `slide_details`: "layout", "hidden" …) all have a `revision` row, and its sentences with a shape's
/// name a template.
#[test]
fn revision_details_have_rows() {
    let source = std::fs::read_to_string(src().join("../../engine/src/compare.rs")).unwrap();
    // A slide's words (`slide_details`) and a shape's, listed after its name (`shape_details`).
    let mut words: Vec<&str> = Vec::new();
    for f in ["fn slide_details(", "fn shape_details("] {
        let body = source.split(f).nth(1).and_then(|b| b.split("\n}\n").next()).unwrap();
        words.extend(body.split('"').skip(1).step_by(2).filter(|w| !w.contains('{') && w.chars().all(|c| c.is_ascii_lowercase() || c == ' ')));
    }
    for w in [
        "order of objects",
        "layout",
        "hidden",
        "shown",
        "text",
        "text formatting",
        "moved",
        "resized",
        "rotated",
        "flipped",
        "shape",
        "formatting",
        "picture",
        "properties",
    ] {
        assert!(words.contains(&w), "found the engine's words: {words:?}");
    }
    set_current(resolve("ko"));
    for w in &words {
        assert_ne!(tc("revision", w), *w, "{w:?} has a revision row");
    }
    for s in ["Title 1 deleted (only in the deck)", "Title 1 inserted (only in the file)"] {
        let shown = t_dynamic(s);
        assert!(shown.starts_with("Title 1 ") && !shown.contains("only in"), "{shown}");
    }
    set_current(resolve("en"));
}

/// Review › Compare's pane in Korean: a changed slide's heading and its details (the slide hidden,
/// its layout changed) read in Korean; nothing of the engine's English is left.
#[test]
fn revisions_pane_details_are_korean() {
    use egui_kittest::kittest::Queryable;
    let mut h = korean_harness();
    let rect = h.state_mut().run("shape.insert", serde_json::json!({"preset": "rect", "rect": [40, 40, 120, 80]})).unwrap()["id"].clone();
    let before = h.state_mut().run("file.saveBytes", serde_json::json!({})).unwrap()["data"].as_str().unwrap().to_string();
    h.state_mut().run("shape.move", serde_json::json!({"ids": [rect], "dx": 30, "dy": 0})).unwrap();
    h.state_mut().run("slide.hide", serde_json::json!({"index": 0})).unwrap();
    let blank = h.state().session.active().unwrap().doc.masters[0].layouts.iter().find(|l| l.name == "Blank").unwrap().id.0;
    h.state_mut().run("slide.layout", serde_json::json!({"layout": blank})).unwrap();
    h.state_mut().run("review.compare", serde_json::json!({"name": "before.deckcraft", "data": before})).unwrap();
    h.state_mut().ui.pane = Some("revisions".into());
    h.run_steps(3);
    let v = h.state_mut().run("review.revisions", serde_json::json!({})).unwrap();
    let details: Vec<String> = v["changes"][0]["details"].as_array().unwrap().iter().map(|d| d.as_str().unwrap().to_string()).collect();
    assert!(details.iter().any(|d| d == "layout") && details.iter().any(|d| d == "hidden" || d == "shown"), "{details:?}");
    assert!(details.iter().any(|d| d.ends_with(": moved")), "a shape's line: {details:?}");
    for d in &details {
        let ko = crate::panels::revision_detail(d).into_owned();
        assert_ne!(&ko, d, "{d:?} has Korean");
        assert!(h.query_by_label(&format!("• {ko}")).is_some(), "{d:?} is shown as {ko:?}");
        assert!(h.query_by_label(&format!("• {d}")).is_none(), "{d:?} in English");
    }
    assert!(h.query_by_label(&fmt(t("Slide {n} changed"), &[("n", "1")])).is_some(), "the change's heading");
}

/// English search on New is as it was: the sample deck's tile answers to its plain words, not to
/// its tile's colon ("deck:"), and a template to its own words.
#[test]
fn new_page_search_in_english_is_unchanged() {
    use egui_kittest::kittest::Queryable;
    let app = crate::SlideApp::new(deckcraft_engine::Session::with_new(), crate::Services::default());
    let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(1400.0, 900.0)).build_ui_state(
        |ui, app: &mut crate::SlideApp| {
            app.logic(ui.ctx());
            app.ui(ui);
        },
        app,
    );
    h.run_steps(3);
    h.state_mut().run("app.backstage", serde_json::json!({"page": "new"})).unwrap();
    for (query, sample, briefing) in
        [("deck:", false, false), ("deck northern", true, false), ("northern", true, false), ("check-in", false, true), ("", true, true)]
    {
        h.state_mut().backstage.as_mut().unwrap().query = query.into();
        h.run_steps(3);
        assert_eq!(h.query_by_label("Sample deck: Northern Lights").is_some(), sample, "{query:?}: the sample deck");
        assert_eq!(h.query_by_label("Briefing").is_some(), briefing, "{query:?}: Briefing");
    }
}

/// Format Shape › Size & Properties says 기준 for the position's From (its reference point), and the
/// Selection pane's title is 선택 창 like the other panes' titles.
#[test]
fn pane_titles_and_position_origin_are_korean() {
    use egui_kittest::kittest::{NodeT, Queryable};
    let mut h = korean_harness();
    let id = h.state_mut().run("shape.insert", serde_json::json!({"preset": "rect", "rect": [40, 40, 120, 80]})).unwrap()["id"].clone();
    h.state_mut().run("edit.select", serde_json::json!({"ids": [id]})).unwrap();
    h.state_mut().ui.pane = Some("format".into());
    h.state_mut().ui.format_tab = "size".into();
    h.run_steps(3);
    assert!(h.query_by_role_and_label(egui::accesskit::Role::ComboBox, "기준").is_some(), "the position's From is 기준");
    assert!(h.query_by_role_and_label(egui::accesskit::Role::ComboBox, t("From")).is_none(), "not {:?}", t("From"));
    h.state_mut().ui.pane = Some("selection".into());
    h.run_steps(3);
    let title = |n: &egui_kittest::Node<'_>| n.accesskit_node().label().as_deref() == Some("선택 창") || n.value().as_deref() == Some("선택 창");
    assert!(h.query_all_by_role(egui::accesskit::Role::Label).any(|n| title(&n)), "the Selection pane's title");
}

/// A shape's changed line keeps the shape's name and translates its words; a name with ": " in it
/// stays whole (the list is after the last one); a word without a row leaves the line as the
/// engine wrote it; English is as written.
#[test]
fn revision_detail_keeps_the_shape_name() {
    use crate::panels::revision_detail;
    set_current(resolve("ko"));
    assert_eq!(revision_detail("Rectangle 3: text, moved"), "Rectangle 3: 텍스트, 이동됨");
    assert_eq!(revision_detail("Note: shape: text formatting, resized"), "Note: shape: 텍스트 서식, 크기 변경됨");
    assert_eq!(revision_detail("layout"), "레이아웃");
    assert_eq!(revision_detail("Rectangle 3: text, zz no row"), "Rectangle 3: text, zz no row");
    assert_eq!(revision_detail("Title 1 deleted (only in the deck)"), "Title 1 삭제됨 (열린 프레젠테이션에만 있음)");
    assert_eq!(revision_detail("Title 1 inserted (only in the file)"), "Title 1 삽입됨 (비교한 파일에만 있음)");
    set_current(resolve("en"));
    for d in ["Rectangle 3: text, moved", "layout", "Title 1 deleted (only in the deck)"] {
        let shown = revision_detail(d);
        assert!(matches!(shown, std::borrow::Cow::Borrowed(_)) && shown == d, "{d:?} became {shown:?}");
    }
}

/// Labels and texts on screen that read as English only: an ASCII word, no Hangul. A label that
/// mixes Korean and English ("슬라이드 3 of 8") isn't caught: the source check and the area tests
/// cover those. A text
/// run (`TextRun`) is skipped: under a label it repeats the label's value, which is checked; under a
/// text field it is what the field holds (a typed or default value, the document's), not wording.
fn english_on_screen(h: &egui_kittest::Harness<'_, crate::SlideApp>) -> BTreeSet<String> {
    use egui::accesskit::Role;
    use egui_kittest::kittest::{NodeT, Queryable};
    let mut out = BTreeSet::new();
    for n in h.query_all(egui_kittest::kittest::by()) {
        let node = n.accesskit_node();
        if matches!(node.role(), Role::TextRun | Role::TextInput | Role::MultilineTextInput) {
            continue;
        }
        for s in [node.label(), n.value()].into_iter().flatten() {
            let s = s.trim();
            let hangul = s.chars().any(|c| ('\u{AC00}'..='\u{D7A3}').contains(&c) || ('\u{3131}'..='\u{318E}').contains(&c));
            if !hangul && s.split(|c: char| !c.is_ascii_alphabetic()).any(|w| w.len() >= 2) {
                out.insert(s.to_string());
            }
        }
    }
    out
}

/// What may read as English on a Korean screen, each for its reason (Global Constraints: names,
/// units and the document's own words stay as written):
/// - `keep_english.txt`'s entries;
/// - the document's words: its shapes' names (the Selection pane), its title (Info: the file's
///   name or the first slide's title);
/// - font family names (Replace Font, Theme Fonts);
/// - the proofing languages' own names (`squiggles::LANGS`), or their tags (`ar-SA`) where the
///   interface fonts can't draw the name;
/// - the AutoCorrect list's English words and the papers' names (Print, `PAPERS`): data shown as written;
/// - a number with a unit (`0.75 pt`, `in`): unit symbols stay;
/// - on the Date and Time dialog only (`screen`), a date or time in its en-US formats (month and
///   day names, AM/PM; Korean date formats are out of scope);
/// - a built-in theme's or colour scheme's name with a number after it (`DeckCraft 2`, a theme's
///   colour variant). Not keep_english's: "Slide 3" is a slide's name left in English.
fn allowed_english(h: &egui_kittest::Harness<'_, crate::SlideApp>, screen: &str, s: &str) -> bool {
    const UNITS: [&str; 5] = ["pt", "in", "cm", "mm", "px"];
    const DATE_WORDS: [&str; 35] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
        "Jan",
        "Feb",
        "Mar",
        "Apr",
        "Jun",
        "Jul",
        "Aug",
        "Sep",
        "Oct",
        "Nov",
        "Dec",
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
        "Sunday",
        "AM",
        "PM",
        "Sept",
        "Mon",
        "Sun",
    ];
    let app = h.state();
    let st = app.session.active().unwrap();
    let doc = &st.doc;
    let words: Vec<&str> = s.split(|c: char| !c.is_ascii_alphabetic()).filter(|w| !w.is_empty()).collect();
    let listed = |s: &str| {
        audit::keep_english().iter().any(|k| k == s)
            || doc.slides.iter().flat_map(|sl| &sl.shapes).any(|sh| sh.name == s)
            || doc.props.title == s
            || st.title() == s
            || crate::ribbon::font_families(app).iter().any(|f| f == s)
            || crate::squiggles::LANGS.iter().any(|(name, tag)| *name == s || *tag == s)
            || deckcraft_engine::cmd::text::AUTOCORRECT.iter().any(|(a, b)| *a == s || *b == s)
            || deckcraft_engine::cmd::file::PAPERS.iter().any(|p| p.1 == s)
    };
    let scheme = |name: &str| {
        deckcraft_model::theme::builtin_themes().iter().any(|t| t.name == name)
            || deckcraft_model::theme::builtin_color_schemes().iter().any(|c| c.name == name)
    };
    listed(s)
        || s.rsplit_once(' ').is_some_and(|(name, n)| n.parse::<u32>().is_ok() && scheme(name))
        || words.iter().all(|w| UNITS.contains(w))
        || (screen == "dialog dateTime" && s.chars().any(|c| c.is_ascii_digit()) && words.iter().all(|w| DATE_WORDS.contains(w)))
}

/// The dialogs' ids, from `dialogs.rs`'s `fn title` (every dialog the app can open by id).
fn dialog_ids() -> Vec<String> {
    let source = std::fs::read_to_string(src().join("dialogs.rs")).unwrap();
    let body = source.split("fn title(id: &str)").nth(1).and_then(|b| b.split("\n}\n").next()).unwrap();
    body.lines().filter_map(|l| l.trim().strip_prefix('"')?.split_once("\" =>").map(|(id, _)| id.to_string())).collect()
}

/// Every screen in Korean draws no label that is English words only, except keep_english.txt's
/// and the document's own words: each ribbon tab (and Shape Format with a shape selected), every
/// File tab page, every dialog `app.dialog` opens, and the task panes (all four Format Shape tabs,
/// Format Background, Animation, Selection, Comments, Revisions). The net for what the source check
/// can't judge: text through a local variable, an array helper or a `match` (Rulings 4, 10).
/// Its limit: text drawn only with the painter (no accessibility node: the title bar's name, canvas
/// overlays) and screens it doesn't open (menus, pop-ups, the slide show) rely on the source check.
#[test]
fn korean_screens_show_no_untranslated_labels() {
    let mut s = deckcraft_engine::Session::with_new();
    deckcraft_engine::sample::open_sample(&mut s).unwrap();
    let app = crate::SlideApp::new(s, crate::Services::default());
    let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(1400.0, 900.0)).build_ui_state(
        |ui, app: &mut crate::SlideApp| {
            app.logic(ui.ctx());
            app.ui(ui);
        },
        app,
    );
    // Without the Korean font (a build without CRAFT_FONTS_DIR) Hangul draws as boxes; the labels
    // are still Korean.
    h.ctx.set_missing_glyph_policy(egui::MissingGlyphPolicy::Tofu);
    h.run_steps(3);
    let english = english_on_screen(&h);
    assert!(["Home", "Insert", "New Slide", "Slide 1"].iter().all(|s| english.contains(*s)), "the net sees English labels: {english:?}");
    h.state_mut().ui.language = "ko".into();
    h.run_steps(3);
    // What the rules let through, and what they don't.
    assert!(allowed_english(&h, "Design", "DeckCraft 2") && allowed_english(&h, "Home", "Slide"), "a theme's variant, the app's name");
    assert!(!allowed_english(&h, "Home", "Slide 3"), "a slide's name left in English is caught");
    assert!(allowed_english(&h, "dialog dateTime", "November 22, 2026") && !allowed_english(&h, "Home", "November 22, 2026"));
    let mut seen: BTreeMap<String, String> = BTreeMap::new();
    let mut look = |h: &mut egui_kittest::Harness<'_, crate::SlideApp>, screen: &str| {
        h.run_steps(3);
        // The Spelling dialog lists the document's misspelt words and the dictionary's
        // suggestions: the document's words, not wording.
        let spelling = h.state_mut().session.execute("review.spelling", &serde_json::json!({})).unwrap_or_default();
        let spelt: BTreeSet<String> = spelling
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|i| std::iter::once(&i["word"]).chain(i["suggestions"].as_array().into_iter().flatten()))
            .filter_map(|w| w.as_str().map(str::to_string))
            .collect();
        for s in english_on_screen(h).into_iter().filter(|s| !spelt.contains(s) && !allowed_english(h, screen, s)) {
            seen.entry(s).or_insert_with(|| screen.to_string());
        }
    };
    for tab in crate::ribbon::TABS {
        h.state_mut().ui.tab = tab.into();
        look(&mut h, tab);
    }
    let band = h.state().session.active().unwrap().doc.slides[0].shapes.iter().find(|s| s.name == "Aurora Band").unwrap().id.0;
    h.state_mut().run("edit.select", serde_json::json!({"ids": [band]})).unwrap();
    h.state_mut().ui.tab = "Shape Format".into();
    look(&mut h, "Shape Format");
    for tab in ["fill", "effects", "size", "text"] {
        h.state_mut().ui.pane = Some("format".into());
        h.state_mut().ui.format_tab = tab.into();
        look(&mut h, &format!("pane format/{tab}"));
    }
    for pane in ["background", "animation", "selection", "comments", "revisions"] {
        h.state_mut().ui.pane = Some(pane.into());
        look(&mut h, &format!("pane {pane}"));
    }
    h.state_mut().ui.pane = None;
    let ids = dialog_ids();
    for id in ["font", "paragraph", "find", "preferences"] {
        assert!(ids.iter().any(|d| d == id), "found the dialogs: {ids:?}");
    }
    for id in &ids {
        h.state_mut().run("app.dialog", serde_json::json!({"id": id})).unwrap();
        look(&mut h, &format!("dialog {id}"));
        h.state_mut().dialog = None;
    }
    for page in ["home", "new", "open", "info", "saveAs", "print", "export", "about"] {
        h.state_mut().run("app.backstage", serde_json::json!({"page": page})).unwrap();
        look(&mut h, &format!("backstage {page}"));
    }
    let listing: Vec<String> = seen.iter().map(|(s, screen)| format!("  {screen}: {s:?}")).collect();
    assert!(seen.is_empty(), "English on Korean screens (translate it, or add a keep_english.txt entry with its reason):\n{}", listing.join("\n"));
}

/// The command palette and the Quick Access Toolbar's list name every command: two different
/// English names never read as one Korean name there (Font and Fonts, Play and Playback).
#[test]
fn command_names_stay_distinct_in_korean() {
    set_current(resolve("ko"));
    let mut by_korean: BTreeMap<String, BTreeSet<&str>> = BTreeMap::new();
    let labels = deckcraft_engine::cmd::command_specs().iter().map(|c| c.label).chain(crate::UI_COMMANDS.iter().map(|c| c.1)).collect::<Vec<_>>();
    for label in labels {
        by_korean.entry(crate::menus::command_name(label).to_string()).or_default().insert(label);
    }
    let same: Vec<String> = by_korean.iter().filter(|(_, en)| en.len() > 1).map(|(ko, en)| format!("  {ko}: {en:?}")).collect();
    set_current(resolve("en"));
    assert!(same.is_empty(), "different commands with one Korean name (add a `command` row):\n{}", same.join("\n"));
    // Design's Fonts button says what the palette and the Quick Access Toolbar say.
    use egui_kittest::kittest::Queryable;
    let mut h = korean_harness();
    h.state_mut().ui.tab = "Design".into();
    h.run_steps(3);
    assert!(h.query_by_role_and_label(egui::accesskit::Role::Button, "테마 글꼴").is_some(), "Design › 테마 글꼴");
}

/// The built-in theme Paper reads 종이 (its `theme` row), not the printer's paper 용지 of the plain
/// row, which Print keeps; a new presentation from its tile keeps the English theme name.
#[test]
fn theme_names_are_translated_for_display_only() {
    use egui_kittest::kittest::Queryable;
    let mut h = korean_harness();
    assert_eq!((tc("theme", "Paper"), t("Paper")), ("종이", "용지"));
    assert_eq!(tc("theme", "Harbor"), t("Harbor"), "a theme without its own row reads as its plain row");
    h.state_mut().run("app.backstage", serde_json::json!({"page": "new"})).unwrap();
    h.run_steps(3);
    assert!(h.query_by_label("용지").is_none(), "the Paper theme isn't shown as the printer's paper");
    h.get_by_label("종이").click();
    h.run_steps(3);
    let d = h.state().session.active().unwrap();
    assert_eq!(d.doc.masters[0].theme.name, "Paper", "the document keeps the English name");
    h.state_mut().run("app.backstage", serde_json::json!({"page": "new"})).unwrap();
    h.run_steps(3);
    h.state_mut().backstage.as_mut().unwrap().query = "종이".into();
    h.run_steps(3);
    assert!(h.query_by_label("종이").is_some() && h.query_by_label(t("Harbor")).is_none(), "a Korean search finds it by its shown name");
}

/// A short template whose value is an id or a count ("no slide {id}", "{n} bytes", "Slide {n}:
/// {issue}") reads only the sentences it is: other sentences that happen to start or end like it
/// stay English whole, not half translated. A `{0}` that holds a whole reason ("not a PPTX file:
/// {0}") still takes one with spaces.
#[test]
fn short_templates_leave_other_sentences_alone() {
    set_current(resolve("ko"));
    for s in [
        "no slide to mark",
        "no slide with id 7",
        "no shape 12 on the current slide",
        "no slide 9 (the presentation has 8)",
        "every reference resolves to its own copied bytes",
        "Slide update helper: stopped",
    ] {
        assert_eq!(t_dynamic(s), s, "{s:?} stays English");
    }
    let reason = "not a PPTX file: the presentation has no slide master";
    assert_eq!(t_dynamic(reason), "PPTX 파일이 아닙니다: the presentation has no slide master");
    for (s, kept) in [("no slide 9", "9"), ("no shape 12", "12"), ("512 bytes", "512"), ("Slide 3: Missing alt text", "3")] {
        let shown = t_dynamic(s);
        assert!(shown != s && shown.contains(kept), "{s:?} is translated with its value: {shown:?}");
    }
    set_current(resolve("en"));
}

/// The engine's reasons a click can meet (Flip on a diagram, Group with one object …) read in
/// Korean after the "invalid parameters" template, and every such row is still the engine's
/// wording (a row whose sentence the engine no longer writes would never be used).
#[test]
fn engine_state_errors_are_korean() {
    let section = DECKCRAFT.split("# The engine's reasons a command didn't run").nth(1).unwrap();
    let section = section.split_once('\n').unwrap().1.split("# Messages with a value").next().unwrap();
    let mut engine = String::new();
    let mut files = Vec::new();
    source_files(&src().join("../../engine/src"), &mut files);
    for f in files {
        engine.push_str(&std::fs::read_to_string(f).unwrap());
    }
    let mut n = 0;
    for (_, _, en, ko) in rows(section) {
        assert!(engine.contains(&format!("\"{en}\"")), "the engine still writes {en:?}");
        // The app's words: Close Path 경로 닫기, Flip 뒤집기.
        assert!(!ko.contains("윤곽선") && !ko.contains("대칭 이동"), "{en:?} → {ko:?} says 경로 and 뒤집기 as the app does");
        n += 1;
    }
    assert!(n > 60, "read the section: {n} rows");
    let mut s = deckcraft_engine::Session::with_new();
    let id = s.execute("shape.insert", &serde_json::json!({"preset": "rect", "rect": [10, 10, 50, 50]})).unwrap()["id"].clone();
    s.execute("edit.select", &serde_json::json!({"ids": [id]})).unwrap();
    let err = s.execute("arrange.group", &serde_json::json!({})).err().unwrap().to_string();
    set_current(resolve("ko"));
    let shown = t_dynamic(&err).into_owned();
    set_current(resolve("en"));
    assert!(shown.contains("`arrange.group`") && !shown.contains("select") && has_ko(err.rsplit(": ").next().unwrap()), "{err:?} → {shown:?}");
}

/// keep_english.txt lets "Slide" through as the app's name; nothing else may say it. A label
/// meaning a slide written "Slide" would pass every check, so the only interface "Slide" in the
/// source is `APP_NAME`, and no command or menu is named "Slide".
#[test]
fn slide_stays_english_only_as_the_app_name() {
    assert!(audit::keep_english().iter().any(|k| k == crate::APP_NAME));
    let found: Vec<String> = audit::candidates(&src()).into_iter().filter(|c| c.text == "Slide").map(|c| format!("{}:{}", c.file, c.line)).collect();
    assert_eq!(found.len(), 1, "\"Slide\" is interface text only as APP_NAME: {found:?}");
    assert!(found[0].starts_with("lib.rs:"), "{found:?}");
    for c in deckcraft_engine::cmd::command_specs() {
        assert!(c.label != "Slide" && !c.menu.contains(&"Slide"), "{} is named or filed under \"Slide\"", c.id);
    }
}

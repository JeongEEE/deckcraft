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

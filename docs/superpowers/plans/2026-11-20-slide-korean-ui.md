# Slide 한국어 화면 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 한국어 환경의 Slide가 리본·백스테이지·메뉴·대화상자·작업창·상태 표시·엔진 메시지까지 모두 한국어로 보이게 하고, 빠진 문구를 테스트가 잡게 한다.

**Architecture:** `ui-egui`에 `i18n` 모듈을 둔다. craft-i18n crate(이미 lock에 있음)에 레포 안 TSV 두 개(`ko/common.tsv` 사본, 포크 `ko/deckcraft.tsv`)를 `include_str!`로 넣는다. 영어 원문이 키다. 공용 위젯은 안에서 번역하고, egui 직접 호출과 엔진 문구는 그리는 곳에서 `t()`/`fmt()`/`t_dynamic()`을 거친다. 누락 검사 테스트는 영역별로 켜고 마지막 Task에서 전체로 넓힌다.

**Tech Stack:** Rust, egui/eframe(원본 고정 rev), craft-i18n(craft-libs rev `63196e66f3033a6623eb629af95a104d26986725`), egui_kittest(기존 UI 테스트).

**Spec:** `docs/superpowers/specs/2026-11-20-slide-korean-ui-design.md`

## Global Constraints

- 번역 코드는 L7 `crates/ui-egui` 안에만 둔다. 엔진·모델 crate는 고치지 않는다(테스트가 엔진 소스를 읽기만 하는 것은 허용).
- 새 외부 의존성 금지. `craft-i18n = { git = "https://github.com/storytold/craft-libs", rev = "63196e66f3033a6623eb629af95a104d26986725" }`만 `crates/ui-egui/Cargo.toml`에 직접 의존으로 추가한다(이미 lock에 있는 rev).
- never-crash: 비테스트 코드에 unwrap/expect/panic/unreachable/todo/unimplemented 금지, `unsafe` 금지.
- 영어 화면은 지금과 글자 하나 다르지 않아야 한다. 기존 UI 테스트(영어 라벨로 버튼을 찾음)는 그대로 통과해야 한다.
- 파일에 저장되는 값(슬라이드 텍스트, 레이아웃·마스터·테마·개체 이름), 명령 ID, 저장 상태 키, egui ID는 번역하지 않는다. ID는 영어 원문으로 만든다.
- 번역 행의 `{name}` 자리표시자 집합, 끝 `…`, `\n` 개수는 영어와 같아야 한다.
- 영어로 두는 것: "DeckCraft Presentation", 테마 이름 "DeckCraft", 글꼴 이름, 단위 기호(pt, cm, px, %), 단축키 표기, 색 코드, 파일 확장자, About의 원본 저작권·라이선스 원문, CLI·MCP·제어 채널·로그.
- 번역 품질: 일반 컴퓨팅 용어로 영어 뜻을 보고 쓴다(clean room). 다른 제품의 번역 표를 베끼지 않는다. 같은 영어는 `ko/common.tsv`·이미 쓴 행과 같은 번역을 쓴다. 문체는 짧은 명사형 라벨, 문장은 "~합니다/~하세요"체로 통일한다.
- 검증: `cargo xtask ci` 8단계를 `CRAFT_FONTS_DIR=/private/tmp/claude-501/-Users-jeong-Dev-MyProjects/292f7c8e-7eaa-4db4-8925-af3e8077d201/scratchpad/craft-fonts-co`로 돌린다. 빌드 환경: `export PATH=$HOME/.cargo/bin:$PATH DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer`. 해당 디렉터리가 없으면 `git clone https://github.com/storytold/craft-fonts <dir> && git -C <dir> checkout 37827adc`.
- 커밋: 한국어 Conventional Commits, Co-Authored-By·AI 표식 금지, 바꾼 파일만 경로로 `git add`(`-A`·`.` 금지), 브랜치 `my-custom`. 커밋·푸시는 Task 리뷰 통과 뒤 컨트롤러가 지시한다.

## Review Focus

1. 한국어 Mac에서 `cargo test`를 돌릴 때 시스템 언어 때문에 기존 영어 라벨 테스트가 깨지는 경우 → 시스템 언어는 데스크톱 앱 `main.rs`만 알려 주고, 라이브러리 기본값은 English다(Task 1 테스트 `auto_is_english_without_system_language`).
2. 언어를 바꿨을 때 접히는 영역·작업창·리본 탭 상태가 풀리거나, 저장된 `ui.tab`·QAT 항목이 번역 문구로 저장되는 경우 → 탭 ID는 영어로 저장한다(Task 2 테스트 `tab_id_stays_english_in_korean`).
3. 레이아웃 이름처럼 화면에서 번역한 값이 명령 인자나 파일로 흘러 들어가는 경우 → 레이아웃 적용 뒤 저장 이름이 영어 그대로다(Task 7 테스트 `layout_names_are_translated_for_display_only`).
4. 엔진 오류 문장에 값이 끼워진 경우(파일 이름, 개수) 틀이 맞지 않으면 일부만 번역되거나 값이 사라지는 경우 → 맞는 틀이 없으면 영어 원문 전체를 그대로 보여 준다(Task 7 테스트 `dynamic_message_without_template_stays_english`, `dynamic_message_keeps_values`).
5. 한국어가 좁은 칸(작은 리본 버튼, 상태 표시줄, 대화상자 라벨 열)에서 잘리는 경우 → Task 8 실제 앱 스크린샷 점검과 짧은 번역/`tc` 문맥 행.

## 파일 구조

| 파일 | 역할 |
|---|---|
| `crates/ui-egui/i18n/ko/common.tsv` (새) | craft-i18n `locales/ko/common.tsv` 사본, 출처 `b5b2805795f3d98b8fef4484db75db0cefbdc630` |
| `crates/ui-egui/i18n/ko/deckcraft.tsv` (새) | 포크 Slide 한국어 목록. 영역별 `#` 절로 나눈다 |
| `crates/ui-egui/i18n/keep_english.txt` (새) | 영어로 두는 화면 문구 `<영어>\t<이유>` |
| `crates/ui-egui/src/i18n.rs` (새) | 번역 함수·언어 결정 |
| `crates/ui-egui/src/i18n/tests.rs` (새) | 목록 검사·누락 검사·표시 검사 |
| `crates/ui-egui/src/i18n/audit.rs` (새, `#[cfg(test)]`) | 소스에서 화면 문구 후보를 모으는 스캐너 |
| `crates/ui-egui/src/state.rs` | `UiState.language` |
| `crates/ui-egui/src/dialogs/preferences.rs` | 언어 드롭다운 |
| `apps/deckcraft/src/main.rs` | 시작할 때 시스템 언어를 `i18n`에 알려 줌 |
| 나머지 `crates/ui-egui/src/**` | 각 Task가 맡은 영역의 문구를 번역 함수로 감쌈 |

---

### Task 1: 번역 장치·설정·누락 검사 틀

**Files:**
- Create: `crates/ui-egui/i18n/ko/common.tsv`, `crates/ui-egui/i18n/ko/deckcraft.tsv`, `crates/ui-egui/i18n/keep_english.txt`, `crates/ui-egui/src/i18n.rs`, `crates/ui-egui/src/i18n/tests.rs`, `crates/ui-egui/src/i18n/audit.rs`
- Modify: `crates/ui-egui/Cargo.toml`, `crates/ui-egui/src/lib.rs`(mod 선언), `crates/ui-egui/src/state.rs`(UiState), `crates/ui-egui/src/app.rs:284`(`ui()` 시작에서 언어 지정), `crates/ui-egui/src/dialogs/preferences.rs`, `apps/deckcraft/src/main.rs`, `crates/ui-egui/src/widgets.rs`(공용 위젯), `ATTRIBUTION.md`, `Cargo.lock`

**Interfaces:**
- Produces (`crate::i18n`, 이후 모든 Task가 씀):
  - `pub const AUTO: &str = "auto";`
  - `pub fn set_system_lang(tags: &[String])`: 데스크톱 앱이 시작 시 한 번 부른다. 부르지 않으면 `auto`는 English.
  - `pub fn resolve(pref: &str) -> craft_i18n::Lang`: `"ko"` → 한국어, `"en"` → English, `"auto"`/그 밖 → 시스템 언어(한국어 계열 태그면 한국어, 아니면 English).
  - `pub fn set_current(lang: craft_i18n::Lang)`, `pub fn current() -> craft_i18n::Lang` (스레드별, 기본 English)
  - `pub fn is_korean() -> bool`
  - `pub fn t(s: &str) -> &str`, `pub fn tc<'a>(context: &str, s: &'a str) -> &'a str`, `pub fn tn(n: u64, one: &str, other: &str) -> String`
  - `pub use craft_i18n::fmt;` (`fmt(template, &[("name", value)]) -> String`)
  - `pub fn has_ko(s: &str) -> bool` (테스트·감사용)
  - `UiState.language: String` (serde 기본값 `"auto"`, `sanitized`가 `auto|ko|en` 밖의 값을 `auto`로)
  - 테스트용(`#[cfg(test)]`): `audit::candidates(dir: &Path) -> Vec<Candidate>`, `Candidate { file: String, line: usize, text: String }`, `audit::missing(c: &[Candidate]) -> Vec<Candidate>` (ko 번역도 keep_english 항목도 없는 후보)

- [ ] **Step 1: 목록 받기.** `gh api "repos/storytold/craft-i18n/contents/locales/ko/common.tsv?ref=b5b2805795f3d98b8fef4484db75db0cefbdc630" -H "Accept: application/vnd.github.raw" > crates/ui-egui/i18n/ko/common.tsv`. 파일 맨 위 주석에 출처 저장소·커밋·라이선스(Apache-2.0)를 덧붙이고 `ATTRIBUTION.md`에 행을 추가한다. `deckcraft.tsv`는 craft-i18n과 같은 머리 주석(열 설명 + clean room 문장)에 "Slide 포크 한국어 목록" 한 줄을 더해 시작한다.

- [ ] **Step 2: 실패하는 테스트 작성** (`i18n/tests.rs`)

```rust
#[test] fn auto_is_english_without_system_language() { assert_eq!(resolve(AUTO), craft_i18n::Lang::EN); }
#[test] fn korean_pref_translates_common_rows() { set_current(resolve("ko")); assert_eq!(t("Cancel"), "취소"); set_current(resolve("en")); assert_eq!(t("Cancel"), "Cancel"); }
#[test] fn unknown_string_falls_back_to_english() { set_current(resolve("ko")); assert_eq!(t("zz-no-such-key"), "zz-no-such-key"); }
#[test] fn catalog_rows_keep_placeholders_ellipsis_and_newlines() { /* deckcraft.tsv·common.tsv 모든 행: {..} 집합·끝 …·\n 수가 같다. I18N.check_all()이 비어 있다 */ }
#[test] fn no_duplicate_keys_in_fork_catalog() { /* (context, 영어) 쌍 중복 없음 */ }
#[test] fn keep_english_entries_are_still_in_source() { /* keep_english.txt 각 항목이 audit::candidates(src)에 있다 */ }
#[test] fn ui_state_language_defaults_to_auto_and_sanitizes() { /* 기본 "auto", "xx" → "auto" */ }
#[test] fn korean_coverage_widgets() { /* audit::missing(candidates(widgets.rs, ribbon.rs의 TABS·tab_label)).is_empty() */ }
```
`common.tsv`에 "Cancel"이 없거나 번역이 다르면 그 파일에 실제로 있는 행으로 바꾼다.

- [ ] **Step 3: 실패 확인.** `cargo test -p deckcraft-ui-egui i18n::` → 컴파일 실패 또는 FAIL.

- [ ] **Step 4: 구현.**
  - `i18n.rs`: `static I18N: LazyLock<I18n> = LazyLock::new(|| I18n::new(&[("ko", COMMON, DECKCRAFT)], &["ko"]).0)`. `new`가 돌려준 문제 목록은 `log::warn!`으로 남긴다. 시스템 언어는 `OnceLock<Lang>`에 저장한다(`I18N.match_tags`).
  - `audit.rs`: 후보 규칙 1차안. 비테스트 `.rs`(파일명 `_tests.rs`, `tests.rs`, `#[cfg(test)]` 모듈 아래 줄 제외)에서, 영문자로 시작하고 공백이나 대문자 단어를 포함하는 문자열 리터럴 중 화면 출력 함수 인자(공용 위젯, `label`, `button`, `on_hover_text`, `selectable_label`, `checkbox`, `RichText::new`, `heading`, `menu_button`, `Window::new`, `t(`, `tc(`, `fmt(`, `toast` 등)나 `const`/배열의 라벨로 쓰이는 것. 제외: `log::`·`format!` 디버그·`id_salt`/`Id::new` 인자·serde 이름·명령 ID 형태(`a.b`)·경로·확장자. 규칙과 측정한 후보 수를 `audit.rs` 문서 주석에 적는다. 거짓 양성은 규칙으로 줄이고, 남은 영어 유지 항목만 `keep_english.txt`에 넣는다.
  - `widgets.rs`: 라벨·툴팁을 받는 공용 함수(`big_button`, `big_button_ex`, `small_button*`, `check_item`, `icon_toggle`, `color_split`, `dropdown`(label과 current), `gallery`, `spinner`, `section`, `drop_button` 등)는 그리기 직전에 `t()`. ID를 만드는 데 쓰는 문자열은 원문 그대로 둔다. 리본 탭 이름(`ribbon.rs` `TABS`, `tab_label`)은 표시할 때만 `t()`. `ko/deckcraft.tsv`에 이 범위의 번역 행을 넣는다.
  - `state.rs`: `language` 필드 + `sanitized`.
  - `app.rs` `ui()` 시작: `i18n::set_current(i18n::resolve(&self.ui.language))`. 같은 스레드에서 그리는 다른 진입점(발표자 보기·별도 문서 창 등)이 `SlideApp::ui`를 거치지 않으면 거기에도 넣는다.
  - `preferences.rs`: "Language" 드롭다운(`Automatic (system)` / `한국어` / `English`). "한국어"·"English"는 언어 이름이라 어느 언어에서나 그대로 둔다(keep_english 대상 아님, 번역 행에서 같은 값). 바꾸면 다음 프레임부터 적용된다.
  - `main.rs`: eframe 시작 전 `deckcraft_ui_egui::i18n::set_system_lang(&sys_locale::get_locales().collect::<Vec<_>>())`. sys-locale이 apps/deckcraft의 직접 의존이 아니면 `craft_i18n`이 제공하는 시스템 언어 API(`I18n::system_lang`)를 감싼 `i18n::detect_system_lang()`를 대신 노출해 main이 그것만 부르게 한다.

- [ ] **Step 5: 통과 확인.** `cargo test -p deckcraft-ui-egui i18n::` PASS, `cargo test -p deckcraft-ui-egui` 전체 PASS(영어 라벨 테스트 포함), `cargo xtask ci` 8단계 PASS.

- [ ] **Step 6: 보고.** 측정한 후보 수(영역별: ribbon·dialogs·backstage·panels·canvas·show·sorter·app·최상위 파일), 확정한 후보 규칙, keep_english 항목 수를 보고서에 적는다. 다음 Task들이 이 숫자로 범위를 잡는다.

### Task 2: 리본 1 — 홈·삽입·그리기·디자인·전환·애니메이션·QAT·제목줄

**Files:** Modify `crates/ui-egui/src/ribbon/{home,insert,draw,design,transitions,animations,galleries,qat,title_bar}.rs`, `crates/ui-egui/src/ribbon.rs`(위 탭이 쓰는 부분), `crates/ui-egui/i18n/ko/deckcraft.tsv`, `crates/ui-egui/src/i18n/tests.rs`

**Interfaces:** Consumes Task 1 `t`/`tc`/`tn`/`fmt`/`audit`. Produces 없음.

- [ ] **Step 1: 테스트 추가.** `korean_coverage_ribbon_1`(위 파일들 `audit::missing` 비어 있음). `tab_id_stays_english_in_korean`: 한국어로 지정한 하네스에서 리본의 "삽입" 탭을 눌러도 `app.ui.tab == "Insert"`. QAT에 항목을 넣어도 저장 값은 명령 ID다.
- [ ] **Step 2: 실패 확인.** `cargo test -p deckcraft-ui-egui korean_coverage_ribbon_1` FAIL(누락 목록 출력).
- [ ] **Step 3: 구현.** egui 직접 호출·`format!` 문장을 `t()`/`fmt(t("…{n}…"), …)`로 감싸고 번역 행을 `deckcraft.tsv`의 `# Ribbon 1` 절에 넣는다. 갤러리 이름(테마 효과·전환·애니메이션 효과 이름 등) 중 파일에 저장되지 않는 표시용 이름은 번역하고, 저장되는 이름은 표시할 때만 번역한다.
- [ ] **Step 4: 통과 확인.** 위 두 테스트 PASS, `cargo test -p deckcraft-ui-egui` 전체 PASS.

### Task 3: 리본 2 — 슬라이드 쇼·녹화·검토·보기·상황별 탭

**Files:** Modify `crates/ui-egui/src/ribbon/{slide_show,review,view,shape_format,picture_format,picture_styles 관련,table,chart,smartart,masters,playback,arrange}.rs`와 `ribbon.rs`의 나머지, `deckcraft.tsv`, `i18n/tests.rs`

- [ ] **Step 1:** 테스트 `korean_coverage_ribbon_2`(위 파일들 + `ribbon.rs` 전체). 이 Task가 끝나면 `crates/ui-egui/src/ribbon/`·`ribbon.rs` 전체 후보가 0이어야 한다.
- [ ] **Step 2:** FAIL 확인.
- [ ] **Step 3:** Task 2와 같은 방식. `deckcraft.tsv` `# Ribbon 2` 절.
- [ ] **Step 4:** PASS 확인, 전체 테스트 PASS.

### Task 4: 엔진 명령 이름·메뉴 경로·비활성 사유·메뉴·명령 팔레트

**Files:** Modify `crates/ui-egui/src/menus.rs`, 명령 팔레트·검색·키팁·툴팁에서 `CommandInfo`/`CommandSpec`의 `label`·`menu`·`disabled_reason`을 그리는 모든 곳(`rg "\.label\b|disabled_reason|\.menu\b" crates/ui-egui/src`), `deckcraft.tsv`, `i18n/tests.rs`

- [ ] **Step 1: 테스트.**
  - `korean_coverage_command_labels`: `deckcraft_engine::cmd::command_specs()`의 모든 `label`과 `menu` 세그먼트가 `has_ko` 또는 keep_english.
  - `korean_coverage_disabled_reasons`: `crates/engine/src/cmd.rs`·`crates/engine/src/cmd/*.rs`에서 `enabled` 계열 함수가 돌려주는 `Err("…")`/`Err(format!("…"))`/`.into()` 문자열을 소스 스캔으로 모아 `has_ko`(값이 끼워진 것은 `{}`를 `{0}`, `{1}` 자리표시자로 바꾼 틀 기준).
  - `korean_coverage_menus`: `menus.rs` 등 이 Task가 맡은 UI 파일의 `audit::missing`이 비어 있다.
  - `command_search_matches_korean_and_english`: 한국어일 때 명령 팔레트에서 한국어 이름과 영어 이름 둘 다로 같은 명령을 찾는다(검색어를 번역 라벨과 원문 라벨에 모두 대조).
- [ ] **Step 2:** FAIL 확인.
- [ ] **Step 3: 구현.** 그리는 곳에서 `t(label)`, 메뉴 경로는 세그먼트마다 `t()`로 번역한 뒤 ` › `로 잇는다. 비활성 사유는 Task 7의 `t_dynamic`이 아직 없으므로 고정 문구는 `t()`, 값이 끼워진 것은 행만 추가하고 표시는 Task 7에서 연결한다. 그 경우 보고서에 해당 위치를 남긴다.
- [ ] **Step 4:** PASS 확인.

### Task 5: 대화상자 1 — about … language

**Files:** Modify `crates/ui-egui/src/dialogs.rs`(제목 매핑 포함), `crates/ui-egui/src/dialogs/{about,alt_text,body,columns,credits,date_time,equation,export,find,font,grid_options,hyperlink,language}.rs`, `deckcraft.tsv`, `i18n/tests.rs`

- [ ] **Step 1:** `korean_coverage_dialogs_1`.
- [ ] **Step 2:** FAIL 확인.
- [ ] **Step 3:** 감싸고 번역한다. About의 원본 저작권·라이선스 원문은 keep_english(이유: 법적 고지 원문). 포크 문구("DeckCraft 기반" 등)는 번역한다. 언어 대화상자의 언어 이름 목록은 각 언어의 자기 이름이면 그대로 둔다.
- [ ] **Step 4:** PASS 확인.

### Task 6: 대화상자 2 — paragraph … zoom, 업데이트 화면

**Files:** Modify `crates/ui-egui/src/dialogs/{paragraph,photo_album,preferences,size_position,slide_size,spelling,tabs,theme_colors,theme_fonts,trim,zoom}.rs`, `crates/ui-egui/src/dialogs/updates_ui.rs`·`updates_ui/*.rs`, `deckcraft.tsv`, `i18n/tests.rs`

- [ ] **Step 1:** `korean_coverage_dialogs_2`. 이 Task가 끝나면 `dialogs/`·`dialogs.rs` 전체 후보가 0. 포크 업데이트 문구(카드·모달·Restart to Update / Later·오류)도 포함한다. `tests/fork_identity.rs`가 영어 문구를 검사한다면 영어 그대로 통과해야 한다.
- [ ] **Step 2:** FAIL 확인.
- [ ] **Step 3:** 구현.
- [ ] **Step 4:** PASS 확인, `cargo test -p deckcraft-ui-egui --test fork_identity` PASS.

### Task 7: 작업창·캔버스·쇼·정렬·백스테이지·최상위 파일·엔진 메시지

**Files:** Modify `crates/ui-egui/src/{panels,canvas,show,sorter,backstage,app}/**`, `crates/ui-egui/src/{panels,canvas,show,sorter,backstage,presenter,status,toast,media,opening,doc_window,eyedropper,…}.rs`(ribbon·dialogs·menus 밖 최상위 파일 전부), `crates/ui-egui/src/i18n.rs`, `deckcraft.tsv`, `i18n/tests.rs`

**Interfaces:**
- Produces: `pub fn t_dynamic(s: &str) -> std::borrow::Cow<'_, str>` in `crate::i18n`. 먼저 `t(s)`가 번역하면 그것을 쓴다. 아니면 `deckcraft.tsv`에서 `{…}`가 있는 행을 틀로 읽어(처음 쓸 때 한 번 파싱해 캐시) 고정 조각이 순서대로 맞는 첫 틀을 찾고, 잡힌 값을 번역 틀에 채운다. 맞는 틀이 없으면 원문 그대로.

- [ ] **Step 1: 테스트.**
  - `korean_coverage_panels`, `korean_coverage_canvas_show_sorter`, `korean_coverage_backstage`, `korean_coverage_top_level`(최상위 `.rs`와 `app/`).
  - `dynamic_message_keeps_values`: 한국어일 때 TSV에 넣은 틀 하나(예: 엔진의 실제 오류 문장 틀)로 `t_dynamic("<값이 든 영어 문장>")`이 값을 보존한 한국어가 된다.
  - `dynamic_message_without_template_stays_english`: 맞는 틀이 없는 문장은 글자 하나 다르지 않다.
  - `layout_names_are_translated_for_display_only`: 한국어일 때 레이아웃 갤러리에 "제목 슬라이드"류 번역이 보이고, 그 레이아웃으로 새 슬라이드를 만든 뒤 문서의 레이아웃 이름은 "Title Slide" 그대로다.
  - `placeholder_prompts_are_korean`: 한국어일 때 빈 제목 개체 틀 안내가 번역 문구로 그려지고, 문서 텍스트는 비어 있다.
- [ ] **Step 2:** FAIL 확인.
- [ ] **Step 3: 구현.** 남은 UI 문구를 감싼다. 엔진 오류·상태 문장이 화면에 나오는 곳(토스트, 상태 표시줄, 오류 대화상자, Task 4가 남긴 비활성 사유 위치)은 `t_dynamic`을 거친다. 화면에 자주 나오는 엔진 문장(열기·저장·내보내기 실패, 선택 없음, 붙여넣기 실패 등)은 `rg 'Err\(|EngineError::Other\(|format!\(' crates/engine/src`로 찾아 틀 행을 넣는다. 빈 개체 틀 안내·레이아웃·마스터 이름은 표시 지점에서만 번역한다.
- [ ] **Step 4:** PASS 확인, 전체 테스트·`cargo xtask ci` PASS.

### Task 8: 전체 누락 검사·화면 점검·문서

**Files:** Modify `crates/ui-egui/src/i18n/tests.rs`, `deckcraft.tsv`(점검에서 나온 보정), `docs/custom-release.md`(한국어 화면 절), `AGENTS.md`(Slide 포크 규칙에 "새 화면 문구는 `t()`를 거치고 `ko/deckcraft.tsv`에 넣는다, `korean_coverage_all`이 검사" 한 줄)

- [ ] **Step 1: 테스트.**
  - `korean_coverage_all`: `crates/ui-egui/src` 전체 `audit::missing`이 비어 있다. 영역별 테스트는 남겨 둔다(실패 위치를 좁혀 줌).
  - `korean_screens_show_no_untranslated_labels`: 한국어로 지정한 egui_kittest 하네스에서 리본 10개 탭 각각, 백스테이지(새로 만들기·열기·정보·인쇄·내보내기), Preferences, 글꼴·단락·찾기 대화상자를 그린 뒤, 접근성 트리의 라벨 중 영어 단어만으로 된 것이 keep_english 밖에 없다.
- [ ] **Step 2:** FAIL이면 빠진 곳을 메운다.
- [ ] **Step 3: 실제 앱 점검.** `cargo run --release -p deckcraft -- --sample --control 7990`를 한국어 설정으로 띄운다. 기존 설정을 건드리지 않도록 `HOME=<scratchpad>/home`으로 실행하고, 그 아래 `Library/Application Support/Slide/ui.json`에 `{"language":"ko"}`를 미리 둔다. 앱이 그 파일을 읽지 않으면 Preferences에서 바꾼다. 제어 채널로 리본 탭 10개, 백스테이지, 대화상자 5개 이상, 슬라이드 쇼, 발표자 보기를 `ui.screenshot`으로 찍어 확인한다. 잘림·겹침·네모 글자·영어 잔존을 고친다. 스크린샷은 scratchpad에만 둔다.
- [ ] **Step 4: 용어 일관성.** `deckcraft.tsv`에서 같은 영어 단어의 번역이 행마다 다르지 않은지(Slide/슬라이드, Shape/도형, Placeholder/개체 틀, Transition/전환, Animation/애니메이션, Theme/테마, Layout/레이아웃, Master/마스터, Format/서식, Insert/삽입) 확인한다.
- [ ] **Step 5:** `cargo xtask ci` 8단계 PASS, `python3 packaging/test_fork_contract.py`, `python3 packaging/test_deploy.py` PASS.

릴리스(0.19.1 Mac 재빌드·초안 자산 교체·DMG 확인·Windows 빌드·finalize)는 코드 Task가 아니라 최종 리뷰 뒤 컨트롤러가 진행한다.

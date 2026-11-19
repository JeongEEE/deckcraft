# Slide 포크 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** deckcraft를 원본과 나란히 설치되는 개인 앱 **Slide**로 만들고, 서명 자동 업데이트·macOS 제목줄 이동·한국어 설치 화면·로컬 배포 스크립트를 갖춰 0.19.1을 공개한다.

**Architecture:** Word 포크(`/Users/jeong/Dev/MyProjects/wordcraft` `my-custom`)의 최종 상태를 기준으로 파일을 옮기고 값만 바꾼다. 제목줄 이동만 Grid 포크(`/Users/jeong/Dev/MyProjects/gridcraft` `my-custom`)에서 옮긴다. 참조 diff: `git -C <참조 레포> diff main...my-custom -- <경로>`. cherry-pick 금지.

**Tech Stack:** Rust 1.95 workspace, egui/eframe(원본이 고정한 git 커밋), craft-update(원본 고정 rev `63196e66…` — **올리지 않는다**), WiX 5.0.2, Python 3 배포 스크립트, bash/PowerShell 7.

**Spec:** `docs/superpowers/specs/2026-11-19-slide-fork-design.md`

## Global Constraints

- 앱 이름 `Slide`, 유지관리자 `JeongEEE`, updater id·파일 stem `slide`, 저장소 `JeongEEE/deckcraft`, 태그 `deckcraft-v<버전>`.
- bundle id `io.github.jeongeee.slide`, 설정 `~/Library/Application Support/Slide`·`%APPDATA%\Slide`·Linux `~/.config/slide`.
- Windows: 설치 `%LOCALAPPDATA%\Programs\JeongEEE\Slide`, registry `Software\JeongEEE\Slide`, ProgID `JeongEEE.Slide.*`, 시작 메뉴 `Slide (JeongEEE)`, 파일 연결은 Open With 후보만.
- 업데이트 목록 `SlideUpdate.json`·`SlideUpdate.sig`, 서명 도구 `slide-update-sign`, 개인키 `~/Dev/keystores/slide/update-private-key.hex`, 환경변수 `SLIDE_UPDATE_KEY_FILE`.
- Cargo 패키지·crate 이름(`deckcraft-*`)·명령 ID·`DECKCRAFT_*` 환경변수는 유지. 파일 형식 이름("DeckCraft Presentation (*.deckcraft)")·내장 테마 이름 "DeckCraft"는 앱 이름이 아니므로 유지.
- 새 crate는 `deckcraft-update`(경로 `crates/update`). 레이어 표상 L7 옆(ui-egui·apps만 의존), `xtask` layers 검사 통과.
- Never crash: 비테스트 코드에 `unwrap/expect/panic!/unreachable!/todo!/unimplemented!`·`unsafe` 금지, crate 루트 deny 속성 복사.
- 화면 문구는 영어(한국어 화면은 범위 밖). 설치 화면(MSI·DMG)만 한국어.
- 개인키를 읽거나 만들지 않는다. 테스트는 결정적 테스트 키만.
- 빌드 환경: `export PATH=$HOME/.cargo/bin:$PATH DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer`. cargo 출력은 파이프로 종료코드를 삼키지 말 것(`set -o pipefail` 또는 파일로 받기). 실행 전 바이너리 mtime 확인.
- cwd는 레포 루트 고정, `crates/` 아래로 cd 금지(OMC가 `crates/.omc`를 만들어 workspace 글로브가 깨짐). 생기면 scratchpad로 `mv`.
- 파일 삭제는 `git rm`(작업 트리 `rm` 금지 규칙 때문). 임시 파일은 scratchpad.
- 커밋: Task마다 리뷰 통과 후 그 Task 경로만 `git add <경로>`, 한국어 Conventional Commit, AI 표식·Co-Authored-By 금지. `my-custom`에 push.

## Review Focus

1. **원본 DeckCraft가 이미 설치된 PC**: Slide 설치·업데이트가 원본 앱 번들·설정 폴더·파일 연결을 건드리지 않아야 한다 → Task 2 `test_fork_contract.py`가 식별자가 원본과 다름을 검사, Task 4 fork_identity가 업데이트 폴더 분리를 검사.
2. **공개키가 비어 있는 빌드**(Task 8 전): 업데이트 확인은 돼도 설치는 거부(fail closed) → Task 3 `empty_or_malformed_pinned_key_is_an_error_not_a_trust_root`.
3. **오프라인·GitHub 장애 중 시작**: 요청하지 않은 확인 실패에 창이 뜨면 안 됨 → Task 4 `unrequested_check_failure_shows_no_window` 테스트.
4. **제목줄을 천천히·빠르게 끌기, 놓는 순간 이동**: 창이 포인터를 따라가고 누적 오차가 없어야 함 → Task 5 `title_bar_drag` 모델 테스트 + 실측.
5. **업데이트 중 저장 안 한 프레젠테이션**: 저장 확인에서 취소하면 종료·설치 예약도 취소 → Task 4 `cancelled_save_prompt_cancels_update_restart`.

---

### Task 1: 앱 이름·설정 경로·원본 진입점 정리

**Files:**
- Modify: `apps/deckcraft/src/main.rs`(config_dir, `.with_title`, `.with_app_id`, `run_native` 이름, 로그 문구), `apps/deckcraft/src/logging.rs`(시작 로그 문구만)
- Modify: `crates/ui-egui/src/lib.rs:140`("About DeckCraft"→"About Slide"), `menus.rs:310`, `dialogs.rs:51,73,76`, `ribbon/title_bar.rs:58`, `show/presenter.rs:151`, `backstage/print.rs:87,89`, `dialogs/find.rs:52,67`, `app/files.rs:203` — 앱을 가리키는 "DeckCraft"만 `crate::APP_NAME`으로
- Modify: `crates/ui-egui/src/dialogs/about.rs`(제목 Slide, "Based on DeckCraft" 출처 문구, Discord·getartcraft 링크 제거, 저작권 줄 유지), `backstage/files.rs:213,255`, `backstage/info.rs:221,229`, `app/commands.rs:286-287`(UI 전용 링크 명령이면 Discord 키 제거 — 명령 자체가 MCP·CLI 계약이면 유지하고 사유 주석)
- Modify: 해당 테스트(`backstage/tests.rs:838` 등 Discord·"About DeckCraft" 기대값)
- Modify: `.gitignore`(`.omc/` 추가), `AGENTS.md` 맨 위 "Slide 포크 규칙" 절(Word `AGENTS.md` 상단 절을 Slide 값으로), `README.md`(Word처럼 포크 안내로 축약, 원본 링크·라이선스 유지), `NOTICE`·`ATTRIBUTION.md`(Word diff 참고, 로고 사용 행 정리)
- Delete: 배포물에 ArtCraft 로고가 들어가는 경로가 있으면 그 사용만 제거(`docs/brand/` 파일 삭제는 Word처럼 하되 `cargo xtask assets` 통과 확인)

**Interfaces:**
- Produces: `crates/ui-egui/src/lib.rs`에 `pub const APP_NAME: &str = "Slide";` — 이후 Task와 테스트가 사용.
- Produces: `apps/deckcraft/src/main.rs` `config_dir()` → `…/Slide`(mac·win), `~/.config/slide`(그 외 XDG 경로 규칙은 원본 구현 유지하고 폴더명만).

- [ ] **Step 1:** `crates/ui-egui/tests/fork_identity.rs` 새로 만들고 `fork_app_name_is_slide`: `assert_eq!(deckcraft_ui_egui::APP_NAME, "Slide")`. 기존 UI 테스트 중 About 대화상자 제목이 `"About Slide"`인지 보는 테스트 1개 추가(기존 dialogs 테스트 하네스 방식 따름).
- [ ] **Step 2:** `cargo test -p deckcraft-ui-egui --test fork_identity` → 컴파일 실패(APP_NAME 없음) 확인.
- [ ] **Step 3:** 위 파일들 수정. 설정 폴더가 원본과 갈리는지 확인하는 단위 테스트를 `main.rs` 기존 테스트 모듈(없으면 `#[cfg(test)] mod`)에 `config_dir_is_slides_own`으로 추가(HOME/APPDATA 환경변수를 바꾸는 대신 경로 끝 비교 함수로 분리해 테스트).
- [ ] **Step 4:** `cargo test -p deckcraft-ui-egui` `cargo test -p deckcraft` 실행. 실패는 원본 `main`에서도 나는지 갈라 기록(글꼴 없는 환경 실패 가능). `cargo clippy -p deckcraft-ui-egui -p deckcraft --no-deps -- -D warnings`, `cargo fmt --check`, `cargo xtask assets`.
- [ ] **Step 5:** 커밋 `feat: Slide 개인 포크 앱 이름·설정 경로 분리와 Discord·ArtCraft 진입점 제거`.

### Task 2: 설치 패키지 식별자 (macOS·Windows)

**Files:**
- Modify: `packaging/macos/Info.plist.in`, `packaging/macos/package.sh`(앱 `Slide.app`, 실행 파일 `Slide`/CLI `slide-cli`, DMG `slide-<v>-macos-<arch>.dmg`, CLI zip `slide-cli-<v>-macos-<arch>.zip`, 볼륨 이름 `Slide` 버전 없이, 서명 메타데이터 보존·마운트 후 codesign 검사는 Word `package.sh` diff를 따름)
- Modify: `packaging/windows/deckcraft.wxs`, `install-policy.json`, `install-policy.wxi`, `package.ps1`, `sign.ps1`, `check-install-policy.ps1`, `test-install-policy.ps1`, `INSTALLATION.md` — Word의 같은 파일 diff를 기준으로 Slide 값. UpgradeCode·component GUID는 `uuidgen`으로 새로 만들고 대문자로 고정.
- Modify: `apps/deckcraft/build.rs`(Windows 리소스의 제품명·파일 설명이 있으면 Slide)
- Create: `packaging/test_fork_contract.py`(Word의 것을 Slide 값으로)

**Interfaces:**
- Produces: 자산 이름 규칙 `slide-<v>-macos-universal.dmg`, `slide-cli-<v>-macos-universal.zip`, `slide-<v>-windows-x64.msi`, `slide-<v>-windows-x64-portable.zip` — Task 6·7이 사용.
- Produces: MSI 실행 파일 `slide.exe`·`slide-cli.exe`, macOS 번들 `Slide.app/Contents/MacOS/Slide`.

- [ ] **Step 1:** `packaging/test_fork_contract.py` 작성: plist의 `CFBundleIdentifier == io.github.jeongeee.slide`·`CFBundleName == Slide`, wxs의 UpgradeCode가 원본(`git show main:packaging/windows/deckcraft.wxs`)과 다름, ProgID가 `JeongEEE.Slide.`로 시작, registry `Software\JeongEEE\Slide`, 시작 메뉴 `Slide (JeongEEE)`, 확장자 기본 앱을 바꾸는 항목 없음, install-policy.json 파일 목록이 `slide.exe`/`slide-cli.exe`.
- [ ] **Step 2:** `python3 packaging/test_fork_contract.py` → FAIL 확인.
- [ ] **Step 3:** 파일 수정.
- [ ] **Step 4:** `python3 packaging/test_fork_contract.py` PASS. `pwsh`가 있으면 `pwsh packaging/windows/test-install-policy.ps1`(없으면 없다고 기록). `bash -n packaging/macos/package.sh`.
- [ ] **Step 5:** `packaging/macos/package.sh --arch aarch64`(서명 시크릿 없으면 ad-hoc) 로 DMG 생성, `hdiutil attach -readonly`로 `Slide.app` plist·`codesign --verify --deep --strict` 확인 후 detach. 산출물은 커밋하지 않음.
- [ ] **Step 6:** 커밋 `feat: Slide 설치 식별자 분리(macOS 번들·Windows MSI)`.

### Task 3: 포크 업데이트 crate `deckcraft-update`

**Files:**
- Create: `crates/update/` 전체 — Word `crates/update`(Cargo.toml, `public-key.hex`, `src/{lib,manifest,download,helper,install,target,macos,windows}.rs`, `src/bin/word-update-sign.rs`→`slide-update-sign.rs`, `tests/{install,signing,trust}.rs`)를 옮겨 값 교체: 앱 id `slide`·이름 `Slide`·설명 "Presentations and slide design", 저장소·태그 접두사, `SlideUpdate.*`, bundle id, 설치 경로, helper 인자 `--slide-update-helper`, 환경변수 `SLIDE_UPDATE_*`, 실행 파일 이름(Task 2).
- Modify: `Cargo.toml`(workspace 의존 `deckcraft-update = { path = "crates/update" }`, 새 의존 버전은 Word와 동일하게 고정: `ring =0.17.14`, `zip 9`, `tempfile 3`, `nix =0.31.3`, `winreg =0.55.0`), `Cargo.lock`, `xtask/src/layers.rs`(Word diff처럼 등록)
- `public-key.hex`는 **빈 파일**로 둔다(Task 8에서 채움). `pinned_release_key_matches_owner_key` 테스트는 Task 8에서 추가.

**Interfaces:**
- Produces: `deckcraft_update::APP: craft_update::…::App`(deckcraft가 고정한 craft-update rev의 실제 타입 경로를 열어 확인 — Word는 `craft_update::App`, deckcraft의 lib.rs는 `craft_update::app::App`), `deckcraft_update::public_key() -> Result<Vec<u8>, String>`, `deckcraft_update::helper::{finish, acknowledge_startup}`, `manifest::REPOSITORY = "JeongEEE/deckcraft"`. 나머지 공개 API는 Word와 같은 이름.
- craft-update API 차이가 있으면 **deckcraft 쪽 rev에 맞춰 update crate를 고친다**(craft-libs rev를 올리지 않는다).

- [ ] **Step 1:** Word 테스트(`tests/*.rs`, lib 내부 테스트)를 먼저 옮기고 값 교체 → `cargo test -p deckcraft-update` 컴파일 실패 확인.
- [ ] **Step 2:** 소스 옮기기·값 교체.
- [ ] **Step 3:** `cargo test -p deckcraft-update` 전부 PASS, `cargo clippy -p deckcraft-update --all-targets --no-deps -- -D warnings`, `cargo xtask layers`, `cargo xtask wasm`(update crate가 wasm에 안 들어가는지).
- [ ] **Step 4:** `grep -rn -i 'word\|wordcraft' crates/update` 결과 0건(의도된 비교 문자열 제외).
- [ ] **Step 5:** 커밋 `feat: Slide 서명 업데이트 검증·설치 crate 추가`.

### Task 4: 업데이트 UI를 포크 경로로 연결

**Files:**
- Replace: `crates/ui-egui/src/dialogs/updates_ui.rs`(craft-update 원본 경로) → Word `crates/ui-egui/src/updates_ui.rs` + `updates_ui/view.rs` + `updates_ui/tests.rs` 구조로. 모듈 위치는 deckcraft의 기존 `dialogs/updates_ui` 자리를 유지해도 되고 옮겨도 된다(빌더 판단, `lib.rs:66` use 경로 맞춤).
- Modify: `crates/ui-egui/src/lib.rs:12-15`(`UPDATE_APP`이 `..deckcraft_update::APP`), `app.rs:88,138,146,234,363`, `app/commands.rs:231`(Help ▸ Check for Updates), 종료 흐름(저장 확인 후 재시작 — deckcraft의 quit/close 처리 위치를 찾아 Word `app/quit.rs`·`files.rs` diff와 같은 의미로 연결), `apps/deckcraft/src/main.rs:209-211`(`deckcraft_update::helper::finish`, 첫 프레임 `acknowledge_startup()`), `crates/ui-egui/Cargo.toml`·`apps/deckcraft/Cargo.toml`(deckcraft-update 의존)
- Modify: `crates/ui-egui/tests/fork_identity.rs`(Word의 세 테스트를 Slide 값으로 추가)

**Interfaces:**
- Consumes: Task 3의 `deckcraft_update::{APP, helper, download, manifest, install}`.
- Produces: 동작 — 시작·Help·6시간 확인, 진행 카드, 준비 모달(Restart to update / Later), 저장 확인 후 재시작, 재시작 시 열린 파일 다시 열기, 성공 토스트.

- [ ] **Step 1:** Word `updates_ui/tests.rs`를 옮겨 deckcraft 테스트 하네스에 맞춤. 반드시 포함: `cancelled_save_prompt_cancels_update_restart`, `unrequested_check_failure_shows_no_window`(시작·주기 확인 실패 → 창 없음·상태 Idle), `requested_check_failure_shows_window`(Help 확인 실패 → 창). fork_identity에 `fork_app_identity_is_slide`(id `slide`, name `Slide`, developer `JeongEEE`, bundle id, 원본 `craft_update::suite::DECKCRAFT`와 다름), `fork_never_checks_upstream_releases`(repo `JeongEEE/deckcraft`), `fork_update_folders_are_separate_from_upstream`.
- [ ] **Step 2:** 실패 확인.
- [ ] **Step 3:** 구현·연결. 원본 craft-update의 시작 시 자동 확인이 남아 있지 않은지 `grep -rn 'craft_update::' crates/ui-egui/src apps/deckcraft/src`로 확인(남는 건 App 타입·UI 위젯 재사용뿐이어야 함).
- [ ] **Step 4:** `cargo test -p deckcraft-ui-egui`(Task 1 기준선 밖의 새 실패 0), `--test fork_identity` PASS, clippy·fmt, `cargo xtask wasm`.
- [ ] **Step 5:** 커밋 `feat: Slide 자동 다운로드와 저장 확인 후 재시작 업데이트 연결`.

### Task 5: macOS 제목줄을 끌어 창 이동

**Files:**
- Modify: `crates/ui-egui/src/ribbon/title_bar.rs:36-48` — Grid `crates/ui-egui/src/ribbon/title_bar.rs` diff를 옮김
- Create: `crates/ui-egui/tests/title_bar_drag.rs`(Grid `tests/it/title_bar_drag.rs`를 deckcraft 하네스로; deckcraft는 `tests/it/`이 없으므로 독립 테스트 파일)

**Interfaces:**
- Produces: macOS 전용 `fn drag_window(ui: &Ui, drag: &Response)` 류 함수 하나(이름은 Grid 것 유지). 다른 곳에서 `StartDrag`를 쓰는 제목줄이 생기면 이 함수를 재사용.

- [ ] **Step 1:** 테스트 `#[cfg(target_os = "macos")]`: 창 이동 명령(`OuterPosition`)을 다음 프레임 outer_rect에 반영하는 모델에서 제목줄 빈 곳을 눌러 10px씩 20번 끌면 창이 정확히 200px 이동, 마지막 이벤트가 놓기와 같은 프레임이어도 이동 반영, 포인터 이벤트 없는 재그리기 프레임에선 이동 명령 없음.
- [ ] **Step 2:** 원래 코드에서 FAIL 확인.
- [ ] **Step 3:** 구현: macOS에서 drag_started 때 누른 위치(창 기준) 저장, 이후 포인터 이벤트가 있는 프레임마다 `OuterPosition(현재 창 위치 + 포인터 − 누른 위치)`, `delta()` 사용 금지. 비 macOS는 `StartDrag` 유지. 더블클릭 최대화 유지.
- [ ] **Step 4:** 테스트 PASS. 실제 앱 실측: `HOME=<scratchpad>/home cargo run --release -p deckcraft -- --sample` 으로 띄우고 Grid 메모리의 방법(swiftc CGEvent 드래그 도구, System Events `position of window 1`, 대조군 Finder)으로 200px 드래그 → 200px 이동 확인. 실측 불가면 그 사실을 보고.
- [ ] **Step 5:** 커밋 `fix: macOS에서 제목줄을 끌어 창을 옮기지 못하던 문제 수정`.

### Task 6: 설치 화면 한국어 (WiX·DMG)

**Files:**
- Modify: `packaging/windows/package.ps1`(`wix build … -culture ko-KR -culture en-US` — Word 방식 그대로), `deckcraft.wxs`(포크가 직접 쓰는 문구: 앱 설명·다운그레이드 경고·마침 화면 "Slide 실행" 체크박스를 한국어로)
- Create: `packaging/windows/test-auto-update.ps1`, `packaging/windows/create-update-baseline.ps1`(Word에서 값 교체, 일회용 환경 전용 경고 유지)
- Modify: `packaging/macos/dmg/background.svg`(Word의 한국어 안내 path 이식, 제목 Slide), `background.tiff`·`dmg-layout.DS_Store`(재생성), `generate.py`·`README.md`(Word diff 참고)

**Interfaces:**
- Consumes: Task 2의 볼륨 이름 `Slide`.

- [ ] **Step 1:** `test_fork_contract.py`에 검사 추가: package.ps1에 `ko-KR`와 `en-US` culture, wxs 포크 문구가 한국어, `background.svg`에 원본 제목 문자열 없음.
- [ ] **Step 2:** FAIL 확인 → 수정 → PASS.
- [ ] **Step 3:** 재생성 `uv run --no-project --with 'pillow>=12' --with ds_store==1.3.3 --with mac_alias==2.2.3 python3 packaging/macos/dmg/generate.py`(resvg는 brew). `sips -s format png packaging/macos/dmg/background.tiff --out <scratchpad>/bg.png` 후 PNG를 Read로 눈 확인(안내 문구가 보이고, 아이콘 아래 비어 있음).
- [ ] **Step 4:** DMG를 다시 만들어 Finder에서 열었을 때 배경·배치 확인(스크린샷).
- [ ] **Step 5:** 커밋 `feat: Windows 설치 마법사 한국어와 Mac DMG 끌어 놓기 한국어 안내`.

### Task 7: 로컬 배포 스크립트

**Files:**
- Create: `deploy-macos.sh`, `deploy-windows.ps1`, `deploy.sh`, `packaging/deploy.py`, `packaging/deploy_console.py`, `packaging/local_build.py`, `packaging/release_assets.py`, `packaging/test_deploy.py`, `docs/custom-release.md` — Word의 것을 옮겨 값 교체(저장소·태그·자산·`SlideUpdate.*`·키 경로·`SLIDE_UPDATE_KEY_FILE`·커밋 메시지 `chore: Slide X.Y.Z 릴리스 버전 준비`·로그·락 이름 `slide-*`).
- Modify: `.gitignore`(배포 로그 등 Word와 같게)

**Interfaces:**
- Consumes: Task 2 자산 이름, Task 3 `slide-update-sign`.
- `local_build.py`: **craft-i18n 준비 단계 제거**. 글꼴은 `.github/workflows/release.yml`의 `craft-fonts-ref`(현재 `37827adc41fd165f9a9234ac40f94dbad353aa69`)를 빌드할 SHA에서 읽어 깨끗한 체크아웃을 `CRAFT_FONTS_DIR`로, `CRAFT_FONTS_REQUIRED=1`. CLI 검증은 `deckcraft-cli`(배포 이름 `slide-cli`) `--version`과 샘플 저장·재열기(`slide-cli run --sample … ` 로 `.pptx` 저장 후 다시 여는 명령 — deckcraft CLI 도움말에서 실제 명령 확인).
- macOS universal 빌드에 `x86_64-apple-darwin` target이 필요(현재 미설치). 스크립트는 없으면 설치 명령을 알려 주고 멈춘다(Word 동작 확인 후 같게).

- [ ] **Step 1:** `packaging/test_deploy.py`를 옮겨 값 교체, i18n 관련 테스트 삭제, 글꼴 ref를 release.yml에서 읽는 테스트 추가 → FAIL 확인.
- [ ] **Step 2:** 스크립트 옮기기·수정.
- [ ] **Step 3:** `python3 packaging/test_deploy.py` PASS, `./deploy-macos.sh --dry-run` 동작(실제 업로드 없음), `bash -n`, `pwsh`가 있으면 `deploy-windows.ps1` 구문 검사.
- [ ] **Step 4:** `docs/custom-release.md`에 브랜치 계약·식별자 표·빌드 준비(Windows: VsDevShell 로드, `wix extension add -g WixToolset.UI.wixext/5.0.2`·`WixToolset.Util.wixext/5.0.2`)·배포 절차·키 백업 경고.
- [ ] **Step 5:** 커밋 `feat: Slide Mac·Windows 로컬 빌드 배포 스크립트 추가`.

### Task 8: 운영 키·버전 (소유자 참여)

- [ ] **Step 1:** 소유자에게 `! ~/.cargo/bin/cargo run -q --release -p deckcraft-update --bin slide-update-sign -- keygen <인자는 Word custom-release.md와 동일 형식, 출력 디렉터리 ~/Dev/keystores/slide>` 실행을 요청(세션은 개인키를 읽지 않는다). 생성 후 `stat -f '%Lp' ~/Dev/keystores/slide ~/Dev/keystores/slide/update-private-key.hex`로 700/600만 확인.
- [ ] **Step 2:** 공개키 파일만 `crates/update/public-key.hex`로 복사, `pinned_release_key_matches_owner_key` 테스트 추가(그 공개키 값), `cargo test -p deckcraft-update` PASS.
- [ ] **Step 3:** workspace 버전 `0.19.0`→`0.19.1`, `Cargo.lock` 갱신.
- [ ] **Step 4:** 커밋 `feat: Slide 업데이트 서명 공개키 고정과 0.19.1 버전 준비`.

### Task 9: 전체 검증·배포·문서

- [ ] **Step 1:** `cargo xtask ci` 실행, 실패는 원본 `main` 워크트리(`~/Dev/MyProjects-worktrees/`)에서도 같은지 갈라 기록.
- [ ] **Step 2:** 적대적 리뷰(read-only 에이전트) — 식별자 누락(`grep -rn 'DeckCraft\|deckcraft' packaging apps/deckcraft/src`의 남은 건 의도인지), 업데이트 경로 원본 혼입, 비밀값.
- [ ] **Step 3:** `./deploy-macos.sh`로 0.19.1 Mac 빌드·초안 업로드. `HOME` 격리 실행으로 Slide.app 실행·제목줄 이동·About 확인.
- [ ] **Step 4:** 소유자에게 Windows PC에서 `deploy-windows.ps1 -Version 0.19.1 -Draft` 실행 요청 → 업로드 확인 후 Mac에서 `--finalize`(묻지 않고 공개).
- [ ] **Step 5:** 위키 `MyProjectsWiki/wiki/Slide/` 3문서 제안(승인 후 작성·`./sync-omc-wiki.sh`), 메모리 `project_deckcraft.md`(harness-configs 커밋).

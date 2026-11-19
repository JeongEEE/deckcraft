# Slide 개인 포크 설계

`JeongEEE/deckcraft`를 **Slide**라는 개인 앱으로 배포한다. 모델은 같은 소유자의 두 포크다.

- `JeongEEE/wordcraft` `my-custom`(앱 Word): 업데이트 crate·업데이트 UI·배포 스크립트·설치 화면 한국어·
  DMG 안내·Discord 제거의 **최신 정리본**. 기본 참조.
- `JeongEEE/gridcraft` `my-custom`(앱 Grid): 제목줄 파일 구조(`ribbon/title_bar.rs` + `StartDrag`)가
  deckcraft와 같으므로 **macOS 제목줄 창 이동**은 Grid 구현과 회귀 테스트를 옮긴다.

코드를 cherry-pick하지 않는다. 세 레포의 파일 배치가 달라 충돌만 커지므로 참조 구현의 최종 상태를
읽고 deckcraft 구조에 맞춰 값만 바꿔 옮긴다(`git -C ../wordcraft diff main...my-custom -- <경로>`).

## 왜

유료 PowerPoint 구독 부담을 줄이고, 개인 취향 수정을 얹을 수 있는 프레젠테이션 앱을 Grid·Word와
같은 방식(개인 포크·로컬 빌드 배포·서명 자동 업데이트)으로 운영하기 위해서다.

## 승인된 요구사항

- 앱 표시 이름 **Slide**, 유지관리자 표기 `JeongEEE`.
- `main`은 `upstream/main`(storytold/deckcraft) fast-forward 전용, 개인 변경은 `my-custom`.
- macOS에서 제목줄을 끌어 창을 옮길 수 있어야 한다(지금은 안 된다).
- Windows 설치 마법사 한국어, Mac DMG에 Applications로 끌어 놓으라는 한국어 안내.
- Mac/Windows 각자 PC에서 빌드해 GitHub 릴리스 초안에 올리고, Mac에서 서명 후 **확인 질문 없이 바로
  공개**. GitHub Actions는 배포에 쓰지 않는다. 배포 스크립트 `deploy-macos.sh`·`deploy-windows.ps1`.
- 설치본은 이 포크의 공개 릴리스만 보고 서명 검증 후 스스로 업데이트한다.
- 첫 공개 릴리스까지 실제로 낸다.

## 비목표 / 범위 밖

- **한국어 화면 번역**: deckcraft는 번역 장치가 없는 영어 전용 앱이고 craft-i18n에도 한국어 deckcraft
  카탈로그가 없다(de/fr/es만). 화면 문구 1,050개 이상을 새로 번역해야 해서 별도 작업으로 미룬다.
  단, 포크가 새로 만드는 문구(업데이트 카드·모달 등)도 이번엔 영어로 둔다 — 번역 장치를 그때 한꺼번에 만든다.
- 창 위치·크기 기억(deckcraft에 원래 없음), 리본 접기 버튼(deckcraft에 이미 접기 기능 있음).
- Linux·FreeBSD·웹 배포, 유료 코드 서명·공증, 업스트림 PR, 저장소 이름 변경.

## 안티 목표

- 원본 DeckCraft나 MS PowerPoint 설치·설정·파일 연결을 건드리거나 덮어쓰는 것.
- 업데이트가 원본 저장소 릴리스를 보고 원본 DeckCraft로 덮어쓰는 것.
- 개인키가 세션·명령 인자·로그·git에 들어오는 것.

## 접근 방법

| 방법 | 장단점 | 결정 |
|---|---|---|
| Word 최종 상태 기준 + 제목줄만 Grid | 설치 마법사(WiX 기본 대화상자)·craft-update 교체·craft-libs 고정 방식이 Word와 같고, Word가 Grid를 한 번 옮기며 다듬은 최신판. 제목줄 구조는 Grid와 같음 | **채택** |
| Grid 기준 | 설치 화면이 자체 Dialog라 deckcraft와 구조가 다르고, Word에서 고친 버그(요청 안 한 업데이트 확인 실패 창 등)가 남아 있음 | 기각 |
| 커밋 cherry-pick | 파일 배치가 달라 충돌 폭증 | 기각 |

## 원본과 분리하는 식별자

| 항목 | Slide |
|---|---|
| updater app id / 배포 파일 stem | `slide` |
| 업데이트 저장소 / 태그 | `JeongEEE/deckcraft` / `deckcraft-v<버전>` |
| 업데이트 목록 | `SlideUpdate.json` + `SlideUpdate.sig` |
| macOS 앱 / bundle id | `Slide.app` / `io.github.jeongeee.slide` |
| macOS 설정 | `~/Library/Application Support/Slide` |
| Windows 설치 폴더 | `%LOCALAPPDATA%\Programs\JeongEEE\Slide` |
| Windows 설정 / registry | `%APPDATA%\Slide` / `Software\JeongEEE\Slide` |
| Windows ProgID | `JeongEEE.Slide.*` — PowerPoint의 `PowerPoint.Show.*`와 겹치지 않게 |
| Windows 시작 메뉴 | `Slide (JeongEEE)` |
| Windows UpgradeCode·component GUID | `uuidgen`으로 새로 만들어 고정, 이후 바꾸지 않음 |
| 실행 파일 | `slide.exe`/`slide-cli.exe`, macOS `Slide.app`/`slide-cli` |
| 개인키 | `~/Dev/keystores/slide/update-private-key.hex`(dir 700, file 600), `SLIDE_UPDATE_KEY_FILE` |

Cargo 패키지·crate 이름(`deckcraft-*`)·명령 ID·`DECKCRAFT_*` 환경변수는 유지한다(병합 충돌 최소화).
Linux 설정 경로도 `~/.config/slide`로 분리한다(배포는 안 하지만 개발 실행이 원본 설정과 섞이지 않게).
파일 연결은 "연결 프로그램" 후보로만 등록하고 확장자의 기본 앱은 바꾸지 않는다.

## 원본 진입점 정리

- About·백스테이지의 ArtCraft 로고·워드마크·커뮤니티 링크, 제목줄·메뉴·백스테이지의 **Discord 진입점**을
  Word와 같은 기준으로 뺀다. 원본 저작권·라이선스·기여자 표기와 "DeckCraft 기반" 텍스트 출처는 남긴다.
- MCP 서버(`crates/mcp`)·CLI·엔진 `links.rs`의 Discord 문자열은 사용자 화면이 아니면 그대로 둔다(빌더 판단).

## 자동 업데이트

Word의 `crates/update`(package `deckcraft-update`, 서명 도구 `slide-update-sign`)와
`updates_ui.rs`(+`view.rs`, tests) 최종 상태를 옮긴다. deckcraft의 기존 `dialogs/updates_ui.rs`(craft-update)
자리를 대체한다. 동작은 Word와 같다.

- 시작 시·Help·6시간 주기로 포크의 공개 릴리스만 확인, 백그라운드 다운로드·검증.
- 오른쪽 아래 진행 카드 → 준비 완료 모달(Restart to update / Later) → 저장 확인을 거친 재시작 →
  성공 토스트. 재시작 시 열려 있던 파일을 다시 연다. 저장 확인에서 취소하면 업데이트 종료 예약도 취소.
- 요청하지 않은 확인(시작·주기)이 실패하면 창을 띄우지 않고 로그만(Word 1597f9aa).
- 공개키 `crates/update/public-key.hex`에 고정, 비어 있거나 틀리면 설치 거부(fail closed). 런타임 우회 없음.
- Mac: DMG readonly 마운트·bundle id·버전·codesign 확인 후 교체, 실패 시 복원.
  Windows: MSI는 원래 설치 문맥 유지, 포터블은 배포한 파일만 교체.
- deckcraft에 "일반 종료 시 저장 확인" 흐름이 Word와 다르면 deckcraft의 기존 종료 흐름에 연결한다(빌더 판단).

## macOS 제목줄 창 이동

Grid `ribbon/title_bar.rs`의 수정과 `tests/it/title_bar_drag.rs`를 옮긴다. 원리:
macOS에서는 끄는 동안 **포인터 이벤트가 있는 프레임마다** `OuterPosition`으로 창을 직접 옮겨 누른 지점을
포인터 아래에 둔다. 누른 위치는 drag_started 때 저장하고, 움직임 판단은 `delta()`가 아니라 이벤트 유무.
Windows·Linux는 `StartDrag` 유지. 백스테이지·별도 viewport 대화상자 제목줄에도 같은 문제가 있으면
같은 함수를 재사용한다. 테스트는 이동 명령을 outer_rect에 반영하는 모델이어야 한다(창 위치 고정 모델은
버그를 못 잡는다). 최종 확인은 실제 앱에서 끌어 보는 실측(200px → 200px).

## 설치 화면

- **Windows**: `deckcraft.wxs`는 `WixUI_InstallDir`(WiX 기본 대화상자)라 Word처럼 `-culture ko-KR;en-US`로
  빌드하면 WiX UI 확장 5.0.2 내장 한국어가 나온다. 포크가 직접 넣는 문구(앱 설명·다운그레이드 경고·마침
  화면 실행 체크박스 등)만 한국어로 쓴다.
- **Mac DMG**: Word `packaging/macos/dmg/`의 한국어 안내(outline path, 글꼴 없이 그려짐)를 옮겨 제목을
  Slide로, 볼륨 이름을 버전 없는 `Slide`로 고정한다. `generate.py`로 재생성하고 PNG로 눈 확인.

## 배포 스크립트

Word의 `deploy-macos.sh`, `deploy-windows.ps1`, `deploy.sh`, `packaging/deploy.py`, `deploy_console.py`,
`local_build.py`, `release_assets.py`, `test_deploy.py`, `test_fork_contract.py`,
`windows/test-auto-update.ps1`·`create-update-baseline.ps1`을 옮겨 값만 바꾼다.

- 같은 SHA·버전으로 두 OS 업로드 → 키가 있는 Mac에서 `--finalize`(9개 자산 재확인·서명) → 묻지 않고 공개.
  `--draft`는 공개를 미룬다.
- deckcraft는 craft-i18n을 쓰지 않으므로 Word의 i18n 준비 단계는 뺀다. 글꼴은 `craft-fonts` 고정 커밋
  (원본 `release.yml`의 `craft-fonts-ref`)을 깨끗한 체크아웃으로 `CRAFT_FONTS_DIR`에 넣는다.
- CLI 검증은 deckcraft CLI의 `--version`과 샘플 프레젠테이션 저장·재열기.
- 자산: `slide-<v>-macos-universal.dmg`, `slide-cli-<v>-macos-universal.zip`, `slide-<v>-windows-x64.msi`,
  `slide-<v>-windows-x64-portable.zip`, OS별 빌드 증빙 JSON 2개, `SlideUpdate.json`·`.sig`, `SHA256SUMS.txt`.

## 버전

정본은 `Cargo.toml` `[workspace.package].version`. major.minor는 업스트림, patch는 포크 배포마다 올린다.
업스트림 0.19.0 기준 첫 포크 버전 **0.19.1**. 공개한 버전은 덮어쓰지 않는다.

## 운영 키 (소유자 참여)

소유자가 직접 터미널에서 `!`로 `slide-update-sign keygen`을 한 번 실행해 `~/Dev/keystores/slide/`에 만든다.
세션은 공개키 파일만 읽어 `public-key.hex`에 넣고 커밋한다. 개인키 백업은 소유자 몫.

## 문서

- `AGENTS.md` 맨 위 Slide 포크 규칙, `docs/custom-release.md`(브랜치·식별자·빌드·배포 절차), README 정리.
- 위키 `MyProjectsWiki/wiki/Slide/` 3문서(overview·maintenance·release-update) — 새 폴더라 소유자 승인 후.

## 검증

- `cargo test -p deckcraft-update`, fork identity 테스트, `title_bar_drag`, `packaging/test_fork_contract.py`,
  `packaging/test_deploy.py`, clippy `-D warnings`·fmt. 원래 있던 실패는 원본 `main`에서도 같은지 갈라 기록.
- Mac: `package.sh --arch universal`로 `Slide.app` DMG, codesign·plist·버전, DMG 배경 눈 확인, 실제 앱 제목줄
  드래그 실측, CLI 저장·재열기, DMG 업데이트·복원 smoke.
- Windows는 이 Mac에서 빌드 불가. 정적 정책 검사와 스크립트 테스트까지 하고, 실제 빌드·MSI 한국어 화면은
  소유자 Windows PC에서 `deploy-windows.ps1 -Version 0.19.1 -Draft`로 확인 후 Mac에서 finalize.
- 개인 PC의 기존 DeckCraft 설치·설정은 건드리지 않는다. 실행 검증은 `HOME=<scratchpad>` 격리.

## 빌더에게 맡긴 것

파일 단위 이식 순서, deckcraft 구조에 맞춘 연결 지점(종료 흐름·Help 메뉴·백스테이지), 테스트 세부 구성,
Discord 비화면 문자열 처리, 커밋 분할(Task 단위).

# Slide 포크 개발·배포

Slide는 `JeongEEE/deckcraft`에서 관리하는 DeckCraft 기반 개인 프레젠테이션 앱이다.
사용자에게 보이는 앱 이름은 **Slide**, 유지관리자 표기는 `JeongEEE`, Cargo 패키지는
`deckcraft`와 `deckcraft-cli`다. 원본의 crate 이름, 명령 ID, `DECKCRAFT_*` 환경변수를
유지해 업스트림 변경을 계속 받아들인다. 파일 형식 이름("DeckCraft Presentation (*.deckcraft)")과
내장 테마 이름 "DeckCraft"는 앱 이름이 아니므로 그대로 둔다.

## 브랜치와 원격

| 대상 | 역할 |
|---|---|
| `upstream` | `https://github.com/storytold/deckcraft.git` |
| `origin` | `https://github.com/JeongEEE/deckcraft.git` |
| `main` | 개인 커밋 없이 `upstream/main`만 동기화 (fast-forward 전용) |
| `my-custom` | 개인 변경 개발·검증·커밋 |

업스트림을 반영하는 순서는 다음과 같다. 충돌은 `my-custom`에서 해결한다.

```sh
git fetch --no-tags upstream
git switch main
git merge --ff-only upstream/main
git push origin main
git switch my-custom
git merge main
cargo xtask ci
```

커스텀을 `main`에 역병합하지 않는다. 공개한 커스텀 브랜치는 merge로 갱신하며
일반적인 유지보수에 force push나 rebase를 사용하지 않는다.

## 원본과 분리한 식별자

| 항목 | Slide |
|---|---|
| updater app id / 배포 파일 stem | `slide` |
| 업데이트 저장소 / 태그 | `JeongEEE/deckcraft` / `deckcraft-v<버전>` |
| 업데이트 목록 | `SlideUpdate.json` + `SlideUpdate.sig` |
| macOS 앱 / bundle id | `Slide.app` / `io.github.jeongeee.slide` |
| macOS 설정 | `~/Library/Application Support/Slide` |
| Linux·기타 설정 | `$XDG_CONFIG_HOME/slide` (기본 `~/.config/slide`) |
| Windows 설치 폴더 | `%LOCALAPPDATA%\Programs\JeongEEE\Slide` (모든 사용자: `Program Files\JeongEEE\Slide`) |
| Windows 설정 / registry | `%APPDATA%\Slide` / `Software\JeongEEE\Slide` |
| Windows ProgID | `JeongEEE.Slide.*` (PowerPoint의 `PowerPoint.Show.*`와 겹치지 않게) |
| Windows 시작 메뉴 바로가기 | `Slide (JeongEEE)` |
| Windows UpgradeCode | `1254CE28-A4BC-4E31-BA0A-DC5D959DA341` |
| Windows component GUID | 앱 `CCE82C65-C471-42C0-98B6-F67F5A92D6A3` |
| 실행 파일 | `slide.exe`, `slide-cli.exe` / macOS `Slide.app`, `slide-cli` |
| 배포 파일 | `slide-<버전>-macos-universal.dmg`, `slide-cli-<버전>-macos-universal.zip`, `slide-<버전>-windows-x64.msi`, `slide-<버전>-windows-x64-portable.zip` |

UpgradeCode와 component GUID는 이후 릴리스에서 바꾸지 않는다. 바꾸면 기존 설치를
업그레이드하지 못하고 옆에 하나 더 설치된다. 파일 연결은 "연결 프로그램" 후보로만
등록하고 확장자의 기본 앱은 바꾸지 않는다. 원본 DeckCraft나 PowerPoint의 설치·설정·파일
연결은 건드리지 않는다. ArtCraft 로고·워드마크는 배포물과 화면에서 뺐고, DeckCraft 기반이라는
텍스트 출처와 원본 저작권·라이선스는 남긴다.

## 한국어 화면

화면 언어는 시스템 언어를 따른다(데스크톱 앱이 시작할 때 한 번 읽는다). Preferences의
"언어"에서 자동·한국어·English를 고를 수 있고, 고른 값은 `ui.json`의 `language`에 저장된다.
웹(wasm) 빌드는 배포하지 않으며 한글 글꼴이 없어 한국어를 지원하지 않는다.

번역표는 `crates/ui-egui/i18n/ko/` 아래 두 개다. `common.tsv`는 craft-i18n 공용 목록의 사본이고,
`deckcraft.tsv`는 Slide 포크 목록이다. 같은 영어 키가 둘에 있으면 `deckcraft.tsv` 행이 이긴다.
영어로 두는 화면 문구는 이유와 함께 `crates/ui-egui/i18n/keep_english.txt`에 적는다.
새 화면 문구는 `t()`·`tc()`를 거쳐 그리고 `deckcraft.tsv`에 행을 넣는다.

빠진 번역은 테스트가 잡는다. `korean_coverage_all`은 `crates/ui-egui/src` 전체 소스에서 행이
없거나 `t()`를 거치지 않은 화면 문구를 찾고, `korean_screens_show_no_untranslated_labels`는
한국어로 리본 탭·백스테이지·대화상자·작업창을 그려 영어로만 된 라벨이 남았는지 본다.
두 테스트는 글꼴을 넣은 빌드(`CRAFT_FONTS_DIR`)에서도 돌린다.

번역은 그릴 때만 한다. 파일에 저장되는 값(슬라이드 텍스트, 레이아웃·테마·개체 이름),
명령 ID, `ui.json` 저장값, 제어 채널·MCP·CLI·로그는 영어 그대로다. 날짜·시간 형식 목록은
en-US 고정이라 한국어 화면에서도 영어 날짜가 나온다(한국어 날짜 형식은 범위 밖).

## 빌드와 검증

Rust 1.95 이상, rustfmt, clippy, `wasm32-unknown-unknown` target이 필요하다.
macOS universal 패키지에는 `x86_64-apple-darwin` target도 설치한다.
없으면 배포 스크립트가 빌드 전에 `rustup target add x86_64-apple-darwin` 안내와 함께 멈춘다.

```sh
rustup component add rustfmt clippy
rustup target add wasm32-unknown-unknown x86_64-apple-darwin
cargo build --release --locked -p deckcraft -p deckcraft-cli
python3 packaging/test_fork_contract.py
cargo xtask ci
```

macOS에는 Xcode 또는 Command Line Tools가 필요하다. 개발 도구 경로가 불완전한
Command Line Tools를 가리키지만 Xcode가 설치돼 있다면, 전역 설정을 바꾸지 않고
빌드 명령에 `DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer`를 지정할 수 있다.
Rust가 PATH에 없다면 먼저 `. "$HOME/.cargo/env"`를 실행한다.

DeckCraft는 `craft-i18n`을 쓰지 않는다(화면은 영어 전용). 글꼴만 `storytold/craft-fonts`의
고정 커밋을 깨끗한 체크아웃으로 받아 `CRAFT_FONTS_DIR`로 지정하고, 배포 빌드는
`CRAFT_FONTS_REQUIRED=1`로 누락을 막는다.

macOS 패키징은 `packaging/macos/package.sh --arch universal`, Windows는 MSVC 도구와
WiX 5.0.2를 준비한 뒤 `pwsh packaging/windows/package.ps1 -Arch x64`로 실행한다.
Windows 설치 정책은 Windows에서 `pwsh packaging/windows/test-install-policy.ps1`로 검사한다.

## OS별 로컬 배포

빌드는 각자의 Mac/Windows에서 실행하고 GitHub에는 완성된 파일만 업로드한다.
Actions 실행·runner·배포 브랜치 푸시는 사용하지 않는다. `deploy.sh`는 Mac 명령의 별칭이다.
두 컴퓨터 모두 `my-custom`의 **같은 커밋과 버전**을 사용해야 한다. 한 릴리스를 마무리하기
전에는 소스 변경을 추가하지 않고 두 컴퓨터에서 배포 명령을 동시에 실행하지 않는다.

두 OS 스크립트는 같은 형식으로 단계·소요 시간·결과를 표시한다. 터미널에서는 색상과
진행 애니메이션을 사용하고, 파이프·파일 출력에서는 일반 텍스트를 사용한다. `NO_COLOR`로
색상을 끌 수 있다. 컴파일·패키징의 상세 출력은 git 공통 디렉터리의
`slide-deploy-logs/*.log`에 실행별로 남기며, 실패하면 마지막 오류와 로그 경로,
같은 버전·옵션을 유지한 재시도 명령을 표시한다. `--dry-run`은 로그 파일도 만들지 않는다.
같은 저장소에서 배포가 겹치지 않도록 git 공통 디렉터리의 `slide-deploy.lock`을 잡는다.

완료 화면은 버전·전체 소스 SHA·OS별 업로드 상태·업데이트 서명 상태·초안 링크를 보여준다.
Mac 업로드 후에는 Windows의 pull·커밋 확인·`-Version … -Draft` 명령을,
Windows 업로드 후 두 OS가 모두 있으면 Mac의 pull·커밋 확인·`--finalize` 명령을 안내한다.
Windows부터 실행했다면 먼저 Mac 빌드를 안내한다. 안내에 표시된 SHA를 확인하고
다음 단계로 넘어가며, 이미 공개한 결과에는 finalize를 다시 안내하지 않는다.

### 최초 환경 준비

공통: Python 3.11 이상, git, Rust 1.95.0 이상, GitHub CLI(`gh`)와 포크 쓰기 권한으로
`gh auth login`이 필요하다. 의존성 설치는 자동으로 시스템 설정을 변경하지 않도록 별도로 한다.

Mac: Xcode와 두 Rust target을 준비한다. Xcode가 있으면 스크립트가 `DEVELOPER_DIR`를 설정한다.
Cargo가 PATH에 없어도 `CARGO_HOME/bin`(미설정 시 `~/.cargo/bin`)에서 찾고 그 경로를
하위 Bash/PowerShell에도 전달한다. 전역 PATH나 셸 설정 파일은 변경하지 않는다.

```sh
rustup target add aarch64-apple-darwin x86_64-apple-darwin
```

Windows: PowerShell 7(`pwsh`), Visual Studio Build Tools의 **Desktop development with C++**
(MSVC x64, Windows SDK), .NET SDK와 WiX 5.0.2를 준비한다. PowerShell 실행 정책이 차단하면
해당 터미널에서만 `Set-ExecutionPolicy -Scope Process Bypass`를 사용한다.
업데이트 개인키를 Windows로 복사할 필요는 없다. GitHub 인증은 Windows에서 별도로
`gh auth login`을 실행하고, 두 OS 업로드 후 키가 있는 Mac에서 서명·공개를 마무리한다.

**Windows 준비에서 빠뜨리기 쉬운 두 가지.** 둘 다 빠지면 긴 빌드가 한참 돈 뒤에야 실패한다.

1. **VsDevShell을 같은 PowerShell 안에서 먼저 로드한다.** `rc.exe` 같은 SDK 도구 경로와 MSVC
   환경이 잡혀야 한다. 새 창을 띄우지 말고 배포 명령을 실행할 그 셸에서 로드한다.

   ```powershell
   $vs = & "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe" -latest -products * -property installationPath
   & "$vs\Common7\Tools\Launch-VsDevShell.ps1" -Arch amd64 -SkipAutomaticLocation
   ```

2. **WiX 확장 두 개를 전역으로 등록한다.** 설치 마법사 UI와 Util(파일 연결 등) 확장이 없으면
   MSI 빌드가 거의 끝난 시점에 `WIX0144`로 실패한다.

   ```powershell
   dotnet tool install --global wix --version 5.0.2
   wix extension add -g WixToolset.UI.wixext/5.0.2
   wix extension add -g WixToolset.Util.wixext/5.0.2
   wix extension list -g
   ```

```powershell
rustup target add x86_64-pc-windows-msvc
gh auth login
git clone --branch my-custom https://github.com/JeongEEE/deckcraft.git
cd deckcraft
```

빌드 입력은 글꼴 하나다. 빌드할 커밋의 `.github/workflows/release.yml`에 있는 `craft-fonts-ref`
값(작업 트리가 아니라 그 커밋의 파일)이 가리키는 `storytold/craft-fonts` 커밋을
로컬 git 공통 디렉터리의 `slide-release-fonts/<커밋>`에 받아 `CRAFT_FONTS_DIR`·
`CRAFT_FONTS_REQUIRED=1`로 넘긴다. 값을 찾지 못하거나 40자리 커밋이 아니면 배포를 멈춘다.
`CRAFT_FONTS_DIR`를 직접 지정했다면 그 경로가 있어야 하고 같은 고정 커밋의 깨끗한 체크아웃이어야
한다. 아니면 배포를 멈춘다. 첫 빌드는 오래 걸리고 이후에는 로컬 Cargo target 캐시를 재사용한다.

### 두 컴퓨터에서 실행

미공개 버전 `0.19.1`을 배포하는 예다. 어느 OS부터 실행해도 된다.

```sh
# Mac
./deploy-macos.sh 0.19.1 --draft
```

```powershell
# Windows (VsDevShell을 로드한 같은 PowerShell에서)
git pull --ff-only origin my-custom
.\deploy-windows.ps1 -Version 0.19.1 -Draft
```

버전을 생략하면 **현재 소스가 미공개 버전일 때 그대로 사용**한다. 이미 공개된 버전이라면
첫 PC에서만 다음 패치를 준비하고 `Cargo.toml`·`Cargo.lock`을
`chore: Slide <버전> 릴리스 버전 준비`로 커밋·푸시한다. 두 번째 PC는
그 커밋을 pull한 뒤 같은 버전을 사용한다. 명시한 공개 버전·낮은 버전은 거부한다.

각 PC는 자기 OS의 두 패키지와 빌드 증빙 JSON만 같은 초안에 올린다. 빌드는 미추적 파일을
제외한 확정 커밋의 임시 로컬 clone에서 수행한다. 긴 빌드 중 원래 체크아웃이 바뀌면
업로드를 중단한다. 빌드 실패 후 **자산이 0개이고 태그도 없는 미공개 초안**만 예외로,
같은 버전의 이전 커밋에서 현재 커밋으로 자동 갱신한다. 이전 커밋은 현재 소스의 조상이어야
하며 기존 Source commit 기록도 일치해야 한다. 이미 파일·태그가 있거나 공개된
릴리스는 재지정하지 않는다. 두 OS 중 하나라도 업로드했다면 같은 커밋으로 맞추거나 새 버전을
준비한다. `--finalize`는 빈 초안을 갱신하지 않는다.

Windows PC에는 업데이트 개인키가 없어도 된다. 두 OS가 모이면 키가 있는 Mac에서
빌드 없이 검증·서명·공개를 마무리한다.

```sh
./deploy-macos.sh 0.19.1 --finalize
```

개인키는 아래 "업데이트 서명 키" 절의 위치에 둔다. 다른 위치는
`SLIDE_UPDATE_KEY_FILE` 또는 `--key-file <경로>`로 지정하며 Windows는 `-KeyFile <경로>`와
`-Finalize`를 쓴다.

두 플랫폼을 내려받아 증빙의 버전·SHA·글꼴 커밋·검증 기록·파일 해시를 확인하고,
`SlideUpdate.json`·`SlideUpdate.sig`와 체크섬을 만든다. 업로드한 최종 9개 자산의 바이트도
다시 확인하면 묻지 않고 바로 공개해 자동 업데이트를 배포한다. 공개를 미루려면
`--draft`/`-Draft`를 붙여 서명된 초안으로 남긴다. 키가 있는 PC에서 두 번째 OS 빌드를
`--draft` 없이 실행하면 `--finalize`를 따로 실행하지 않고 이 단계까지 이어져 바로 공개된다.

`--dry-run`/`-DryRun`은 네트워크·파일·커밋을 변경하지 않고 예정된 동작을 보여 준다.
실패하면 같은 버전으로 재실행한다. 버전 준비 중 미커밋 파일이 남았다면 먼저 확인한다.
파일이 올라간 초안의 소스를 바꿔야 한다면 새 버전을 준비한다. 빈 초안의 갱신 중 다른
업로드·태그·메타데이터 변경이 감지되면 중단하므로 두 PC에서 동시에 배포하지 않는다.

### 로컬 검증 범위

양쪽 모두 `packaging/test_fork_contract.py`(fork identity), `cargo test --locked -p deckcraft-update`,
CLI 검사를 실행한다. CLI 검사는 `slide-cli --version`에 배포 버전이 들어 있는지 보고, 내장 샘플을
`run --sample --save sample.pptx`로 저장한 뒤 `info sample.pptx`로 다시 열어 첫 슬라이드 제목이
읽히는지 확인한다. Mac은 `packaging/macos/package.sh --arch universal`로 만든 DMG의 서명과
임시 앱 교체·실패 시 복원(`native_dmg_roundtrip`)을 확인한다. Windows는
`packaging/windows/package.ps1 -Arch x64`의 MSI 정적 설치 정책과 임시 폴더의 포터블
업데이트·복원(`native_windows_portable_roundtrip`)을 검사한다.
**개인 PC의 기존 DeckCraft·Slide를 설치·제거하지 않는다.** 실제 MSI 설치·업그레이드 회귀는 깨끗한
일회용 Windows 환경에서만 `packaging/windows/test-auto-update.ps1`로 별도 실행한다.
정적 검증과 실제 설치 검증을 혼동하지 않는다.

배포 오케스트레이터 회귀 테스트는 `python3 packaging/test_deploy.py`다. 임시 git
원격과 통제된 GitHub/컴파일러 경계를 사용하며 실제 배포·공개를 수행하지 않는다.

## 업데이트 서명 키

자동 업데이트는 릴리스 목록(`SlideUpdate.json`)의 서명을 앱에 고정된 공개키로 검증한다.

- 개인키 정본: `~/Dev/keystores/slide/update-private-key.hex`. 저장소 밖이며 디렉터리 권한
  `700`, 파일 권한 `600`. 저장소를 지우거나 다시 클론해도 유지한다.
- 공개키: `crates/update/public-key.hex`에 고정해 git에 커밋한다. 환경변수로 덮어쓰는
  런타임 우회는 없고, `cargo test -p deckcraft-update`가 고정값을 확인한다. 개인키가 고정
  공개키와 다르면 `slide-update-sign`이 서명을 거부해 공개 전에 멈춘다.
- 개인키 값은 세션·명령 인자·출력·git에 넣지 않는다. 배포 스크립트가 파일에서 읽어
  `SLIDE_UPDATE_PRIVATE_KEY` 환경변수로 서명 하위 프로세스(`slide-update-sign`)에만 전달한다.
  파일 위치는 `SLIDE_UPDATE_KEY_FILE` 또는 `--key-file`로 바꾼다.
- **키를 잃으면 이미 설치된 Slide가 새 키를 믿지 못해 더는 업데이트할 수 없고, 사용자가
  직접 재설치해야 한다. 개인키는 반드시 안전한 곳에 따로 백업한다(저장소·클라우드 동기화 폴더
  밖의 암호화된 보관처 등).** 키 교체는 자동으로 전파되지 않는다.

키 생성은 소유자가 최초 한 번만 터미널에서 직접 한다. 세션(AI 에이전트)은 개인키를 만들거나
읽지 않고 공개키 파일만 `crates/update/public-key.hex`에 반영한다. keygen은 기존 파일이 있으면
거부하지만 마지막 `cp`는 고정 공개키를 덮어쓰므로, **이미 키를 만든 뒤에는 다시 실행하지 않는다.**

```sh
umask 077
mkdir -m 700 "$HOME/Dev/keystores/slide"
cargo run --locked -p deckcraft-update --bin slide-update-sign -- keygen \
  "$HOME/Dev/keystores/slide/update-private-key.hex" \
  "$HOME/Dev/keystores/slide/update-public-key.hex"
cp "$HOME/Dev/keystores/slide/update-public-key.hex" crates/update/public-key.hex
```

## 버전과 릴리스

버전의 정본은 `Cargo.toml`의 `[workspace.package].version`이다. 업스트림 0.19.0 기준
첫 포크 버전은 `0.19.1`이다. 다음 버전은 `cargo run -p xtask -- version set <버전>`으로 바꾸고
`Cargo.toml`과 `Cargo.lock`을 함께 커밋한다. 배포 스크립트는 이미 공개된 버전이면 이 작업을
대신한다.

버전의 major.minor는 업스트림 `main`을 따라가고 patch는 포크 배포마다 올라간다.
업스트림을 병합할 때 업스트림의 major.minor가 포크보다 높으면 포크 버전을 `X.Y.0`으로
바꾸고(예: 원본 `0.20.0`, 포크 `0.19.5` → `0.20.0`), 같으면 업스트림의 patch와 상관없이
포크 버전을 유지한다. 이후 배포 스크립트가 공개된 번호 다음 patch를 준비하므로
`X.Y.1`, `X.Y.2` 순으로 올라간다. 다음 배포 번호는 항상 이미 공개한 번호보다 높아야 한다.

태그는 **`deckcraft-v<버전>`**, 릴리스 제목은 `Slide v<버전>`이다. 원본의 `v<버전>` 태그와
충돌하지 않으며, updater가 저장소 이름 접두사가 붙은 버전을 해석한다. 저장소 이름 변경은
이 규약과 업데이트 주소를 함께 바꾸는 별도 작업이다.

릴리스 초안은 위 OS별 스크립트가 `my-custom`의 확정 SHA로 생성한다. upstream workflow는
동기화용으로 유지하며 이 로컬 배포 경로는 해당 workflow를 호출하거나 그 트리거 브랜치에
푸시하지 않는다. 최종 자산은 9개다.

- `slide-<버전>-macos-universal.dmg`: Apple Silicon·Intel용 Slide.app.
- `slide-cli-<버전>-macos-universal.zip`: macOS CLI(`slide-cli`).
- `slide-<버전>-windows-x64.msi`: Windows 설치 프로그램.
- `slide-<버전>-windows-x64-portable.zip`: 설치 없이 실행하는 Windows 파일.
- `slide-<버전>-macos-build.json`, `slide-<버전>-windows-build.json`: 소스·글꼴 커밋·검증·파일 해시 증빙.
- `SlideUpdate.json`, `SlideUpdate.sig`, `SHA256SUMS.txt`: 업데이트 키로 서명한 업데이트 목록과 체크섬.

draft는 저장소 협업자만 볼 수 있다. 두 OS가 다 갖춰지기 전에는 초안을 공개하지 않는다.
공개된 버전은 덮어쓰지 않는다.

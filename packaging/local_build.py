"""Build and test on the current desktop OS. Never install/remove the user's Slide."""
import json
import os
from contextlib import contextmanager
from pathlib import Path
import re
import tempfile

from release_assets import DeployError, package_names

FONTS_URL = 'https://github.com/storytold/craft-fonts.git'


def pinned_checkout(d, variable, url, sha, cache, label):
    configured = d.env.get(variable)
    folder = Path(configured).expanduser().resolve() if configured else d.common/cache/sha
    if not folder.exists():
        if configured:
            raise DeployError(f'{variable}가 없습니다: {folder}')
        folder.parent.mkdir(parents=True, exist_ok=True)
        d.command('git', 'clone', '--no-checkout', url, folder, capture=False, label=f'릴리스 {label} 다운로드')
        d.command('git', '-C', folder, 'checkout', '--detach', sha, capture=False, label=f'고정 {label} 버전 준비')
    actual = d.command('git', '-C', folder, 'rev-parse', 'HEAD').stdout.strip()
    dirty = d.command('git', '-C', folder, 'status', '--porcelain').stdout.strip()
    if actual != sha or dirty:
        raise DeployError(f'릴리스 {label}은 고정 커밋 {sha[:8]}의 깨끗한 체크아웃이어야 합니다: {folder}')
    return folder


def parse_fonts_ref(text):
    # The shared release workflow takes the pin as `craft-fonts-ref: <40-hex commit>`.
    found = re.findall(r'^\s*craft-fonts-ref:\s*([0-9a-f]{40})\s*$', text, re.MULTILINE)
    if len(found) != 1:
        raise DeployError('빌드할 소스의 .github/workflows/release.yml에서 글꼴 고정 커밋(craft-fonts-ref, 40자리)을 찾지 못했습니다.')
    return found[0]


def fonts_ref(d):
    # The pin comes from the commit being built, not the working tree.
    result = d.command('git', 'show', f'{d.sha}:.github/workflows/release.yml', optional=True)
    if result.returncode:
        raise DeployError('빌드할 소스에 .github/workflows/release.yml이 없어 글꼴 고정 커밋을 읽지 못했습니다.')
    return parse_fonts_ref(result.stdout)


def prepare_fonts(d):
    fonts = pinned_checkout(d, 'CRAFT_FONTS_DIR', FONTS_URL, fonts_ref(d), 'slide-release-fonts', '글꼴')
    d.env.update(CRAFT_FONTS_DIR=str(fonts), CRAFT_FONTS_REQUIRED='1')


@contextmanager
def committed_source(d):
    # Build only committed files. Untracked work and edits in the shared checkout
    # cannot become release input while another session uses it.
    with tempfile.TemporaryDirectory(prefix='slide-build-source-') as scratch:
        source = Path(scratch)/'source'
        d.command('git', 'clone', '--quiet', '--shared', '--no-checkout', d.root, source)
        d.command('git', '-C', source, 'checkout', '--quiet', '--detach', d.sha)
        yield source


def build(d, output):
    prepare_fonts(d)
    d.assert_source()
    with committed_source(d) as source:
        return build_source(d, output, source)


def require_universal_targets(d):
    # Universal builds link both slices; a missing target otherwise fails minutes into the build.
    sysroot = Path(d.command('rustc', '--print', 'sysroot').stdout.strip())
    if not (sysroot/'lib/rustlib/x86_64-apple-darwin').is_dir():
        raise DeployError('x86_64-apple-darwin Rust target이 없습니다. 먼저 `rustup target add x86_64-apple-darwin`을 실행하세요.')


def build_source(d, output, source):
    d.env.update(DIST=str(output), DECKCRAFT_VERSION=d.version, DECKCRAFT_BUILD_SHA=d.sha,
                 CARGO_TARGET_DIR=str(d.root/'target'))
    d.env.pop('SLIDE_UPDATE_SMOKE_PACKAGE', None)
    d.env.pop('SLIDE_UPDATE_SMOKE_INSTALL', None)
    def run(*args, **kwargs):
        return d.command(*args, cwd=source, **kwargs)

    if d.args.platform == 'macos':
        require_universal_targets(d)
    run(os.sys.executable, 'packaging/test_fork_contract.py', capture=False, label='포크 설치 식별자 검증')
    run(d.cargo, 'test', '--locked', '-p', 'deckcraft-update', capture=False, label='자동 업데이트 회귀 검사')
    if d.args.platform == 'macos':
        run('bash', 'packaging/macos/package.sh', '--arch', 'universal', capture=False, label='macOS universal 빌드·DMG 패키징')
        # The signed binary that goes into the CLI ZIP.
        cli = d.root/f'target/macos-package/slide-cli-{d.version}-macos-universal/slide-cli'
        run(d.cargo, 'test', '--locked', '-p', 'deckcraft-update', '--lib', 'native_dmg_roundtrip', '--', '--ignored',
                  capture=False, label='DMG 업데이트·실패 시 복원 검증',
                  env=dict(d.env, SLIDE_UPDATE_SMOKE_PACKAGE=str(output/package_names(d.version, 'macos')[0])))
    else:
        run('pwsh', '-NoProfile', '-File', 'packaging/windows/package.ps1', '-Arch', 'x64',
            capture=False, label='Windows x64 빌드·MSI/ZIP 패키징')
        cli = d.root/'target/windows-package/x64/slide-cli.exe'
        # The MSI's static policy check is part of package.ps1. Installation tests are
        # deliberately limited to a disposable machine; never touch a personal install.
        run(d.cargo, 'test', '--locked', '-p', 'deckcraft-update', '--lib', 'native_windows_portable_roundtrip', '--', '--ignored',
                  capture=False, label='포터블 업데이트·실패 시 복원 검증',
                  env=dict(d.env, SLIDE_UPDATE_SMOKE_PACKAGE=str(output/package_names(d.version, 'windows')[1])))
    if d.version not in d.command(cli, '--version').stdout:
        raise DeployError('빌드한 CLI의 버전이 다릅니다.')
    with tempfile.TemporaryDirectory(prefix='slide-cli-smoke-') as scratch:
        document = Path(scratch)/'sample.pptx'
        d.command(cli, 'run', '--sample', '--save', document, capture=False, label='PPTX 저장 검증')
        slides = json.loads(d.command(cli, 'info', document).stdout).get('slides', [])
        # First slide of the built-in sample presentation.
        if not slides or slides[0].get('title') != 'Northern Lights':
            raise DeployError('CLI로 저장한 PPTX를 다시 열어 읽지 못했습니다.')
    d.assert_source()
    return ['fork-identity', 'updater-tests', 'package-policy', 'cli-smoke']

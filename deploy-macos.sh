#!/usr/bin/env bash
set -euo pipefail
SLIDE_DEPLOY_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
exec python3 "$SLIDE_DEPLOY_ROOT/packaging/deploy.py" --platform macos "$@"

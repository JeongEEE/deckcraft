#!/usr/bin/env bash
# Compatibility alias; all builds now run locally on macOS.
set -euo pipefail
SLIDE_DEPLOY_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
exec "$SLIDE_DEPLOY_ROOT/deploy-macos.sh" "$@"

#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
# Build on the destination OS. No install/uninstall scripts enter the payload.
exec python3 scripts/package.py "$@"

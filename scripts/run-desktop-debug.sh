#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DESKTOP_DIR="${ROOT}/desktop"

DEFAULT_URL="https://rts-0-zvorygin.fly.dev/?rtsDebug=1"
export RTS_DESKTOP_URL="${RTS_DESKTOP_URL:-${DEFAULT_URL}}"
export RTS_TAURI_OPEN_DEVTOOLS="${RTS_TAURI_OPEN_DEVTOOLS:-1}"

echo "Launching desktop debug shell"
echo "URL: ${RTS_DESKTOP_URL}"
echo "Devtools: ${RTS_TAURI_OPEN_DEVTOOLS}"
echo

cd "${DESKTOP_DIR}"
exec cargo tauri dev "$@"

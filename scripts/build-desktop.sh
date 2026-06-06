#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DESKTOP_TAURI_DIR="${ROOT}/desktop/src-tauri"
BUNDLE_DIR="${DESKTOP_TAURI_DIR}/target/release/bundle"
APP_PATH="${BUNDLE_DIR}/macos/Bewegungskrieg.app"

cd "${DESKTOP_TAURI_DIR}"
cargo tauri build "$@"

DMG_PATH="$(fd --max-depth 1 --type f '^Bewegungskrieg.*\.dmg$' "${BUNDLE_DIR}/dmg" | head -n 1 || true)"

echo
echo "Desktop build finished."
echo "Bundle directory: ${BUNDLE_DIR}"

if [[ -d "${APP_PATH}" ]]; then
  echo "App bundle: ${APP_PATH}"
fi

if [[ -f "${DMG_PATH}" ]]; then
  echo "DMG: ${DMG_PATH}"
fi

#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage:
  scripts/open-tauri-game.sh [app args...]

Opens the macOS Tauri desktop shell for the RTS game. The shell starts at the
Beta/Mainline selector.

For a local dev server, pass the existing shell override through the environment:
  RTS_DESKTOP_SERVER_URL=http://127.0.0.1:<port>/ scripts/open-tauri-game.sh
EOF
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SHELL_RUN="$ROOT/desktop/maccursor-shell/run.sh"

if [[ ! -x "$SHELL_RUN" ]]; then
  echo "error: Tauri shell runner is not executable: $SHELL_RUN" >&2
  exit 1
fi

exec "$SHELL_RUN" "$@"

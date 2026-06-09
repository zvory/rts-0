#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage:
  scripts/fly-logs.sh [beta|mainline|app-name] [recent|tail] [fly logs flags...]

Examples:
  scripts/fly-logs.sh beta recent
  scripts/fly-logs.sh mainline recent
  scripts/fly-logs.sh beta tail --region ewr

Reads FLY_API_TOKEN from the environment, or from .env if it is unset.
EOF
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

channel="${1:-beta}"
mode="${2:-recent}"
if [[ $# -gt 0 ]]; then
  shift
fi
if [[ $# -gt 0 ]]; then
  shift
fi

repo_root="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
env_file="$repo_root/.env"

if [[ -z "${FLY_API_TOKEN:-}" && -f "$env_file" ]]; then
  token="$(
    sed -nE 's/^(export[[:space:]]+)?FLY_API_TOKEN=(.*)$/\2/p' "$env_file" \
      | tail -n 1
  )"
  token="${token#\"}"
  token="${token%\"}"
  token="${token#\'}"
  token="${token%\'}"
  if [[ -n "$token" ]]; then
    export FLY_API_TOKEN="$token"
  fi
fi

if [[ -z "${FLY_API_TOKEN:-}" ]]; then
  echo "error: FLY_API_TOKEN is not set and was not found in .env" >&2
  exit 2
fi

case "$channel" in
  mainline|main|prod|production)
    app="${FLY_MAINLINE_APP:-rts-0-zvorygin}"
    ;;
  beta)
    app="${FLY_BETA_APP:-rts-0-zvorygin-beta}"
    ;;
  *)
    app="$channel"
    ;;
esac

args=(logs -a "$app" --json)
case "$mode" in
  recent)
    args+=(--no-tail)
    ;;
  tail)
    ;;
  *)
    echo "error: unknown mode '$mode' (expected recent or tail)" >&2
    usage >&2
    exit 2
    ;;
esac

exec flyctl "${args[@]}" "$@"

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

Reads FLY_API_TOKEN from the environment, this worktree's .env, or the main worktree's .env.
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
env_files=("$repo_root/.env")

main_worktree="$(
  git worktree list --porcelain 2>/dev/null \
    | awk '
      /^worktree / { path = substr($0, 10) }
      /^branch refs\/heads\/main$/ { print path; exit }
    '
)"
if [[ -n "$main_worktree" && "$main_worktree" != "$repo_root" ]]; then
  env_files+=("$main_worktree/.env")
fi

if [[ -z "${FLY_API_TOKEN:-}" ]]; then
  for env_file in "${env_files[@]}"; do
    if [[ -f "$env_file" ]]; then
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
        break
      fi
    fi
  done
fi

if [[ -z "${FLY_API_TOKEN:-}" ]]; then
  for env_file in "${env_files[@]}"; do
    if [[ -f "$env_file" ]]; then
      env_hint="$env_file"
      break
    fi
  done
fi

if [[ -z "${FLY_API_TOKEN:-}" ]]; then
  echo "error: FLY_API_TOKEN is not set and was not found in ${env_hint:-.env}" >&2
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

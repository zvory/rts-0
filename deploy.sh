#!/usr/bin/env bash

set -euo pipefail

MAINLINE_APP="${FLY_MAINLINE_APP:-rts-0-zvorygin}"
BETA_APP="${FLY_BETA_APP:-rts-0-zvorygin-beta}"
LAUNCHER_APP="${FLY_LAUNCHER_APP:-rts-0-zvorygin-launcher}"

usage() {
  cat <<'EOF'
Usage:
  ./deploy.sh [mainline|beta|launcher] [commit]
  ./deploy.sh --channel mainline --commit <commit>
  ./deploy.sh --channel beta --commit <commit>
  ./deploy.sh --channel launcher --commit <commit>

Deploys to Fly.io. With no commit, deploys the current checkout. With a
commit, deploys that exact git commit from a temporary detached worktree.

Channels:
  mainline, main, production, prod, release  -> FLY_MAINLINE_APP or rts-0-zvorygin
  beta                                      -> FLY_BETA_APP or rts-0-zvorygin-beta
  launcher                                  -> FLY_LAUNCHER_APP or rts-0-zvorygin-launcher

Options:
  --app <name>       Override the Fly app name.
  --channel <name>   Deployment channel.
  --commit <rev>     Git commit/revision to deploy.
  -h, --help         Show this help.
EOF
}

channel="mainline"
commit=""
app_override=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    -h|--help)
      usage
      exit 0
      ;;
    --app)
      app_override="${2:-}"
      if [[ -z "$app_override" ]]; then
        echo "error: --app requires a value" >&2
        exit 2
      fi
      shift 2
      ;;
    --channel)
      channel="${2:-}"
      if [[ -z "$channel" ]]; then
        echo "error: --channel requires a value" >&2
        exit 2
      fi
      shift 2
      ;;
    --commit|--revision|--rev)
      commit="${2:-}"
      if [[ -z "$commit" ]]; then
        echo "error: $1 requires a value" >&2
        exit 2
      fi
      shift 2
      ;;
    mainline|main|production|prod|release|beta|launcher)
      channel="$1"
      shift
      ;;
    *)
      if [[ -z "$commit" ]]; then
        commit="$1"
        shift
      else
        echo "error: unexpected argument: $1" >&2
        usage >&2
        exit 2
      fi
      ;;
  esac
done

case "$channel" in
  mainline|main|production|prod|release)
    app="$MAINLINE_APP"
    config_file="fly.mainline.toml"
    ;;
  beta)
    app="$BETA_APP"
    config_file="fly.beta.toml"
    ;;
  launcher)
    app="$LAUNCHER_APP"
    config_file="fly.launcher.toml"
    ;;
  *)
    echo "error: unknown channel: $channel" >&2
    usage >&2
    exit 2
    ;;
esac

if [[ -n "$app_override" ]]; then
  app="$app_override"
fi

repo_root="$(git rev-parse --show-toplevel)"
deploy_dir="$repo_root"
cleanup_dir=""

cleanup() {
  if [[ -n "$cleanup_dir" ]]; then
    git -C "$repo_root" worktree remove --force "$cleanup_dir" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

if [[ -n "$commit" ]]; then
  resolved_commit="$(git -C "$repo_root" rev-parse --verify "$commit^{commit}")"
  short_commit="$(git -C "$repo_root" rev-parse --short=12 "$resolved_commit")"
  cleanup_dir="$(mktemp -d "${TMPDIR:-/tmp}/rts-deploy.XXXXXX")"
  rmdir "$cleanup_dir"
  git -C "$repo_root" worktree add --detach "$cleanup_dir" "$resolved_commit"
  deploy_dir="$cleanup_dir"
else
  resolved_commit="$(git -C "$repo_root" rev-parse HEAD)"
  short_commit="$(git -C "$repo_root" rev-parse --short=12 HEAD)"
fi

echo "Deploying $short_commit to Fly app '$app' from $deploy_dir"

config_path="$deploy_dir/$config_file"

flyctl config validate \
  --app "$app" \
  --config "$config_path" \
  --strict

deploy_cmd=(
  flyctl deploy
  --app "$app"
  --config "$config_path"
  --build-arg "COMMIT_HASH=$short_commit"
  --ha=false
  --now
)

deploy_cmd+=("$deploy_dir")

"${deploy_cmd[@]}"

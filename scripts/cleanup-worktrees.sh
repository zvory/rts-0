#!/usr/bin/env bash
# Clean merged RTS worktrees and stale per-worktree Cargo target directories.
set -euo pipefail

WORKTREE_ROOT="${RTS_WORKTREE_ROOT:-/tmp/rts-worktrees}"
TARGET_BASE_DIR="${RTS_CARGO_TARGET_BASE_DIR:-/tmp/rts-cargo-target}"
MIN_TARGET_AGE_HOURS="${RTS_WORKTREE_CLEANUP_MIN_TARGET_AGE_HOURS:-12}"
MAX_TARGET_REMOVALS="${RTS_WORKTREE_CLEANUP_MAX_TARGET_REMOVALS:-3}"
MODE="manual"
DRY_RUN=0

usage() {
  cat <<'EOF'
Usage: scripts/cleanup-worktrees.sh [--auto] [--dry-run]

Removes merged clean task worktrees and task worktrees inactive for 72 hours.
Preserves commits and backs up dirty source before removal. Protects the current
checkout, main, locked worktrees, phase-runner markers, and modified playtest notes.
Also removes stale Cargo target directories. See docs/pr-first-workflow.md.

Options:
  --auto       Non-intrusive hook mode: only runs from main and limits target cleanup.
  --dry-run    Print what would be removed without deleting it.
  -h, --help   Show this help.

Environment:
  RTS_WORKTREE_ROOT=/tmp/rts-worktrees
  RTS_CARGO_TARGET_BASE_DIR=/tmp/rts-cargo-target
  RTS_WORKTREE_CLEANUP_MIN_TARGET_AGE_HOURS=12
  RTS_WORKTREE_CLEANUP_MAX_TARGET_REMOVALS=3
EOF
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --auto) MODE="auto" ;;
    --dry-run) DRY_RUN=1 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown argument: $1" >&2; usage >&2; exit 2 ;;
  esac
  shift
done

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

current_branch="$(git branch --show-current)"
if [ "$MODE" = "auto" ] && [ "$current_branch" != "main" ]; then
  exit 0
fi

if ! git show-ref --verify --quiet refs/heads/main; then
  echo "cleanup-worktrees: local main branch is missing; skipping" >&2
  exit 0
fi

now_epoch="$(date +%s)"

run_rm_rf() {
  local path="$1"
  if [ "$DRY_RUN" = "1" ]; then
    echo "would remove $path"
  else
    rm -rf "$path"
  fi
}

target_names_for_root() {
  local root="$1"
  local repo_name
  repo_name="$(basename "$root")"

  # Protect both logical (/tmp/...) and physical (/private/tmp/...) spellings on macOS.
  local candidates=("$root")
  local tmp_alias
  tmp_alias=""
  case "$root" in
    /private/tmp/*) tmp_alias="/tmp/${root#/private/tmp/}" ;;
    /tmp/*) tmp_alias="/private/tmp/${root#/tmp/}" ;;
  esac
  if [ -n "$tmp_alias" ] && [ "$tmp_alias" != "$root" ]; then
    candidates+=("$tmp_alias")
  fi

  local physical
  physical="$(cd "$root" 2>/dev/null && pwd -P || true)"
  if [ -n "$physical" ] && [ "$physical" != "$root" ]; then
    candidates+=("$physical")
  fi

  local candidate hash
  for candidate in "${candidates[@]}"; do
    if command -v shasum >/dev/null 2>&1; then
      hash="$(printf '%s' "$candidate" | shasum -a 256 | awk '{ print substr($1, 1, 12) }')"
    else
      hash="$(printf '%s' "$candidate" | cksum | awk '{ print $1 }')"
    fi
    printf '%s\n' "${repo_name}-${hash}-server"
  done
}

target_dirs_for_root() {
  local root="$1"
  local name
  target_names_for_root "$root" | while IFS= read -r name; do
    printf '%s/%s\n' "$TARGET_BASE_DIR" "$name"
  done
}

path_mtime_epoch() {
  stat -f '%m' "$1" 2>/dev/null || stat -c '%Y' "$1"
}

path_age_hours() {
  local mtime
  mtime="$(path_mtime_epoch "$1")"
  echo $(( (now_epoch - mtime) / 3600 ))
}

active_target_names_file="$(mktemp -t rts-active-targets.XXXXXX)"
trap 'rm -f "$active_target_names_file"' EXIT

while IFS= read -r worktree_path; do
  [ -n "$worktree_path" ] || continue
  [ -d "$worktree_path" ] || continue

  while IFS= read -r target_name; do
    printf '%s\n' "$target_name" >>"$active_target_names_file"
  done < <(target_names_for_root "$worktree_path")

done < <(git worktree list --porcelain | awk '/^worktree / { sub(/^worktree /, ""); print }')

if [ "$DRY_RUN" = "1" ]; then
  python3 "$repo_root/scripts/worktree-retention.py" --dry-run
else
  python3 "$repo_root/scripts/worktree-retention.py"
fi

if [ -d "$TARGET_BASE_DIR" ]; then
  removed_targets=0
  while IFS= read -r target_dir; do
    [ -d "$target_dir" ] || continue
    target_name="$(basename "$target_dir")"

    if rg -Fxq "$target_name" "$active_target_names_file"; then
      continue
    fi

    age_hours="$(path_age_hours "$target_dir")"
    if [ "$age_hours" -lt "$MIN_TARGET_AGE_HOURS" ]; then
      continue
    fi

    echo "cleanup-worktrees: removing stale Cargo target $target_dir (${age_hours}h old)"
    run_rm_rf "$target_dir"
    removed_targets=$((removed_targets + 1))

    if [ "$MODE" = "auto" ] && [ "$removed_targets" -ge "$MAX_TARGET_REMOVALS" ]; then
      break
    fi
  done < <(fd -HI . "$TARGET_BASE_DIR" -t d -d 1 | sort)
fi

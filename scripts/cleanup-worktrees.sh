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

Removes clean, already-merged zvorygin/* worktrees under /tmp/rts-worktrees
and their Cargo target dirs. Also removes a bounded number of stale target dirs
under /tmp/rts-cargo-target that do not belong to any active worktree.

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

is_within_worktree_root() {
  local path="$1"
  case "$path" in
    "$WORKTREE_ROOT"/*|/private"$WORKTREE_ROOT"/*) return 0 ;;
    *) return 1 ;;
  esac
}

path_mtime_epoch() {
  stat -f '%m' "$1"
}

path_age_hours() {
  local mtime
  mtime="$(path_mtime_epoch "$1")"
  echo $(( (now_epoch - mtime) / 3600 ))
}

declare -a removable_worktrees=()
active_target_names_file="$(mktemp -t rts-active-targets.XXXXXX)"
trap 'rm -f "$active_target_names_file"' EXIT

while IFS= read -r worktree_path; do
  [ -n "$worktree_path" ] || continue
  [ -d "$worktree_path" ] || continue

  while IFS= read -r target_name; do
    printf '%s\n' "$target_name" >>"$active_target_names_file"
  done < <(target_names_for_root "$worktree_path")

  if ! is_within_worktree_root "$worktree_path"; then
    continue
  fi

  branch="$(git -C "$worktree_path" branch --show-current 2>/dev/null || true)"
  case "$branch" in
    zvorygin/*) ;;
    *) continue ;;
  esac

  if [ -n "$(git -C "$worktree_path" status --porcelain=v1 2>/dev/null)" ]; then
    continue
  fi

  if git merge-base --is-ancestor "$branch" main 2>/dev/null; then
    removable_worktrees+=("$worktree_path")
  fi
done < <(git worktree list --porcelain | awk '/^worktree / { sub(/^worktree /, ""); print }')

if [ "${#removable_worktrees[@]}" -gt 0 ]; then
  for worktree_path in "${removable_worktrees[@]}"; do
    branch="$(git -C "$worktree_path" branch --show-current)"
    echo "cleanup-worktrees: removing merged clean worktree $worktree_path ($branch)"

    while IFS= read -r target_dir; do
      if [ -d "$target_dir" ]; then
        run_rm_rf "$target_dir"
      fi
    done < <(target_dirs_for_root "$worktree_path")

    if [ "$DRY_RUN" = "1" ]; then
      echo "would git worktree remove $worktree_path"
      echo "would delete branch $branch"
    else
      git worktree remove "$worktree_path"
      git branch -d "$branch" >/dev/null 2>&1 || true
    fi
  done
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

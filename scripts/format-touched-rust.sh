#!/usr/bin/env bash
# Format only Rust source files changed by this branch or left modified in the worktree.
# The PR lifecycle invokes this after the final quality pass, so callers normally do not
# need to run it directly.
set -euo pipefail

BASE_REF="origin/main"

usage() {
  cat <<'EOF'
Usage: scripts/format-touched-rust.sh [--base REF]

Formats changed Rust files under server/ with the repository-pinned rustfmt. Files outside
the current branch diff and worktree changes are left untouched.
EOF
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --base)
      BASE_REF="${2:?missing --base value}"
      shift
      ;;
    --base=*)
      BASE_REF="${1#*=}"
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "format-touched-rust: unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
  shift
done

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

if ! git rev-parse --verify "$BASE_REF" >/dev/null 2>&1; then
  echo "format-touched-rust: base ref does not exist: $BASE_REF" >&2
  exit 2
fi

files=()
file_count=0
add_file() {
  local file="$1"
  local existing
  for existing in "${files[@]-}"; do
    [ -n "$existing" ] || continue
    [ "$existing" = "$file" ] && return
  done
  files[$file_count]="$file"
  file_count=$((file_count + 1))
}

while IFS= read -r file; do
  case "$file" in
    server/*.rs)
      [ -f "$file" ] && add_file "$file"
      ;;
  esac
done < <(
  {
    git diff --name-only --diff-filter=ACMR "$BASE_REF...HEAD"
    git diff --name-only --diff-filter=ACMR
    git diff --cached --name-only --diff-filter=ACMR
  } | LC_ALL=C sort -u
)

if [ "$file_count" -eq 0 ]; then
  exit 0
fi

rustfmt --edition 2021 --emit files "${files[@]}"

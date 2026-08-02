#!/usr/bin/env bash
# Prepare the lockfile-keyed shared npm cache and link it into one repository worktree.
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cache_base="${RTS_NODE_DEPS_CACHE_DIR:-/tmp/rts-node-deps}"
wait_seconds="${RTS_NODE_DEPS_WAIT_SECONDS:-180}"
quiet=0

usage() {
  cat <<'EOF'
Usage: scripts/ensure-node-deps.sh [options]

Installs the root package-lock into a shared cache and points the selected
worktree's node_modules at it. The cache is reused safely across worktrees.

Options:
  --repo DIR        Repository worktree to prepare. Default: current worktree.
  --cache-dir DIR   Shared cache root. Default: RTS_NODE_DEPS_CACHE_DIR or /tmp/rts-node-deps.
  --quiet           Suppress normal progress output.
  -h, --help        Show this help.
EOF
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --repo)
      repo_root="${2:?missing --repo value}"
      shift
      ;;
    --cache-dir)
      cache_base="${2:?missing --cache-dir value}"
      shift
      ;;
    --quiet) quiet=1 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "ensure-node-deps: unknown option: $1" >&2; usage >&2; exit 2 ;;
  esac
  shift
done

repo_root="$(cd "$repo_root" && pwd)"
package_json="$repo_root/package.json"
package_lock="$repo_root/package-lock.json"
local_node_modules="$repo_root/node_modules"

info() {
  if [ "$quiet" != "1" ]; then
    printf 'ensure-node-deps: %s\n' "$1"
  fi
}

fail() {
  echo "ensure-node-deps: $1" >&2
  exit 1
}

[ -f "$package_json" ] || fail "missing $package_json"
[ -f "$package_lock" ] || fail "missing $package_lock"
command -v node >/dev/null 2>&1 || fail "node is not installed"
command -v npm >/dev/null 2>&1 || fail "npm is not installed"
[[ "$wait_seconds" =~ ^[0-9]+$ ]] || fail "RTS_NODE_DEPS_WAIT_SECONDS must be a non-negative integer"

[ -n "$cache_base" ] || fail "refusing unsafe cache directory: <empty>"

lock_hash="$(node - "$package_json" "$package_lock" <<'NODE'
const crypto = require("node:crypto");
const fs = require("node:fs");
const digest = crypto.createHash("sha256");
for (const filename of process.argv.slice(2)) digest.update(fs.readFileSync(filename)).update("\0");
process.stdout.write(digest.digest("hex"));
NODE
)"
cache_dir="$cache_base/$lock_hash"
cache_node_modules="$cache_dir/node_modules"
ready_file="$cache_dir/.ready"
lock_dir="$cache_dir.lock"
lock_owner_file="$lock_dir/owner"
temporary_dir=""
install_log=""
owns_lock=0
lock_token="$$-${RANDOM:-0}"
ownerless_observations=0

release_lock() {
  if [ "$owns_lock" = "1" ] && [ -d "$lock_dir" ] &&
     [ "$(cat "$lock_owner_file" 2>/dev/null || true)" = "$lock_token" ]; then
    rm -f "$lock_owner_file"
    rmdir "$lock_dir" 2>/dev/null || true
  fi
  owns_lock=0
}

cleanup() {
  if [ -n "$temporary_dir" ] && [ -d "$temporary_dir" ]; then
    rm -rf "$temporary_dir"
  fi
  if [ -n "$install_log" ] && [ -f "$install_log" ]; then
    rm -f "$install_log"
  fi
  release_lock
}
trap cleanup EXIT

declared_dependencies_present() {
  [ -f "$ready_file" ] && [ -d "$cache_node_modules" ] || return 1
  cmp -s "$package_json" "$cache_dir/package.json" || return 1
  cmp -s "$package_lock" "$cache_dir/package-lock.json" || return 1
  node - "$package_json" "$cache_node_modules" <<'NODE'
const fs = require("node:fs");
const path = require("node:path");
const manifest = JSON.parse(fs.readFileSync(process.argv[2], "utf8"));
const nodeModules = process.argv[3];
const names = new Set([
  ...Object.keys(manifest.dependencies || {}),
  ...Object.keys(manifest.devDependencies || {}),
  ...Object.keys(manifest.optionalDependencies || {}),
]);
for (const name of names) {
  const installed = path.join(nodeModules, ...name.split("/"));
  if (!fs.statSync(installed, { throwIfNoEntry: false })?.isDirectory()) process.exit(1);
}
NODE
}

mkdir -p "$cache_base" || fail "could not create cache root $cache_base"
cache_base="$(cd "$cache_base" && pwd)"
case "$cache_base" in
  /|"$repo_root"|"${HOME:-/nonexistent}")
    fail "refusing unsafe cache directory: $cache_base"
    ;;
esac
cache_dir="$cache_base/$lock_hash"
cache_node_modules="$cache_dir/node_modules"
ready_file="$cache_dir/.ready"
lock_dir="$cache_dir.lock"
lock_owner_file="$lock_dir/owner"

reclaim_dead_lock() {
  local owner owner_pid quarantine
  owner="$(cat "$lock_owner_file" 2>/dev/null || true)"
  owner_pid="${owner%%-*}"
  if [[ "$owner_pid" =~ ^[0-9]+$ ]]; then
    ownerless_observations=0
    kill -0 "$owner_pid" 2>/dev/null && return 1
  else
    ownerless_observations=$((ownerless_observations + 1))
    # mkdir publishes the lock just before its owner file is written. Give a live
    # installer one polling interval to finish that tiny handoff before reclaiming.
    [ "$ownerless_observations" -ge 2 ] || return 1
  fi

  quarantine="$lock_dir.stale-$lock_token"
  if mv "$lock_dir" "$quarantine" 2>/dev/null; then
    if [ "$(cat "$quarantine/owner" 2>/dev/null || true)" = "$owner" ]; then
      info "reclaiming dependency cache lock left by process ${owner_pid:-unknown}"
      rm -rf "$quarantine"
      return 0
    fi
    # The owner changed between inspection and quarantine. Restore it when possible;
    # otherwise leave the new owner intact and discard only the quarantined directory.
    mv "$quarantine" "$lock_dir" 2>/dev/null || rm -rf "$quarantine"
  fi
  return 1
}

if ! declared_dependencies_present; then
  deadline=$((SECONDS + wait_seconds))
  while true; do
    if mkdir "$lock_dir" 2>/dev/null; then
      owns_lock=1
      printf '%s\n' "$lock_token" >"$lock_owner_file"
      break
    fi
    if declared_dependencies_present; then
      break
    fi
    if [ "$SECONDS" -ge "$deadline" ]; then
      fail "timed out waiting for dependency cache lock $lock_dir"
    fi
    reclaim_dead_lock || true
    info "waiting for dependency cache lock $lock_dir"
    sleep 1
  done

  if [ "$owns_lock" = "1" ] && ! declared_dependencies_present; then
    temporary_dir="$cache_base/.tmp-$lock_hash-$$"
    rm -rf "$temporary_dir"
    mkdir -p "$temporary_dir"
    cp "$package_json" "$package_lock" "$temporary_dir/"
    install_log="$(mktemp "${TMPDIR:-/tmp}/rts-npm-ci.XXXXXX")"
    info "installing lockfile dependencies into $cache_dir"
    if ! (cd "$temporary_dir" && npm ci --ignore-scripts --no-audit --fund=false) >"$install_log" 2>&1; then
      cat "$install_log" >&2
      fail "npm ci failed"
    fi
    rm -f "$install_log"
    install_log=""
    rm -rf "$cache_dir"
    mv "$temporary_dir" "$cache_dir"
    temporary_dir=""
    touch "$ready_file"
    release_lock
  elif [ "$owns_lock" = "1" ]; then
    # Another worktree may publish the cache just before this process acquires
    # the released lock. Never leave that no-longer-needed lock behind.
    release_lock
  fi
fi

declared_dependencies_present || fail "cache is incomplete at $cache_node_modules"

if [ -L "$local_node_modules" ]; then
  current_target="$(readlink "$local_node_modules")"
  if [ "$current_target" != "$cache_node_modules" ]; then
    rm "$local_node_modules"
  fi
elif [ -e "$local_node_modules" ]; then
  info "replacing worktree-local node_modules with the shared cache link"
  rm -rf "$local_node_modules"
fi

if [ ! -e "$local_node_modules" ]; then
  ln -s "$cache_node_modules" "$local_node_modules"
fi

info "ready: $local_node_modules -> $cache_node_modules"

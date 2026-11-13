#!/usr/bin/env bash
# Prepare the lockfile-keyed shared npm cache and link it into one repository worktree.
set -euo pipefail

repo_root=""
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

if [ -z "$repo_root" ]; then
  repo_root="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
fi
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
command -v python3 >/dev/null 2>&1 || fail "python3 is required for dependency cache locking"
[[ "$wait_seconds" =~ ^[0-9]+$ ]] || fail "RTS_NODE_DEPS_WAIT_SECONDS must be a non-negative integer"

[ -n "$cache_base" ] || fail "refusing unsafe cache directory: <empty>"

# Resolve relative cache arguments before creating anything. A cache nested in the
# destination node_modules would be removed below when the worktree-local directory
# is replaced with a symlink, leaving that symlink dangling.
cache_base="$(node - "$cache_base" <<'NODE'
const path = require("node:path");
process.stdout.write(path.resolve(process.argv[2]));
NODE
)"
case "$cache_base" in
  "$local_node_modules"|"$local_node_modules"/*)
    fail "cache directory must not be inside $local_node_modules"
    ;;
esac

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
temporary_dir=""
install_log=""

cleanup() {
  if [ -n "$temporary_dir" ] && [ -d "$temporary_dir" ]; then
    rm -rf "$temporary_dir"
  fi
  if [ -n "$install_log" ] && [ -f "$install_log" ]; then
    rm -f "$install_log"
  fi
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
# A persistent inode and an OS advisory lock replace PID files and stale-lock
# guessing. Never unlink the lock file: waiters must all lock the same inode.
# Re-exec under the lock, inheriting its descriptor through bash and npm so a
# killed wrapper cannot release the lock while its installer is still running.
if ! declared_dependencies_present; then
  if [ "${RTS_NODE_DEPS_LOCKED_CACHE:-}" != "$cache_dir" ]; then
    helper_path="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/$(basename "${BASH_SOURCE[0]}")"
    exec python3 - "$cache_dir.install.lock" "$wait_seconds" "$cache_dir" \
      "$helper_path" "$repo_root" "$cache_base" "$quiet" <<'PYLOCK'
import fcntl
import os
import sys
import time

lock_path, wait_seconds, cache_dir, helper, repo_root, cache_base, quiet = sys.argv[1:]
fd = os.open(lock_path, os.O_CREAT | os.O_RDWR, 0o600)
deadline = time.monotonic() + int(wait_seconds)
while True:
    try:
        fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        break
    except BlockingIOError:
        if time.monotonic() >= deadline:
            sys.exit(f"ensure-node-deps: timed out waiting for dependency cache lock {lock_path}")
        time.sleep(min(0.05, max(0, deadline - time.monotonic())))
os.set_inheritable(fd, True)
os.environ["RTS_NODE_DEPS_LOCKED_CACHE"] = cache_dir
args = ["bash", helper, "--repo", repo_root, "--cache-dir", cache_base]
if quiet == "1":
    args.append("--quiet")
os.execvp(args[0], args)
PYLOCK
  fi

  # Recheck after acquiring the lock: another installer may have just published.
  if ! declared_dependencies_present; then
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
    # Publish the completed install and its readiness marker in one rename.
    # Readers should only observe a cache after its ready marker is in place.
    touch "$temporary_dir/.ready"
    rm -rf "$cache_dir"
    mv "$temporary_dir" "$cache_dir"
    temporary_dir=""
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

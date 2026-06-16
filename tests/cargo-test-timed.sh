#!/usr/bin/env bash
# Run the server Cargo workspace test suite package-by-package and print a timing
# summary that is useful in CI logs.
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
SERVER_DIR="$REPO_ROOT/server"
MANIFEST_PATH="$SERVER_DIR/Cargo.toml"

if [ "$#" -gt 0 ]; then
  echo "usage: tests/cargo-test-timed.sh" >&2
  exit 2
fi

if ! command -v node >/dev/null 2>&1; then
  echo "node not found on PATH; cargo timing package discovery needs Node" >&2
  exit 2
fi

elapsed_since() { printf '%ss' "$((SECONDS - $1))"; }

packages="$(
  cargo metadata --manifest-path "$MANIFEST_PATH" --no-deps --format-version=1 \
    | node -e '
const fs = require("node:fs");
const metadata = JSON.parse(fs.readFileSync(0, "utf8"));
const defaults = new Set(metadata.workspace_default_members);
for (const pkg of metadata.packages) {
  if (defaults.has(pkg.id)) console.log(pkg.name);
}
'
)"

if [ -z "$packages" ]; then
  echo "could not discover Cargo workspace default members" >&2
  exit 1
fi

names=()
durations=()
statuses=()
failed=()
total_start=$SECONDS

while IFS= read -r package_name; do
  [ -n "$package_name" ] || continue
  names+=("$package_name")
  start=$SECONDS
  printf 'cargo-test-timing: START %s\n' "$package_name"
  if cargo test --manifest-path "$MANIFEST_PATH" -p "$package_name"; then
    status=PASS
  else
    status=FAIL
    failed+=("$package_name")
  fi
  elapsed=$((SECONDS - start))
  durations+=("$elapsed")
  statuses+=("$status")
  printf 'cargo-test-timing: %s %s (%ss)\n' "$status" "$package_name" "$elapsed"
done <<< "$packages"

total_elapsed=$((SECONDS - total_start))
printf '\nCargo test package timing summary:\n'
for i in "${!names[@]}"; do
  printf '  %-5s %5ss  %s\n' "${statuses[$i]}" "${durations[$i]}" "${names[$i]}"
done
printf '  %-5s %5ss  %s\n' "TOTAL" "$total_elapsed" "cargo test packages"

if [ "${#failed[@]}" -eq 0 ]; then
  exit 0
fi

printf '\nCargo test package failures:\n' >&2
for package_name in "${failed[@]}"; do
  printf '  %s\n' "$package_name" >&2
done
exit 1

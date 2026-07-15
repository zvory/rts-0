#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

case "${1:-}" in
  --integrated|--visual)
    shift
    TARGET_DIR="$("${ROOT}/scripts/cargo-shared-target.sh" --print-target-dir)"
    CARGO_TARGET_DIR="${TARGET_DIR}" cargo build \
      --release \
      --manifest-path "${ROOT}/server/Cargo.toml" \
      -p rts-server \
      --bin rts-server
    export CARGO_TARGET_DIR="${TARGET_DIR}"
    export RTS_SERVER_BIN="${TARGET_DIR}/release/rts-server"
    export RTS_TEST_TICK_MS="${RTS_TEST_TICK_MS:-33}"
    export RTS_CLIENT_PERF_HEADED=1
    exec node "${ROOT}/scripts/client-perf-harness.mjs" \
      --workload supply-300-hellhole-integrated \
      --seconds 30 \
      "$@"
    ;;
  -h|--help)
    cat <<'EOF'
Usage:
  scripts/hellhole-perf-harness.sh [--ticks N] [--json]
  scripts/hellhole-perf-harness.sh --integrated [client perf options]

The default is an isolated, client-free server API benchmark. --integrated (alias --visual)
starts the release server and opens the live Lab workload in a visible Chrome window.
EOF
    ;;
  *)
    exec cargo run \
      --release \
      --manifest-path "${ROOT}/server/Cargo.toml" \
      --bin hellhole-perf-harness \
      -- \
      "$@"
    ;;
esac

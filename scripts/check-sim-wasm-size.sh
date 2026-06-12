#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SERVER_MANIFEST="$ROOT/server/Cargo.toml"
RAW_WASM="$ROOT/server/target/wasm32-unknown-unknown/release/rts_sim_wasm.wasm"
MAX_BYTES="${RTS_SIM_WASM_MAX_BYTES:-1250000}"

cargo build --manifest-path "$SERVER_MANIFEST" -p rts-sim-wasm --release --target wasm32-unknown-unknown

BYTES="$(wc -c < "$RAW_WASM" | tr -d '[:space:]')"
if [ "$BYTES" -gt "$MAX_BYTES" ]; then
  echo "rts-sim-wasm raw bundle is ${BYTES} bytes, above limit ${MAX_BYTES}" >&2
  exit 1
fi

echo "rts-sim-wasm raw bundle is ${BYTES} bytes (limit ${MAX_BYTES})"

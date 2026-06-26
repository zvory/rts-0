#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SERVER_MANIFEST="$ROOT/server/Cargo.toml"
RAW_WASM="$ROOT/server/target/wasm32-unknown-unknown/release/rts_sim_wasm.wasm"
OUT_DIR="$ROOT/client/vendor/sim-wasm"

cargo build --manifest-path "$SERVER_MANIFEST" -p rts-sim-wasm --release --target wasm32-unknown-unknown --locked

mkdir -p "$OUT_DIR"

if ! command -v wasm-bindgen >/dev/null 2>&1; then
  cat >&2 <<'EOF'
wasm-bindgen CLI is required to generate the browser-loading JS glue.

Install a CLI version compatible with the wasm-bindgen crate in Cargo.lock, then rerun:
  cargo install wasm-bindgen-cli --version 0.2.123
  scripts/build-sim-wasm.sh
EOF
  exit 1
fi

wasm-bindgen "$RAW_WASM" \
  --target web \
  --out-dir "$OUT_DIR" \
  --out-name rts_sim_wasm

wc -c "$OUT_DIR/rts_sim_wasm_bg.wasm"

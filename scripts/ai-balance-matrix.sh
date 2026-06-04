#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cd "$REPO_ROOT/server"
cargo build --release --bin ai-balance-matrix
exec "$REPO_ROOT/server/target/release/ai-balance-matrix" "$@"

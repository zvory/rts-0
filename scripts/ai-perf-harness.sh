#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

export RTS_PERF="${RTS_PERF:-sample}"
export RTS_PERF_SAMPLE_EVERY="${RTS_PERF_SAMPLE_EVERY:-300}"
export RTS_PERF_LOG_SNAPSHOTS="${RTS_PERF_LOG_SNAPSHOTS:-1}"
export RUST_LOG="${RUST_LOG:-info,server::perf=debug}"

cd "${ROOT}/server"
exec cargo run --release --bin ai-perf-harness -- "$@"

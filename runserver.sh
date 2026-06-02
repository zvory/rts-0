#!/usr/bin/env bash
set -euo pipefail

PORT=${RTS_ADDR:-0.0.0.0:8080}
PORT=${PORT##*:}
lsof -ti tcp:"$PORT" | xargs kill -9 2>/dev/null || true

cd "$(dirname "$0")/server"
exec cargo run

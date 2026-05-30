#!/usr/bin/env bash
set -euo pipefail

# Kill any existing rts-server on port 8080
PID=$(lsof -t -i :8080 2>/dev/null || true)
if [ -n "${PID:-}" ]; then
    echo "Killing existing server (PID: $PID)..."
    kill $PID
    sleep 0.5
fi

cd "$(dirname "$0")/server"
exec cargo run

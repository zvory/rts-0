#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")" && pwd)"
LOG_FILE="$(mktemp -t runaireplay.XXXXXX.log)"
SERVER_PID=""

cleanup() {
    if [ -n "$SERVER_PID" ] && kill -0 "$SERVER_PID" 2>/dev/null; then
        kill "$SERVER_PID" 2>/dev/null || true
    fi
}
trap cleanup EXIT

kill_port_8080() {
    local pids
    pids=$(lsof -ti :8080 2>/dev/null || true)
    if [ -z "$pids" ]; then
        return
    fi
    echo "Stopping existing server on :8080..."
    kill $pids 2>/dev/null || true
    for i in $(seq 1 10); do
        if ! lsof -ti :8080 >/dev/null 2>&1; then
            return
        fi
        sleep 1
    done
    pids=$(lsof -ti :8080 2>/dev/null || true)
    if [ -n "$pids" ]; then
        echo "Force stopping lingering server on :8080..."
        kill -9 $pids 2>/dev/null || true
    fi
}

echo "Running real_ai_vs_real_ai test..."
set +e
(cd "$REPO_ROOT/server" && cargo test real_ai_vs_real_ai -- --nocapture) >"$LOG_FILE" 2>&1
TEST_STATUS=$?
set -e
cat "$LOG_FILE"

ARTIFACT=$(rg -o 'REPLAY_ARTIFACT=[^ ]+' "$LOG_FILE" | head -1 | cut -d= -f2)
if [ -z "$ARTIFACT" ]; then
    ARTIFACT=$(rg -o 'view replay: http://localhost:8080/dev/selfplay\?replay=[^ ]+' "$LOG_FILE" | head -1 | sed 's/.*replay=//')
fi
if [ -z "$ARTIFACT" ]; then
    echo "ERROR: could not find REPLAY_ARTIFACT in test output" >&2
    exit 1
fi
echo "Artifact: $ARTIFACT"

kill_port_8080
echo "Starting server..."
cd "$REPO_ROOT/server"
cargo run &
SERVER_PID=$!
echo "Server PID: $SERVER_PID"
for i in $(seq 1 30); do
    if lsof -ti :8080 >/dev/null 2>&1; then
        break
    fi
    sleep 1
done

open "http://localhost:8080/dev/selfplay?replay=${ARTIFACT}"

if [ "$TEST_STATUS" -ne 0 ]; then
    echo "Test failed with status $TEST_STATUS; replay opened above." >&2
    exit "$TEST_STATUS"
fi

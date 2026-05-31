#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")" && pwd)"
LOG_FILE="$(mktemp -t runaireplay.XXXXXX.log)"
SERVER_PID=""
SERVER_URL=""

cleanup() {
    if [ -n "$SERVER_PID" ] && kill -0 "$SERVER_PID" 2>/dev/null; then
        kill "$SERVER_PID" 2>/dev/null || true
    fi
}
trap cleanup EXIT

alloc_port() {
    node -e 'const net = require("node:net"); const s = net.createServer(); s.listen(0, "127.0.0.1", () => { console.log(s.address().port); s.close(); });'
}

if ! command -v node >/dev/null 2>&1; then
    echo "node not found on PATH — this script needs Node to allocate a free port." >&2
    exit 2
fi

echo "Running real_ai_vs_real_ai test..."
set +e
(cd "$REPO_ROOT/server" && cargo test real_ai_vs_real_ai -- --nocapture) >"$LOG_FILE" 2>&1
TEST_STATUS=$?
set -e
cat "$LOG_FILE"

ARTIFACT=$(rg -o 'REPLAY_ARTIFACT=[^ ]+' "$LOG_FILE" | head -1 | cut -d= -f2)
if [ -z "$ARTIFACT" ]; then
    ARTIFACT=$(rg -o 'view replay: /dev/selfplay\?replay=[^ ]+' "$LOG_FILE" | head -1 | sed 's/.*replay=//')
fi
if [ -z "$ARTIFACT" ]; then
    echo "ERROR: could not find REPLAY_ARTIFACT in test output" >&2
    exit 1
fi
echo "Artifact: $ARTIFACT"

PORT="$(alloc_port)"
SERVER_URL="http://127.0.0.1:${PORT}"
echo "Starting server on $SERVER_URL..."
cd "$REPO_ROOT/server"
RTS_ADDR="127.0.0.1:${PORT}" cargo run >"$LOG_FILE.server" 2>&1 &
SERVER_PID=$!
for i in $(seq 1 30); do
    if curl -fsS --max-time 1 "$SERVER_URL/" >/dev/null 2>&1; then
        break
    fi
    sleep 1
done

open "${SERVER_URL}/dev/selfplay?replay=${ARTIFACT}"

if [ "$TEST_STATUS" -ne 0 ]; then
    echo "Test failed with status $TEST_STATUS; replay opened above." >&2
    exit "$TEST_STATUS"
fi
